use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::body::Body;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Json, Response, Sse};
use axum::routing::{get, post};
use axum::Router;
use axum_server::tls_rustls::RustlsConfig;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use rand::RngCore;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::broadcast;
use tokio_util::io::ReaderStream;

use crate::app::AppState;
use crate::features::network::sse::{self, sse_stream, SseEvent};
use crate::infra::db::Database;
use crate::models::SongId;

const SESSION_COOKIE: &str = "vide_session";
const SESSION_TTL: Duration = Duration::from_secs(60 * 60 * 24 * 30);

struct Session {
    csrf: String,
    expires: u64,
}

struct AuthState {
    sessions: Mutex<HashMap<String, Session>>,
    password_hash: Mutex<Option<String>>,
    failed_logins: Mutex<(u32, u64)>,
    db: Arc<Database>,
}

impl AuthState {
    async fn new(db: Arc<Database>) -> Self {
        let password_hash = db
            .get_setting("server_password")
            .await
            .ok()
            .flatten()
            .and_then(|value| value.as_str().map(ToString::to_string));
        let mut sessions = HashMap::new();
        if let Ok(Some(Value::Array(items))) = db.get_setting("server_sessions").await {
            let now = now_secs();
            for item in items {
                let Some(token) = item.get("token").and_then(Value::as_str) else { continue };
                let Some(csrf) = item.get("csrf").and_then(Value::as_str) else { continue };
                let Some(expires) = item.get("expires").and_then(Value::as_u64) else { continue };
                if expires > now {
                    sessions.insert(token.to_string(), Session { csrf: csrf.to_string(), expires });
                }
            }
        }
        Self {
            sessions: Mutex::new(sessions),
            password_hash: Mutex::new(password_hash),
            failed_logins: Mutex::new((0, 0)),
            db,
        }
    }

    fn verify(&self, password: &str) -> bool {
        let guard = match self.password_hash.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        let Some(hash) = guard.as_ref() else {
            return false;
        };
        let Ok(parsed) = PasswordHash::new(hash) else {
            return false;
        };
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    }

    fn allow_login(&self) -> bool {
        let Ok(guard) = self.failed_logins.lock() else {
            return false;
        };
        let (count, since) = *guard;
        count < 5 || now_secs().saturating_sub(since) >= 60
    }

    fn record_login_failure(&self) {
        if let Ok(mut guard) = self.failed_logins.lock() {
            let now = now_secs();
            if now.saturating_sub(guard.1) >= 60 {
                *guard = (0, now);
            }
            guard.0 = guard.0.saturating_add(1);
            guard.1 = now;
        }
    }

    fn reset_login_failures(&self) {
        if let Ok(mut guard) = self.failed_logins.lock() {
            *guard = (0, 0);
        }
    }

    async fn login(&self) -> (String, String) {
        let mut token_bytes = [0u8; 32];
        let mut csrf_bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut token_bytes);
        rand::thread_rng().fill_bytes(&mut csrf_bytes);
        let token = hex(&token_bytes);
        let csrf = hex(&csrf_bytes);
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.insert(
                token.clone(),
                Session {
                    csrf: csrf.clone(),
                    expires: now_secs() + SESSION_TTL.as_secs(),
                },
            );
        }
        self.persist().await;
        (token, csrf)
    }

    fn session(&self, token: &str) -> Option<String> {
        let mut sessions = self.sessions.lock().ok()?;
        let session = sessions.get(token)?;
        if session.expires <= now_secs() {
            sessions.remove(token);
            return None;
        }
        Some(session.csrf.clone())
    }

    async fn logout(&self, token: &str) {
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.remove(token);
        }
        self.persist().await;
    }

    async fn persist(&self) {
        let items = match self.sessions.lock() {
            Ok(sessions) => sessions
                .iter()
                .map(|(token, session)| {
                    json!({
                        "token": token,
                        "csrf": session.csrf,
                        "expires": session.expires,
                    })
                })
                .collect::<Vec<_>>(),
            Err(_) => return,
        };
        let _ = self
            .db
            .update_setting("server_sessions", &Value::Array(items))
            .await;
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| e.to_string())
}

#[derive(Clone)]
struct ServerState {
    app: Arc<AppState>,
    auth: Arc<AuthState>,
    public_media: bool,
    data_dir: PathBuf,
    web_dir: PathBuf,
    audit_path: PathBuf,
    secure_cookies: bool,
    sse_tx: broadcast::Sender<SseEvent>,
}

fn media_path(state: &ServerState, song: &crate::models::Song) -> Option<PathBuf> {
    song.paths.iter().find(|path| path.is_file()).cloned().or_else(|| {
        let candidate = state.data_dir.join("cache").join(format!("{}.media", song.id.0));
        if candidate.is_file() {
            Some(candidate)
        } else {
            None
        }
    })
}

fn cookie_token(headers: &HeaderMap) -> Option<String> {
    let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
    cookie.split(';').find_map(|part| {
        let part = part.trim();
        part.strip_prefix(SESSION_COOKIE)
            .and_then(|value| value.strip_prefix('='))
            .map(ToString::to_string)
    })
}

fn request_token(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers.get(header::AUTHORIZATION).and_then(|value| value.to_str().ok()) {
        if let Some(token) = value.strip_prefix("Bearer ") {
            return Some(token.trim().to_string());
        }
    }
    cookie_token(headers)
}

fn require_session(state: &ServerState, headers: &HeaderMap, csrf: Option<&str>) -> Result<String, StatusCode> {
    let token = request_token(headers).ok_or(StatusCode::UNAUTHORIZED)?;
    let session_csrf = state.auth.session(&token).ok_or(StatusCode::UNAUTHORIZED)?;
    if let Some(csrf) = csrf {
        if csrf != session_csrf {
            return Err(StatusCode::FORBIDDEN);
        }
    }
    Ok(token)
}

