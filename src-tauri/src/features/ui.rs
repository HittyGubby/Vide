use crate::app::AppState;
use crate::features::library::ScanResult;
use crate::features::player::PlaybackStatus;
use crate::features::plugin::types::SearchResultItem;
use crate::features::search::fuzzy::FuzzyMatch;
use crate::infra::hash::song_id_from_source;
use crate::models::song::{Song, SongId, SongMetadata, Source};
use crate::models::tag::{MetaTag, Tag};

// ── Library ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_songs(state: tauri::State<'_, AppState>) -> Result<Vec<Song>, String> {
    state
        .db
        .query_songs(&crate::infra::db::QueryFilter::default())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_library_songs(state: tauri::State<'_, AppState>) -> Result<Vec<Song>, String> {
    state
        .db
        .list_library_songs()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_library_roots(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let config = state.config.lock().await;
    Ok(config
        .library_roots
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

#[tauri::command]
pub async fn add_library_root(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    let mut config = state.config.lock().await;
    let p = std::path::PathBuf::from(&path);
    if !config.library_roots.contains(&p) {
        config.library_roots.push(p);
        config.save(&*state.db).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_song(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<Option<Song>, String> {
    state
        .db
        .get_song(&SongId(id))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_songs(
    state: tauri::State<'_, AppState>,
    query: String,
) -> Result<Vec<Song>, String> {
    crate::features::search::search(&*state.db, &query).await
}

#[tauri::command]
pub async fn scan_library(state: tauri::State<'_, AppState>) -> Result<ScanResult, String> {
    let config = state.config.lock().await;
    let mut total = ScanResult::default();

    for root in &config.library_roots {
        let result = crate::features::library::scan_root(&*state.db, root).await;
        total.scanned += result.scanned;
        total.added += result.added;
        total.updated += result.updated;
        total.skipped += result.skipped;
        total.errors.extend(result.errors);
    }

    Ok(total)
}

// ── Playback ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_player_status(state: tauri::State<'_, AppState>) -> Result<PlaybackStatus, String> {
    let player = state.player.lock().await;
    Ok(player.status())
}

#[tauri::command]
pub async fn get_lyrics(
    state: tauri::State<'_, AppState>,
    song_id: String,
) -> Result<Option<String>, String> {
    let song = state
        .db
        .get_song(&SongId(song_id))
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "song not found".to_string())?;

    // Check stored lyrics first
    if let Some(lyrics) = &song.metadata.lyrics {
        if !lyrics.is_empty() {
            return Ok(Some(lyrics.clone()));
        }
    }

    // Plugin lyrics capability (future use)
    // if let Some(plugins) = state.plugins.as_ref() {
    //     if let Some(source) = song.sources.first() {
    //         ...
    //     }
    // }

    Ok(None)
}

#[tauri::command]
pub async fn play_song(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<PlaybackStatus, String> {
    let song = state
        .db
        .get_song(&SongId(id.clone()))
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "song not found".to_string())?;

    let descriptor = if let Some(path) = song.paths.first() {
        // Local file — play directly
        crate::features::plugin::types::StreamDescriptor::UrlStream {
            id,
            url: path.to_string_lossy().to_string(),
            headers: None,
        }
    } else if let Some(source) = song.sources.first() {
        // Online source — resolve stream via plugin
        let plugins = state
            .plugins
            .as_ref()
            .ok_or_else(|| "plugin system not available".to_string())?;
        plugins.stream(&source.platform, &source.id, &id).await?
    } else {
        return Err("no playable source found".to_string());
    };

    let mut player = state.player.lock().await;
    player.play(&descriptor).await
}

#[tauri::command]
pub async fn stop_playback(state: tauri::State<'_, AppState>) -> Result<PlaybackStatus, String> {
    let mut player = state.player.lock().await;
    Ok(player.stop().await)
}

#[tauri::command]
pub async fn seek(
    state: tauri::State<'_, AppState>,
    position_secs: f64,
) -> Result<PlaybackStatus, String> {
    let mut player = state.player.lock().await;
    player.seek(position_secs).await?;
    Ok(player.status())
}

// ── Online Plugin Search ──────────────────────────────────────────

#[tauri::command]
pub async fn search_online(
    state: tauri::State<'_, AppState>,
    query: String,
    platform: Option<String>,
) -> Result<Vec<SearchResultItem>, String> {
    let plugins = state
        .plugins
        .as_ref()
        .ok_or_else(|| "plugin system not available".to_string())?;

    if let Some(platform) = platform {
        plugins.search(&platform, &query).await
    } else {
        // Search all platforms that have a registered search provider
        let mut all_results = Vec::new();
        for manifest in &plugins.manifests {
            for cap in &manifest.capabilities {
                if cap.capability == "search" {
                    if let Ok(mut results) = plugins.search(&cap.platform, &query).await {
                        all_results.append(&mut results);
                    }
                }
            }
        }
        Ok(all_results)
    }
}

/// Resolve an online song by platform + id, creating a Song in the DB if
/// it does not already exist.
#[tauri::command]
pub async fn resolve_song(
    state: tauri::State<'_, AppState>,
    platform: String,
    id: String,
) -> Result<Song, String> {
    let song_id = song_id_from_source(&platform, &id);

    // Return existing song if already in DB
    if let Some(song) = state
        .db
        .get_song(&song_id)
        .await
        .map_err(|e| e.to_string())?
    {
        return Ok(song);
    }

    // Resolve metadata via plugin
    let plugins = state
        .plugins
        .as_ref()
        .ok_or_else(|| "plugin system not available".to_string())?;
    let item = plugins.resolve(&platform, &id).await?;

    let title = item.title.clone();
    let artist = item.artist.clone().unwrap_or_else(|| "Unknown".to_string());
    let album = item.album.clone().unwrap_or_else(|| "Unknown".to_string());

    let song = Song {
        id: song_id,
        title,
        artist,
        album,
        tags: Vec::new(),
        sources: vec![Source {
            platform,
            id,
        }],
        paths: Vec::new(),
        metadata: SongMetadata {
            duration: item.duration,
            ..Default::default()
        },
    };

    state
        .db
        .upsert_song(&song)
        .await
        .map_err(|e| e.to_string())?;

    Ok(song)
}

// ── Search ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn fuzzy_search_songs(
    state: tauri::State<'_, AppState>,
    query: String,
) -> Result<Vec<FuzzyMatch>, String> {
    crate::features::search::fuzzy_search_songs(&*state.db, &query)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn autocomplete(
    state: tauri::State<'_, AppState>,
    prefix: String,
) -> Result<Vec<String>, String> {
    crate::features::search::autocomplete(&*state.db, &prefix, 10)
        .await
        .map_err(|e| e.to_string())
}

// ── Tags ──────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_meta_tags(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<MetaTag>, String> {
    crate::features::tags::get_meta_tags(&*state.db)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_tag(
    state: tauri::State<'_, AppState>,
    song_id: String,
    namespace: String,
    value: String,
) -> Result<(), String> {
    crate::features::tags::add_tag(&*state.db, &SongId(song_id), &namespace, &value)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_tag(
    state: tauri::State<'_, AppState>,
    song_id: String,
    namespace: String,
    value: String,
) -> Result<(), String> {
    crate::features::tags::remove_tag(&*state.db, &SongId(song_id), &namespace, &value)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_song_tags(
    state: tauri::State<'_, AppState>,
    song_id: String,
) -> Result<Vec<Tag>, String> {
    crate::features::tags::get_song_tags(&*state.db, &SongId(song_id))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_namespaces(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    crate::features::tags::get_namespaces(&*state.db)
        .await
        .map_err(|e| e.to_string())
}

// ── Playlists ─────────────────────────────────────────────────────

#[tauri::command]
pub async fn import_m3u(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<crate::models::playlist::Playlist, String> {
    crate::features::playlist::import_m3u(&*state.db, std::path::Path::new(&path))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_playlist(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<Option<crate::models::playlist::Playlist>, String> {
    state
        .db
        .get_playlist(&crate::models::playlist::PlaylistId(id))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_playlists(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::models::playlist::Playlist>, String> {
    state
        .db
        .list_playlists()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_playlist(
    state: tauri::State<'_, AppState>,
    playlist_id: String,
    output_path: String,
) -> Result<(), String> {
    crate::features::playlist::export_playlist(
        &*state.db,
        &crate::models::playlist::PlaylistId(playlist_id),
        std::path::Path::new(&output_path),
    )
    .await
    .map_err(|e| e.to_string())
}

// ── Server ────────────────────────────────────────────────────────

#[tauri::command]
pub async fn start_server(state: tauri::State<'_, AppState>, addr: String) -> Result<(), String> {
    let socket_addr: std::net::SocketAddr = addr
        .parse()
        .map_err(|e| format!("invalid address: {e}"))?;

    let mut network = state.network.lock().await;
    network
        .start(socket_addr, state.db.clone(), state.player.clone())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_server(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut network = state.network.lock().await;
    network.stop().await;
    Ok(())
}

#[tauri::command]
pub async fn get_server_status(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    Ok(state.network.lock().await.is_running())
}
