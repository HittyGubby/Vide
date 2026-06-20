use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Sse},
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use tokio::sync::broadcast;
use tracing::error;

use crate::features::network::sse::{self, SseEvent, sse_stream};
use crate::features::player::PlayerManager;
use crate::features::plugin::types::StreamDescriptor;
use crate::infra::db::{Database, QueryFilter};

/// Shared state injected into all axum handlers.
#[derive(Clone)]
pub struct ApiState {
    pub db: Arc<Database>,
    pub player: Arc<tokio::sync::Mutex<PlayerManager>>,
    pub sse_tx: broadcast::Sender<SseEvent>,
}

/// Build the axum router with all API routes.
pub fn build_router(state: ApiState) -> Router {
    Router::new()
        .route("/api/v1/status", get(status_handler))
        .route("/api/v1/songs", get(list_songs_handler))
        .route("/api/v1/songs/{id}", get(get_song_handler))
        .route("/api/v1/songs/{id}/play", get(play_song_handler))
        .route("/api/v1/search", get(search_handler))
        .route("/api/v1/player/status", get(player_status_handler))
        .route("/api/v1/player/stream", get(player_sse_handler))
        .route("/api/v1/player/play", post(player_play_handler))
        .route("/api/v1/player/stop", post(player_stop_handler))
        .with_state(state)
}

// ── Handlers ──────────────────────────────────────────────────────

async fn status_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "name": "Vide",
        "version": "0.1.0",
        "api": "v1"
    }))
}

async fn list_songs_handler(
    State(state): State<ApiState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let songs = state
        .db
        .query_songs(&QueryFilter::default())
        .await
        .map_err(|e| {
            error!("list songs error: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(serde_json::json!(songs)))
}

async fn get_song_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let song = state
        .db
        .get_song(&crate::models::SongId(id))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(serde_json::to_value(&song).unwrap_or_default()))
}

/// Serve the audio file for a song.
/// Returns the first available local file.
async fn play_song_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let song = state
        .db
        .get_song(&crate::models::SongId(id))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let path = song.paths.first().ok_or(StatusCode::NOT_FOUND)?;
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(axum::response::Response::builder()
        .header("Content-Type", mime_guess::from_path(path).first_or_octet_stream().as_ref())
        .body(axum::body::Body::from_stream(
            tokio_util::io::ReaderStream::new(file),
        ))
        .unwrap())
}

#[derive(Deserialize)]
struct SearchParams {
    q: String,
}

async fn search_handler(
    State(state): State<ApiState>,
    Query(params): Query<SearchParams>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let songs = crate::features::search::search(&state.db, &params.q)
        .await
        .map_err(|e| {
            error!("search error: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(serde_json::json!(songs)))
}

async fn player_status_handler(
    State(state): State<ApiState>,
) -> Json<serde_json::Value> {
    let player = state.player.lock().await;
    Json(serde_json::to_value(player.status()).unwrap_or_default())
}

async fn player_sse_handler(
    State(state): State<ApiState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>> {
    let rx = state.sse_tx.subscribe();
    sse_stream(rx)
}

#[derive(Deserialize)]
struct PlayParams {
    id: String,
}

/// Start playback of a song on the server.
async fn player_play_handler(
    State(state): State<ApiState>,
    Json(params): Json<PlayParams>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let song = state
        .db
        .get_song(&crate::models::SongId(params.id.clone()))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Create a StreamDescriptor from the first available path or source
    let descriptor = if let Some(path) = song.paths.first() {
        StreamDescriptor::UrlStream {
            id: params.id.clone(),
            url: path.to_string_lossy().to_string(),
            headers: None,
        }
    } else if let Some(source) = song.sources.first() {
        // For online sources, use a placeholder URL
        StreamDescriptor::UrlStream {
            id: params.id.clone(),
            url: format!("https://{}/{}", source.platform, source.id),
            headers: None,
        }
    } else {
        return Err(StatusCode::NOT_FOUND);
    };

    let mut player = state.player.lock().await;
    let status = player.play(&descriptor).await.map_err(|e| {
        error!("play error: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Publish SSE event
    sse::publish_status(&state.sse_tx, &status);

    Ok(Json(serde_json::to_value(&status).unwrap_or_default()))
}

async fn player_stop_handler(
    State(state): State<ApiState>,
) -> Json<serde_json::Value> {
    let mut player = state.player.lock().await;
    let status = player.stop().await;

    sse::publish_status(&state.sse_tx, &status);

    Json(serde_json::to_value(&status).unwrap_or_default())
}