fn require_csrf(state: &ServerState, headers: &HeaderMap) -> Result<(), StatusCode> {
    let csrf = headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok());
    require_session(state, headers, csrf).map(|_| ())
}

fn audit(state: &ServerState, action: &str, detail: &str) {
    let line = format!("{} {action} {detail}\n", now_secs());
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&state.audit_path)
    {
        use std::io::Write;
        let _ = file.write_all(line.as_bytes());
    }
}

async fn status(State(state): State<ServerState>) -> Json<Value> {
    Json(json!({
        "name": "Vide",
        "version": "0.2.0",
        "api": "v1",
        "public_media": state.public_media
    }))
}

#[derive(Deserialize)]
struct LoginRequest {
    password: String,
}

async fn login(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Result<Response, StatusCode> {
    if !state.auth.allow_login() {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    if !state.auth.verify(&request.password) {
        state.auth.record_login_failure();
        audit(&state, "login_failed", "");
        return Err(StatusCode::UNAUTHORIZED);
    }
    state.auth.reset_login_failures();
    let (token, csrf) = state.auth.login().await;
    audit(&state, "login_ok", "");
    let secure = state.secure_cookies
        || headers
            .get("x-forwarded-proto")
            .and_then(|value| value.to_str().ok())
            .map(|value| value == "https")
            .unwrap_or(false);
    let cookie = format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax{}",
        if secure { "; Secure" } else { "" }
    );
    let mut response = Json(json!({
        "authenticated": true,
        "csrf": csrf,
        "token": token
    }))
    .into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    );
    Ok(response)
}

async fn logout(State(state): State<ServerState>, headers: HeaderMap) -> Response {
    if let Some(token) = request_token(&headers) {
        state.auth.logout(&token).await;
        audit(&state, "logout", "");
    }
    let mut response = Json(json!({ "authenticated": false })).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static("vide_session=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax"),
    );
    response
}

async fn me(State(state): State<ServerState>, headers: HeaderMap) -> Json<Value> {
    let Some(token) = request_token(&headers) else {
        return Json(json!({ "authenticated": false }));
    };
    match state.auth.session(&token) {
        Some(csrf) => Json(json!({ "authenticated": true, "csrf": csrf })),
        None => Json(json!({ "authenticated": false })),
    }
}

fn public_song(mut song: crate::models::Song) -> Value {
    // The read-only API needs a song id, metadata and sources; local absolute
    // paths are private and are never sent to clients.
    let local = !song.paths.is_empty();
    song.paths.clear();
    song.metadata.path = None;
    let mut value = serde_json::to_value(song).unwrap_or_default();
    if let Some(object) = value.as_object_mut() {
        object.insert("local".to_string(), Value::Bool(local));
    }
    value
}

async fn list_songs(State(state): State<ServerState>) -> Result<Json<Value>, StatusCode> {
    let songs = state
        .app
        .db
        .list_library_songs()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let songs: Vec<_> = songs.into_iter().map(public_song).collect();
    Ok(Json(json!(songs)))
}

async fn get_song(
    State(state): State<ServerState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Value>, StatusCode> {
    let song = state
        .app
        .db
        .get_song(&SongId(id))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(
        serde_json::to_value(public_song(song)).unwrap_or_default(),
    ))
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
}

async fn search(
    State(state): State<ServerState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Value>, StatusCode> {
    let songs = crate::features::search::search(&state.app.db, &query.q)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let songs: Vec<_> = songs.into_iter().map(public_song).collect();
    Ok(Json(json!(songs)))
}

async fn ensure_media_path(
    state: &ServerState,
    song: &crate::models::Song,
) -> Option<PathBuf> {
    if let Some(path) = media_path(state, song) {
        return Some(path);
    }
    let source = song.sources.first()?;
    let plugins = state.app.plugins.as_ref()?;
    let descriptor = plugins
        .stream(&source.platform, &source.id, &song.id.0)
        .await
        .ok()?;
    let cache = crate::features::cache::CacheManager::new(
        state.app.db.as_ref().clone(),
        &state.data_dir,
    )
    .await
    .ok()?;
    cache
        .download_from_descriptor(&song.id, &descriptor)
        .await
        .ok()
}

#[derive(Deserialize)]
struct ImageQuery {
    url: String,
    referer: Option<String>,
}

async fn image_proxy(Query(query): Query<ImageQuery>) -> Result<Response, StatusCode> {
    if !query.url.starts_with("http://") && !query.url.starts_with("https://") {
        return Err(StatusCode::BAD_REQUEST);
    }
    let client = reqwest::Client::new();
    let mut request = client
        .get(&query.url)
        .header(reqwest::header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120 Safari/537.36");
    if let Some(referer) = query.referer {
        request = request.header(reqwest::header::REFERER, referer);
    }
    let response = request.send().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    if !response.status().is_success() {
        return Err(StatusCode::BAD_GATEWAY);
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();
    if !content_type.starts_with("image/") {
        return Err(StatusCode::BAD_GATEWAY);
    }
    let bytes = response.bytes().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "public, max-age=86400")
        .body(Body::from(bytes))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn download_song(
    State(state): State<ServerState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Response, StatusCode> {
    require_session(&state, &headers, None)?;
    let song = state
        .app
        .db
        .get_song(&SongId(id))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let path = ensure_media_path(&state, &song)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let title = song
        .metadata
        .display_title
        .clone()
        .unwrap_or_else(|| song.title.clone());
    let extension = match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
    {
        "" | "media" | "tmp" | "bin" => "mp3",
        other => other,
    };
    let filename = format!("{}.{}", title.replace(['"', '\\', '/', '\n', '\r'], "_"), extension);
    Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .body(Body::from(bytes))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn media(
    State(state): State<ServerState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Response, StatusCode> {
    if !state.public_media && require_session(&state, &headers, None).is_err() {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let song = state
        .app
        .db
        .get_song(&SongId(id))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let path = ensure_media_path(&state, &song)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;
    let metadata = tokio::fs::metadata(&path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let total = metadata.len();
    let content_type = mime_guess::from_path(&path)
        .first_or_octet_stream()
        .to_string();

    let range = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_range);

    if let Some((start, end)) = range {
        if start >= total || start > end {
            return Ok(Response::builder()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(header::CONTENT_RANGE, format!("bytes */{total}"))
                .body(Body::empty())
                .unwrap());
        }
        let end = end.min(total.saturating_sub(1));
        let length = end - start + 1;
        let mut file = tokio::fs::File::open(&path)
            .await
            .map_err(|_| StatusCode::NOT_FOUND)?;
        file.seek(std::io::SeekFrom::Start(start))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let stream = ReaderStream::new(file.take(length));
        return Ok(Response::builder()
            .status(StatusCode::PARTIAL_CONTENT)
            .header(header::CONTENT_TYPE, content_type)
            .header(header::CONTENT_LENGTH, length.to_string())
            .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{total}"))
            .header(header::ACCEPT_RANGES, "bytes")
            .body(Body::from_stream(stream))
            .unwrap());
    }

    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, total.to_string())
        .header(header::ACCEPT_RANGES, "bytes")
        .body(Body::from_stream(ReaderStream::new(file)))
        .unwrap())
}

fn parse_range(value: &str) -> Option<(u64, u64)> {
    let value = value.strip_prefix("bytes=")?;
    let (start, end) = value.split_once('-')?;
    let start = start.parse().ok()?;
    let end = if end.is_empty() {
        u64::MAX
    } else {
        end.parse().ok()?
    };
    Some((start, end))
}

#[derive(Deserialize)]
struct PlayModeRequest {
    mode: crate::features::player::PlayMode,
}

async fn set_play_mode(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<PlayModeRequest>,
) -> Result<Json<Value>, StatusCode> {
    require_csrf(&state, &headers)?;
    let mut player = state.app.player.lock().await;
    player.set_play_mode(request.mode);
    let status = player.status();
    sse::publish_status(&state.sse_tx, &status);
    Ok(Json(serde_json::to_value(status).unwrap_or_default()))
}

async fn player_status(State(state): State<ServerState>) -> Json<Value> {
    let mut status = {
        let mut player = state.app.player.lock().await;
        player.check_alive().await;
        player.refresh().await;
        player.status()
    };
    if status.media_id.is_none() {
        if let Ok(Some((song_id, position))) = state.app.db.load_last_played().await {
            if !song_id.is_empty() {
                status.media_id = Some(song_id.clone());
                status.elapsed_secs = Some(position);
                if let Ok(Some(song)) = state.app.db.get_song(&SongId(song_id)).await {
                    if let Some(duration) = song.metadata.duration {
                        status.duration_secs = Some(duration);
                    }
                }
            }
        }
    }
    Json(serde_json::to_value(status).unwrap_or_default())
}

async fn player_stream(
    State(state): State<ServerState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>> {
    sse_stream(state.sse_tx.subscribe())
}

#[derive(Deserialize)]
struct PlayRequest {
    id: String,
    #[serde(default)]
    position_secs: Option<f64>,
}

async fn player_play(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<PlayRequest>,
) -> Result<Json<Value>, StatusCode> {
    require_csrf(&state, &headers)?;
    let song = state
        .app
        .db
        .get_song(&SongId(request.id.clone()))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let path = media_path(&state, &song).ok_or(StatusCode::NOT_FOUND)?;
    let song_id = request.id;
    let descriptor = crate::features::plugin::types::StreamDescriptor::UrlStream {
        id: song_id.clone(),
        url: path.to_string_lossy().to_string(),
        headers: None,
    };
    let mut player = state.app.player.lock().await;
    player
        .play(&descriptor)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Some(position) = request.position_secs {
        if position > 0.0 {
            let _ = player.seek_when_ready(position).await;
        }
    }
    let status = player.status();
    audit(&state, "play", &song_id);
    sse::publish_status(&state.sse_tx, &status);
    Ok(Json(serde_json::to_value(status).unwrap_or_default()))
}

async fn player_stop(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    require_csrf(&state, &headers)?;
    let mut player = state.app.player.lock().await;
    let status = player.stop().await;
    sse::publish_status(&state.sse_tx, &status);
    Ok(Json(serde_json::to_value(status).unwrap_or_default()))
}

async fn player_next(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    require_csrf(&state, &headers)?;
    let mut player = state.app.player.lock().await;
    let status = player.play_next().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sse::publish_status(&state.sse_tx, &status);
    Ok(Json(serde_json::to_value(status).unwrap_or_default()))
}

async fn player_prev(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    require_csrf(&state, &headers)?;
    let mut player = state.app.player.lock().await;
    let status = player.play_prev().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sse::publish_status(&state.sse_tx, &status);
    Ok(Json(serde_json::to_value(status).unwrap_or_default()))
}

async fn persist_plugin_config(
    state: &ServerState,
    config: &crate::features::plugin::config::PluginConfig,
) -> Result<(), StatusCode> {
    let value = serde_json::to_value(config).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state
        .app
        .db
        .update_setting("plugin_config", &value)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}

async fn plugin_full_state(state: &ServerState) -> Result<Json<Value>, StatusCode> {
    let plugins = state
        .app
        .plugins
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(Json(serde_json::to_value(plugins.state().await).unwrap_or_default()))
}

async fn reload_plugins(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    require_csrf(&state, &headers)?;
    let plugins = state.app.plugins.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    plugins.reload().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    plugin_full_state(&state).await
}

#[derive(Deserialize)]
struct PluginEnabledRequest {
    name: String,
    enabled: bool,
}

async fn set_plugin_enabled(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<PluginEnabledRequest>,
) -> Result<Json<Value>, StatusCode> {
    require_csrf(&state, &headers)?;
    let plugins = state.app.plugins.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let config = plugins
        .set_plugin_enabled(&request.name, request.enabled)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    persist_plugin_config(&state, &config).await?;
    plugin_full_state(&state).await
}

#[derive(Deserialize)]
struct PluginArgvRequest {
    name: String,
    argv: Vec<String>,
    platform: Option<String>,
}

async fn set_plugin_argv(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<PluginArgvRequest>,
) -> Result<Json<Value>, StatusCode> {
    require_csrf(&state, &headers)?;
    let plugins = state.app.plugins.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let config = plugins
        .set_plugin_argv(&request.name, request.argv)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    persist_plugin_config(&state, &config).await?;
    plugin_full_state(&state).await
}

async fn set_plugin_platform_argv(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<PluginArgvRequest>,
) -> Result<Json<Value>, StatusCode> {
    require_csrf(&state, &headers)?;
    let plugins = state.app.plugins.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let platform = request.platform.unwrap_or_default();
    let config = plugins
        .set_platform_argv(&request.name, &platform, request.argv)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    persist_plugin_config(&state, &config).await?;
    plugin_full_state(&state).await
}

#[derive(Deserialize)]
struct PluginRouteArgvRequest {
    name: String,
    platform: String,
    capability: String,
    argv: Vec<String>,
}

async fn set_plugin_route_argv(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<PluginRouteArgvRequest>,
) -> Result<Json<Value>, StatusCode> {
    require_csrf(&state, &headers)?;
    let plugins = state.app.plugins.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let config = plugins
        .set_route_argv(&request.name, &request.platform, &request.capability, request.argv)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    persist_plugin_config(&state, &config).await?;
    plugin_full_state(&state).await
}

#[derive(Deserialize)]
struct PluginConcurrencyRequest {
    name: String,
    concurrency: Option<u32>,
}

async fn set_plugin_concurrency(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<PluginConcurrencyRequest>,
) -> Result<Json<Value>, StatusCode> {
    require_csrf(&state, &headers)?;
    let plugins = state.app.plugins.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let config = plugins
        .set_plugin_concurrency(&request.name, request.concurrency)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    persist_plugin_config(&state, &config).await?;
    plugin_full_state(&state).await
}

#[derive(Deserialize)]
struct PluginRouteRequest {
    platform: String,
    capability: String,
    providers: Vec<String>,
}

async fn set_plugin_route(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<PluginRouteRequest>,
) -> Result<Json<Value>, StatusCode> {
    require_csrf(&state, &headers)?;
    let plugins = state.app.plugins.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let config = plugins
        .set_route_providers(&request.platform, &request.capability, request.providers)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    persist_plugin_config(&state, &config).await?;
    plugin_full_state(&state).await
}

async fn plugin_state_public(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Json<Value> {
    let Some(plugins) = state.app.plugins.as_ref() else {
        return Json(json!({ "plugin_dir": "", "plugins": [], "routes": [] }));
    };
    let full = plugins.state().await;
    if request_token(&headers)
        .and_then(|token| state.auth.session(&token))
        .is_some()
    {
        return Json(serde_json::to_value(full).unwrap_or_default());
    }
    let routes = full
        .routes
        .iter()
        .map(|route| {
            json!({
                "platform": route.platform,
                "capability": route.capability,
                "providers": route.providers.iter().map(|provider| json!({
                    "plugin": provider.plugin,
                    "enabled": provider.enabled,
                    "argv": [],
                })).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    Json(json!({
        "plugin_dir": "",
        "plugins": [],
        "routes": routes,
    }))
}

#[derive(Deserialize)]
struct OnlineSearchRequest {
    query: String,
    platform: String,
    limit: Option<u32>,
}

async fn search_online(
    State(state): State<ServerState>,
    Json(request): Json<OnlineSearchRequest>,
) -> Result<Json<Value>, StatusCode> {
    let plugins = state
        .app
        .plugins
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let tracks = plugins
        .search(
            &request.platform,
            &request.query,
            request.limit.unwrap_or(20),
        )
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    Ok(Json(serde_json::to_value(tracks).unwrap_or_default()))
}

#[derive(Deserialize)]
struct ResolveRequest {
    platform: String,
    source_id: String,
}

async fn ensure_online_song(
    state: &ServerState,
    platform: &str,
    source_id: &str,
) -> Result<crate::models::Song, StatusCode> {
    let song_id = crate::infra::hash::song_id_from_source(platform, source_id);
    let plugins = state
        .app
        .plugins
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    if let Some(mut song) = state
        .app
        .db
        .get_song(&song_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        let needs_metadata = song.metadata.cover_url.is_none()
            || song.metadata.duration.is_none()
            || song.metadata.added_time.is_none();
        if needs_metadata {
            if let Ok(item) = plugins.resolve(platform, source_id).await {
                if song.metadata.cover_url.is_none() {
                    song.metadata.cover_url = item.cover_url.clone();
                }
                if song.metadata.duration.is_none() {
                    song.metadata.duration = item.duration;
                }
                if song.metadata.added_time.is_none() {
                    song.metadata.added_time = Some(crate::infra::time::now_date());
                }
                let _ = state.app.db.upsert_song(&song).await;
            }
        }
        return Ok(song);
    }
    let item = plugins
        .resolve(platform, source_id)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    let song = crate::models::Song {
        id: song_id,
        title: item.title.clone(),
        artist: item.artist.clone().unwrap_or_else(|| "Unknown".to_string()),
        album: item.album.clone().unwrap_or_else(|| "Unknown".to_string()),
        tags: Vec::new(),
        sources: vec![crate::models::Source {
            platform: platform.to_string(),
            id: source_id.to_string(),
        }],
        paths: Vec::new(),
        metadata: crate::models::SongMetadata {
            duration: item.duration,
            cover_url: item.cover_url.clone(),
            added_time: Some(crate::infra::time::now_date()),
            ..Default::default()
        },
    };
    state
        .app
        .db
        .upsert_song(&song)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(song)
}

async fn resolve_song(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<ResolveRequest>,
) -> Result<Json<Value>, StatusCode> {
    // Resolving is part of read-only search/playback. When media is public,
    // guests may resolve search results too; private servers still require
    // a session.
    if !state.public_media {
        require_session(&state, &headers, None)?;
    }
    let song = ensure_online_song(&state, &request.platform, &request.source_id).await?;
    Ok(Json(serde_json::to_value(public_song(song)).unwrap_or_default()))
}

#[derive(Deserialize)]
struct PlayOnlineRequest {
    results: Vec<crate::features::plugin::types::Track>,
    index: isize,
}

async fn play_online_results(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<PlayOnlineRequest>,
) -> Result<Json<Value>, StatusCode> {
    // Search result playback is part of the public search experience when
    // media is public. Private servers still require a session.
    if !state.public_media {
        require_csrf(&state, &headers)?;
    }
    let mut ids = Vec::with_capacity(request.results.len());
    for item in &request.results {
        let song = ensure_online_song(&state, &item.platform, &item.source_id).await?;
        ids.push(song.id.0);
    }
    let mut player = state.app.player.lock().await;
    let status = player
        .set_queue(ids, request.index)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sse::publish_status(&state.sse_tx, &status);
    Ok(Json(serde_json::to_value(status).unwrap_or_default()))
}

async fn song_tags(
    State(state): State<ServerState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Value>, StatusCode> {
    let tags = crate::features::tags::get_song_tags(&state.app.db, &SongId(id))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!(tags)))
}

async fn tag_history(State(state): State<ServerState>) -> Result<Json<Value>, StatusCode> {
    let values = crate::features::tags::get_tag_history(&state.app.db, 50)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!(values)))
}

async fn namespaces(State(state): State<ServerState>) -> Result<Json<Value>, StatusCode> {
    let values = crate::features::tags::get_namespaces(&state.app.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!(values)))
}

async fn meta_tags(State(state): State<ServerState>) -> Result<Json<Value>, StatusCode> {
    let values = crate::features::tags::get_meta_tags(&state.app.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!(values)))
}

#[derive(Deserialize)]
struct TagValuesQuery {
    namespace: String,
    #[serde(default)]
    prefix: String,
    limit: Option<usize>,
}

async fn tag_values(
    State(state): State<ServerState>,
    Query(query): Query<TagValuesQuery>,
) -> Result<Json<Value>, StatusCode> {
    let values = state
        .app
        .db
        .tag_values(&query.namespace, &query.prefix, query.limit.unwrap_or(20).clamp(1, 50))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!(values)))
}

#[derive(serde::Serialize, Deserialize)]
struct ServerSettings {
    library_roots: Vec<String>,
    exclude_roots: Vec<String>,
}

async fn cache_stats(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    require_session(&state, &headers, None)?;
    let entries = state
        .app
        .db
        .list_cache_entries()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut audio_count = 0u64;
    let mut audio_size = 0u64;
    for entry in &entries {
        if let Ok(meta) = tokio::fs::metadata(&entry.path).await {
            audio_count += 1;
            audio_size += meta.len();
        }
    }
    let songs = state
        .app
        .db
        .list_library_songs()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut lyric_count = 0u64;
    let mut metadata_count = 0u64;
    for song in &songs {
        if song.metadata.lyrics.as_deref().map(|value| !value.trim().is_empty()).unwrap_or(false) {
            lyric_count += 1;
        }
        if serde_json::to_value(&song.metadata)
            .ok()
            .and_then(|value| value.as_object().map(|object| object.values().any(|value| !value.is_null())))
            .unwrap_or(false)
        {
            metadata_count += 1;
        }
    }
    Ok(Json(json!([
        { "kind": "image", "label": "图片缓存", "count": 0, "size": 0 },
        { "kind": "audio", "label": "音频缓存", "count": audio_count, "size": audio_size },
        { "kind": "lyric", "label": "歌词缓存", "count": lyric_count, "size": null },
        { "kind": "metadata", "label": "元信息缓存", "count": metadata_count, "size": null },
    ])))
}

#[derive(Deserialize)]
struct CacheClearRequest {
    kind: String,
}

async fn clear_song_cache(
    State(state): State<ServerState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Value>, StatusCode> {
    require_csrf(&state, &headers)?;
    let song_id = SongId(id);
    let entries = state
        .app
        .db
        .get_cache_entries(&song_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Some(entry) = entries.first() {
        let _ = tokio::fs::remove_file(&entry.path).await;
        let _ = tokio::fs::remove_file(entry.path.with_extension("tmp")).await;
    }
    state
        .app
        .db
        .delete_cache_entry(&song_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    audit(&state, "cache_clear_song", &song_id.0);
    Ok(Json(json!({ "removed": entries.len() })))
}

async fn clear_cache(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<CacheClearRequest>,
) -> Result<Json<Value>, StatusCode> {
    require_csrf(&state, &headers)?;
    let mut removed = 0u64;
    match request.kind.as_str() {
        "audio" => {
            let entries = state
                .app
                .db
                .list_cache_entries()
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            for entry in &entries {
                if tokio::fs::remove_file(&entry.path).await.is_ok() {
                    removed += 1;
                }
            }
            state
                .app
                .db
                .delete_all_cache_entries()
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
        "lyric" => {
            let songs = state
                .app
                .db
                .list_library_songs()
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            for song in songs {
                if song.metadata.lyrics.as_deref().map(|value| !value.trim().is_empty()).unwrap_or(false) {
                    let mut updated = song;
                    updated.metadata.lyrics = None;
                    state.app.db.upsert_song(&updated).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    removed += 1;
                }
            }
        }
        "metadata" => {
            let songs = state
                .app
                .db
                .list_library_songs()
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            for song in songs {
                let has_metadata = serde_json::to_value(&song.metadata)
                    .ok()
                    .and_then(|value| value.as_object().map(|object| object.values().any(|value| !value.is_null())))
                    .unwrap_or(false);
                if has_metadata {
                    let mut updated = song;
                    updated.metadata = Default::default();
                    state.app.db.upsert_song(&updated).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    removed += 1;
                }
            }
        }
        "image" => {}
        _ => return Err(StatusCode::BAD_REQUEST),
    }
    audit(&state, "cache_clear", &request.kind);
    Ok(Json(json!({ "removed": removed })))
}

async fn scan_library(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    require_csrf(&state, &headers)?;
    let (roots, excludes) = {
        let config = state.app.config.lock().await;
        (config.library_roots.clone(), config.exclude_roots.clone())
    };
    let extensions = state
        .app
        .db
        .load_app_settings()
        .await
        .map(|settings| settings.scan_extensions)
        .unwrap_or_else(|_| crate::models::settings::default_scan_extensions());
    let extensions = if extensions.is_empty() {
        crate::models::settings::default_scan_extensions()
    } else {
        extensions
    };
    audit(&state, "scan", "");
    let result = crate::features::library::scan_roots(
        &state.app.db,
        &roots,
        &excludes,
        &extensions,
    )
    .await;
    Ok(Json(serde_json::to_value(result).unwrap_or_default()))
}

async fn get_server_settings(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    require_session(&state, &headers, None)?;
    let config = state.app.config.lock().await;
    let roots = config
        .library_roots
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let excludes = config
        .exclude_roots
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let scan_extensions = state
        .app
        .db
        .load_app_settings()
        .await
        .map(|settings| settings.scan_extensions)
        .unwrap_or_else(|_| crate::models::settings::default_scan_extensions());
    Ok(Json(json!({
        "library_roots": roots,
        "exclude_roots": excludes,
        "scan_extensions": scan_extensions,
    })))
}

async fn set_server_settings(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(settings): Json<ServerSettings>,
) -> Result<Json<Value>, StatusCode> {
    require_csrf(&state, &headers)?;
    {
        let mut config = state.app.config.lock().await;
        config.library_roots = settings
            .library_roots
            .into_iter()
            .map(PathBuf::from)
            .collect();
        config.exclude_roots = settings
            .exclude_roots
            .into_iter()
            .map(PathBuf::from)
            .collect();
        config
            .save(&state.app.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    Ok(Json(json!({ "ok": true })))
}

async fn import_songs(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(songs): Json<Vec<crate::models::Song>>,
) -> Result<Json<Value>, StatusCode> {
    require_csrf(&state, &headers)?;
    let mut count = 0u64;
    for song in songs {
        state
            .app
            .db
            .upsert_song(&song)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        count += 1;
    }
    audit(&state, "import_songs", &count.to_string());
    Ok(Json(json!({ "imported": count })))
}

#[derive(Deserialize)]
struct TagRequest {
    song_id: String,
    namespace: String,
    value: String,
}

async fn add_tag(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<TagRequest>,
) -> Result<StatusCode, StatusCode> {
    require_csrf(&state, &headers)?;
    audit(&state, "tag_add", &format!("{}:{}:{} ", request.song_id, request.namespace, request.value));
    crate::features::tags::add_tag(
        &state.app.db,
        &SongId(request.song_id),
        &request.namespace,
        &request.value,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_tag(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<TagRequest>,
) -> Result<StatusCode, StatusCode> {
    require_csrf(&state, &headers)?;
    crate::features::tags::remove_tag(
        &state.app.db,
        &SongId(request.song_id),
        &request.namespace,
        &request.value,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct MetadataRequest {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    notes: Option<String>,
}

fn clean_override(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

async fn update_metadata(
    State(state): State<ServerState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(request): Json<MetadataRequest>,
) -> Result<Json<Value>, StatusCode> {
    require_csrf(&state, &headers)?;
    let mut song = state
        .app
        .db
        .get_song(&SongId(id))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    song.metadata.display_title = clean_override(request.title);
    song.metadata.artist_override = clean_override(request.artist);
    song.metadata.album_override = clean_override(request.album);
    song.metadata.notes = clean_override(request.notes);
    state
        .app
        .db
        .upsert_song(&song)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::to_value(public_song(song)).unwrap_or_default()))
}

#[derive(Deserialize)]
struct QueueRequest {
    ids: Vec<String>,
    start_index: isize,
}

async fn player_queue_set(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<QueueRequest>,
) -> Result<Json<Value>, StatusCode> {
    require_csrf(&state, &headers)?;
    let mut player = state.app.player.lock().await;
    let status = player
        .set_queue(request.ids, request.start_index)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sse::publish_status(&state.sse_tx, &status);
    Ok(Json(serde_json::to_value(status).unwrap_or_default()))
}

async fn player_queue_get(
    State(state): State<ServerState>,
) -> Json<Value> {
    let player = state.app.player.lock().await;
    let (ids, index) = player.queue_snapshot();
    Json(json!({ "ids": ids, "current_index": index }))
}

#[derive(Deserialize)]
struct SeekRequest {
    position_secs: f64,
}

async fn player_seek(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<SeekRequest>,
) -> Result<Json<Value>, StatusCode> {
    require_csrf(&state, &headers)?;
    let mut player = state.app.player.lock().await;
    player
        .seek(request.position_secs)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(serde_json::to_value(player.status()).unwrap_or_default()))
}

#[derive(Deserialize)]
struct VolumeRequest {
    volume: f64,
}

async fn player_volume(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<VolumeRequest>,
) -> Result<Json<Value>, StatusCode> {
    require_csrf(&state, &headers)?;
    let mut player = state.app.player.lock().await;
    player
        .set_volume(request.volume)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(serde_json::to_value(player.status()).unwrap_or_default()))
}

#[derive(Deserialize)]
struct MuteRequest {
    muted: bool,
}

async fn player_mute(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<MuteRequest>,
) -> Result<Json<Value>, StatusCode> {
    require_csrf(&state, &headers)?;
    let mut player = state.app.player.lock().await;
    player
        .set_mute(request.muted)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(serde_json::to_value(player.status()).unwrap_or_default()))
}

#[derive(Deserialize)]
struct SpeedRequest {
    speed: f64,
}

async fn player_speed(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<SpeedRequest>,
) -> Result<Json<Value>, StatusCode> {
    require_csrf(&state, &headers)?;
    let mut player = state.app.player.lock().await;
    player
        .set_speed(request.speed)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(serde_json::to_value(player.status()).unwrap_or_default()))
}

async fn player_toggle(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    require_csrf(&state, &headers)?;
    let mut player = state.app.player.lock().await;
    // Refresh the end-of-track state before toggling so a hidden/stale client
    // does not turn "pause" into a restart of an already-finished mpv process.
    player.check_alive().await;
    player.refresh().await;
    let status = player
        .toggle_pause()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sse::publish_status(&state.sse_tx, &status);
    Ok(Json(serde_json::to_value(status).unwrap_or_default()))
}

async fn lyrics(
    State(state): State<ServerState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Value>, StatusCode> {
    let mut song = state
        .app
        .db
        .get_song(&SongId(id))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if let Some(lyrics) = &song.metadata.lyrics {
        if !lyrics.is_empty() {
            return Ok(Json(json!(lyrics)));
        }
    }

    if let Some(path) = song.paths.iter().find(|path| path.exists()) {
        if let Ok(file_meta) = crate::features::metadata::extract_file_metadata(path) {
            if let Some(lyrics) = file_meta.lyrics {
                if !lyrics.is_empty() {
                    song.metadata.lyrics = Some(lyrics.clone());
                    let _ = state.app.db.upsert_song(&song).await;
                    return Ok(Json(json!(lyrics)));
                }
            }
        }
    }

    if let Some(plugins) = state.app.plugins.as_ref() {
        if let Some(source) = song.sources.first() {
            if let Ok(Some(lyrics)) = plugins.lyrics(&source.platform, &source.id).await {
                if !lyrics.text.is_empty() {
                    song.metadata.lyrics = Some(lyrics.text.clone());
                    let _ = state.app.db.upsert_song(&song).await;
                    return Ok(Json(json!(lyrics.text)));
                }
            }
        }
    }

    Ok(Json(Value::Null))
}

fn build_router(state: ServerState) -> Router {
    Router::new()
        .route("/api/v1/status", get(status))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/me", get(me))
        .route("/api/v1/songs", get(list_songs))
        .route("/api/v1/songs/:id", get(get_song))
        .route("/api/v1/server-settings", get(get_server_settings).post(set_server_settings))
        .route("/api/v1/library/scan", post(scan_library))
        .route("/api/v1/cache/stats", get(cache_stats))
        .route("/api/v1/cache/clear", post(clear_cache))
        .route("/api/v1/cache/song/:id", post(clear_song_cache))
        .route("/api/v1/import/songs", post(import_songs))
        .route("/api/v1/songs/:id/tags", get(song_tags))
        .route("/api/v1/songs/:id/metadata", post(update_metadata))
        .route("/api/v1/tags", post(add_tag))
        .route("/api/v1/tags/remove", post(remove_tag))
        .route("/api/v1/tag-history", get(tag_history))
        .route("/api/v1/namespaces", get(namespaces))
        .route("/api/v1/meta-tags", get(meta_tags))
        .route("/api/v1/tag-values", get(tag_values))
        .route("/api/v1/search", get(search))
        .route("/api/v1/plugins/state", get(plugin_state_public))
        .route("/api/v1/plugins/reload", post(reload_plugins))
        .route("/api/v1/plugins/enabled", post(set_plugin_enabled))
        .route("/api/v1/plugins/argv", post(set_plugin_argv))
        .route("/api/v1/plugins/platform-argv", post(set_plugin_platform_argv))
        .route("/api/v1/plugins/route-argv", post(set_plugin_route_argv))
        .route("/api/v1/plugins/concurrency", post(set_plugin_concurrency))
        .route("/api/v1/plugins/route", post(set_plugin_route))
        .route("/api/v1/search/online", post(search_online))
        .route("/api/v1/resolve", post(resolve_song))
        .route("/api/v1/player/play-online", post(play_online_results))
        .route("/api/v1/media/:id", get(media))
        .route("/api/v1/download/:id", get(download_song))
        .route("/api/v1/image", get(image_proxy))
        .route("/api/v1/player/status", get(player_status))
        .route("/api/v1/player/mode", post(set_play_mode))
        .route("/api/v1/player/stream", get(player_stream))
        .route("/api/v1/player/play", post(player_play))
        .route("/api/v1/player/queue", get(player_queue_get).post(player_queue_set))
        .route("/api/v1/player/stop", post(player_stop))
        .route("/api/v1/player/next", post(player_next))
        .route("/api/v1/player/prev", post(player_prev))
        .route("/api/v1/player/toggle", post(player_toggle))
        .route("/api/v1/player/seek", post(player_seek))
        .route("/api/v1/player/volume", post(player_volume))
        .route("/api/v1/player/mute", post(player_mute))
        .route("/api/v1/player/speed", post(player_speed))
        .route("/api/v1/lyrics/:id", get(lyrics))
        .fallback_service(
            ServeDir::new(&state.web_dir)
                .fallback(ServeFile::new(state.web_dir.join("index.html"))),
        )
        .with_state(state)
        .layer(axum::extract::DefaultBodyLimit::max(16 * 1024 * 1024))
        .layer(cors_layer())
}

fn cors_layer() -> CorsLayer {
    let origins = [
        "tauri://localhost",
        "http://tauri.localhost",
        "https://tauri.localhost",
        "http://localhost:1420",
        "http://127.0.0.1:1420",
    ]
    .iter()
    .filter_map(|origin| origin.parse::<HeaderValue>().ok())
    .collect::<Vec<_>>();
    CorsLayer::new()
        .allow_origin(origins)
        .allow_credentials(true)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            HeaderName::from_static("x-csrf-token"),
        ])
}

fn default_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("vide")
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|arg| arg == name).and_then(|index| args.get(index + 1).cloned())
}

fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|arg| arg == name)
}

pub async fn run_from_args(args: Vec<String>) -> Result<(), String> {
    let data_dir = arg_value(&args, "--data-dir")
        .map(PathBuf::from)
        .unwrap_or_else(default_data_dir);
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("create data dir: {e}"))?;
    let db_path = data_dir.join("vide.db");

    if let Some(password) = arg_value(&args, "--set-password") {
        let db = Database::open(&db_path).await.map_err(|e| e.to_string())?;
        let hash = hash_password(&password)?;
        db.update_setting("server_password", &Value::String(hash))
            .await
            .map_err(|e| e.to_string())?;
        println!("password set for {}", db_path.display());
        return Ok(());
    }

    let app = Arc::new(AppState::initialize(&db_path).await);
    let auth = Arc::new(AuthState::new(app.db.clone()).await);
    if auth.password_hash.lock().map(|value| value.is_none()).unwrap_or(true) {
        if let Ok(password) = std::env::var("VIDE_ADMIN_PASSWORD") {
            let hash = hash_password(&password)?;
            app.db
                .update_setting("server_password", &Value::String(hash.clone()))
                .await
                .map_err(|e| e.to_string())?;
            *auth.password_hash.lock().unwrap() = Some(hash);
        } else {
            eprintln!("warning: no server password configured; run with --set-password <password>");
        }
    }

    let bind = arg_value(&args, "--bind").unwrap_or_else(|| "[::]:8080".to_string());
    let addr: SocketAddr = bind.parse().map_err(|e| format!("invalid --bind: {e}"))?;
    let public_media = flag(&args, "--public-media");
    let web_dir = arg_value(&args, "--web-dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../build"));
    let tls_cert = arg_value(&args, "--tls-cert");
    let tls_key = arg_value(&args, "--tls-key");
    if tls_cert.is_some() != tls_key.is_some() {
        return Err("--tls-cert and --tls-key must be used together".to_string());
    }
    let secure_cookies = tls_cert.is_some();
    let (sse_tx, _) = crate::features::network::sse::new_channel();
    let audit_path = data_dir.join("audit.log");
    let tick_player = app.player.clone();
    let tick_tx = sse_tx.clone();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        loop {
            ticker.tick().await;
            let mut player = tick_player.lock().await;
            player.check_alive().await;
            player.refresh().await;
            let status = player.status();
            if status.state != crate::features::player::PlaybackState::Stopped {
                sse::publish_status(&tick_tx, &status);
            }
        }
    });
    let state = ServerState {
        app,
        auth,
        public_media,
        data_dir,
        web_dir,
        audit_path,
        secure_cookies,
        sse_tx,
    };
    let router = build_router(state);
    if let (Some(cert), Some(key)) = (tls_cert, tls_key) {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let config = RustlsConfig::from_pem_file(cert, key)
            .await
            .map_err(|e| format!("load TLS certificate: {e}"))?;
        println!("Vide server listening on https://{addr} (public_media={public_media})");
        axum_server::bind_rustls(addr, config)
            .serve(router.into_make_service())
            .await
            .map_err(|e| e.to_string())
    } else {
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| format!("bind {addr}: {e}"))?;
        println!("Vide server listening on http://{addr} (public_media={public_media})");
        axum::serve(listener, router)
            .await
            .map_err(|e| e.to_string())
    }
}

/// CLI entry point for password setup.
pub async fn set_password(db_path: &Path, password: &str) -> Result<(), String> {
    let db = Database::open(db_path).await.map_err(|e| e.to_string())?;
    let hash = hash_password(password)?;
    db.update_setting("server_password", &Value::String(hash))
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
