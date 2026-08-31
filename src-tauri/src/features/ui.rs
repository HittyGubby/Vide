use crate::app::AppState;
use crate::features::library::ScanResult;
use crate::features::player::PlaybackStatus;
use crate::features::plugin::types::SearchResultItem;
use crate::features::search::fuzzy::FuzzyMatch;
use crate::infra::hash::song_id_from_source;
use crate::models::song::{Song, SongId, SongMetadata, Source};
use crate::models::tag::{MetaTag, Tag};
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tracing::warn;

// ── Library ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_songs(state: tauri::State<'_, AppState>) -> Result<Vec<Song>, String> {
    state
        .db
        .query_songs(&crate::infra::db::QueryFilter::default())
        .await
        .map_err(|e| e.to_string())
}

/// Import a batch of songs (upsert). Returns the number imported.
#[tauri::command]
pub async fn import_songs(
    state: tauri::State<'_, AppState>,
    songs: Vec<Song>,
) -> Result<u64, String> {
    let mut count = 0u64;
    for song in &songs {
        state
            .db
            .upsert_song(song)
            .await
            .map_err(|e| e.to_string())?;
        count += 1;
    }
    Ok(count)
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

/// Remove an included library root by path.
#[tauri::command]
pub async fn remove_library_root(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    let mut config = state.config.lock().await;
    let p = std::path::PathBuf::from(&path);
    config.library_roots.retain(|r| r != &p);
    config.save(&*state.db).await.map_err(|e| e.to_string())
}

/// Get the excluded folder list.
#[tauri::command]
pub async fn get_exclude_roots(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let config = state.config.lock().await;
    Ok(config
        .exclude_roots
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

/// Add an excluded folder.
#[tauri::command]
pub async fn add_exclude_root(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    let mut config = state.config.lock().await;
    let p = std::path::PathBuf::from(&path);
    if !config.exclude_roots.contains(&p) {
        config.exclude_roots.push(p);
        config.save(&*state.db).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Remove an excluded folder by path.
#[tauri::command]
pub async fn remove_exclude_root(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    let mut config = state.config.lock().await;
    let p = std::path::PathBuf::from(&path);
    config.exclude_roots.retain(|r| r != &p);
    config.save(&*state.db).await.map_err(|e| e.to_string())
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

/// Clear (delete) the cached media for a song, forcing a fresh fetch on
/// the next playback. Returns the number of cache entries removed.
#[tauri::command]
pub async fn clear_song_cache(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<u64, String> {
    let song_id = SongId(id.clone());
    let entries = state
        .db
        .get_cache_entries(&song_id)
        .await
        .map_err(|e| e.to_string())?;

    if let Some(entry) = entries.first() {
        // Remove the on-disk file(s) too.
        let _ = std::fs::remove_file(&entry.path);
        let _ = std::fs::remove_file(entry.path.with_extension("tmp"));
    }

    state
        .db
        .delete_cache_entry(&song_id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(entries.len() as u64)
}

/// Download an online song's stream into the local cache and register the
/// cached file as a local path for the song, so it becomes playable offline
/// (i.e. it joins the local library). Returns the number of paths now on the
/// song (0 for already-local songs, >=1 after a successful download).
#[tauri::command]
pub async fn download_song(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<u64, String> {
    let song_id = SongId(id.clone());
    let song = state
        .db
        .get_song(&song_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "song not found".to_string())?;

    // Already a local file — nothing to download.
    if !song.paths.is_empty() {
        return Ok(song.paths.len() as u64);
    }

    // Need a plugin stream descriptor to know what to fetch.
    let (platform, source_id) = song
        .sources
        .first()
        .map(|s| (s.platform.clone(), s.id.clone()))
        .ok_or_else(|| "no online source to download".to_string())?;

    let plugins = state
        .plugins
        .as_ref()
        .ok_or_else(|| "plugin system not available".to_string())?;
    let descriptor = plugins.stream(&platform, &source_id, &id).await?;

    // Build a CacheManager rooted in the app data directory.
    let data_dir = dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let basic_dir = data_dir.join("vide");
    let cache = crate::features::cache::CacheManager::new(
        state.db.as_ref().clone(),
        &basic_dir,
    )
    .await
    .map_err(|e| format!("cache init: {e}"))?;

    let cached_path = cache
        .download_from_descriptor(&song_id, &descriptor)
        .await
        .map_err(|e| format!("download: {e}"))?;

    // Register the cached file as a local path (dedupe).
    let mut song = song;
    song.paths.push(cached_path);
    state
        .db
        .upsert_song(&song)
        .await
        .map_err(|e| format!("save song: {e}"))?;

    Ok(song.paths.len() as u64)
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

    // Full re-scan semantics: replace the previous local result set instead of
    // merging with stale entries, so songs that no longer exist are dropped.
    let cleared = state
        .db
        .clear_local_songs()
        .await
        .map_err(|e| e.to_string())?;
    total.skipped += cleared;

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
    let mut player = state.player.lock().await;
    // Detect a natural end (mpv exited on its own) so the frontend can
    // advance the queue, then pull duration/progress so the UI shows real
    // track length instead of 0:00.
    player.check_alive().await;
    player.refresh().await;
    let mut status = player.status();

    // If nothing is actually loaded but we remember a last-played song, report
    // it as a paused/memory state so the UI keeps showing where the user left
    // off instead of being reset to idle by the frontend poll.
    if status.media_id.is_none() {
        if let Ok(Some((song_id, position))) = state.db.load_last_played().await {
            if !song_id.is_empty() {
                status.media_id = Some(song_id.clone());
                status.elapsed_secs = Some(position);
                // Also report the known duration so the UI isn't stuck at 0:00.
                if let Ok(Some(song)) = state.db.get_song(&SongId(song_id)).await {
                    if let Some(d) = song.metadata.duration {
                        status.duration_secs = Some(d);
                    }
                }
            }
        }
    }

    Ok(status)
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

    // For a local file, re-read the embedded lyrics tag (e.g. ID3 USLT),
    // cache it on the song, and return it.
    if let Some(path) = song.paths.first() {
        if let Ok(file_meta) = crate::features::metadata::extract_file_metadata(path) {
            if let Some(lyrics) = &file_meta.lyrics {
                if !lyrics.is_empty() {
                    let mut song = song;
                    song.metadata.lyrics = Some(lyrics.clone());
                    let _ = state.db.upsert_song(&song).await;
                    return Ok(Some(lyrics.clone()));
                }
            }
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
    position_secs: Option<f64>,
) -> Result<PlaybackStatus, String> {
    let mut player = state.player.lock().await;
    player.play_song_id(&id, position_secs).await
}

/// Set the playback queue and optionally start playing at `start_index`
/// (index of the song to play, or -1 to just store the queue).
#[tauri::command]
pub async fn set_play_queue(
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
    start_index: i64,
) -> Result<PlaybackStatus, String> {
    let mut player = state.player.lock().await;
    player.set_queue(ids, start_index as isize).await
}

/// Play the next song in the queue (as decided by Rust).
#[tauri::command]
pub async fn play_next(
    state: tauri::State<'_, AppState>,
) -> Result<PlaybackStatus, String> {
    let mut player = state.player.lock().await;
    player.play_next().await
}

/// Play the previous song in the queue.
#[tauri::command]
pub async fn play_prev(
    state: tauri::State<'_, AppState>,
) -> Result<PlaybackStatus, String> {
    let mut player = state.player.lock().await;
    player.play_prev().await
}

/// Set the play order mode (single / reverse / shuffle / loop).
#[tauri::command]
pub async fn set_play_mode(
    state: tauri::State<'_, AppState>,
    mode: crate::features::player::PlayMode,
) -> Result<PlaybackStatus, String> {
    let mut player = state.player.lock().await;
    player.set_play_mode(mode);
    Ok(player.status())
}

#[tauri::command]
pub async fn stop_playback(state: tauri::State<'_, AppState>) -> Result<PlaybackStatus, String> {
    let mut player = state.player.lock().await;
    Ok(player.stop().await)
}

/// Toggle between playing and paused. Unlike stop, this keeps the current
/// song loaded so the UI (title/artist/detail page) is NOT reset.
#[tauri::command]
pub async fn toggle_pause(state: tauri::State<'_, AppState>) -> Result<PlaybackStatus, String> {
    let mut player = state.player.lock().await;
    player.toggle_pause().await
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

/// Set playback speed (rate). 1.0 = normal.
#[tauri::command]
pub async fn set_speed(
    state: tauri::State<'_, AppState>,
    speed: f64,
) -> Result<PlaybackStatus, String> {
    let mut player = state.player.lock().await;
    player.set_speed(speed).await?;
    Ok(player.status())
}

/// Set playback volume (0..=100).
#[tauri::command]
pub async fn set_volume(
    state: tauri::State<'_, AppState>,
    volume: f64,
) -> Result<PlaybackStatus, String> {
    let mut player = state.player.lock().await;
    player.set_volume(volume).await?;
    Ok(player.status())
}

/// Mute / unmute playback.
#[tauri::command]
pub async fn set_mute(
    state: tauri::State<'_, AppState>,
    muted: bool,
) -> Result<PlaybackStatus, String> {
    let mut player = state.player.lock().await;
    player.set_mute(muted).await?;
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

/// Import an online playlist URL (YouTube/Bilibili/etc.) into the library.
///
/// Uses the plugin system to resolve the playlist entries, upserts each entry
/// as a Song in the library, and stores an ordered Playlist referencing them.
/// Returns the created playlist.
#[tauri::command]
pub async fn import_playlist_url(
    state: tauri::State<'_, AppState>,
    url: String,
) -> Result<crate::models::playlist::Playlist, String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let plugins = state
        .plugins
        .as_ref()
        .ok_or_else(|| "plugin system not available".to_string())?;
    let items = plugins.playlist(&url).await?;
    if items.is_empty() {
        return Err("playlist is empty or not resolvable".to_string());
    }

    let mut entries: Vec<crate::models::playlist::PlaylistEntry> = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        let song_id = crate::infra::hash::song_id_from_source(&item.platform, &item.id);

        // Upsert the song if not already present.
        if state
            .db
            .get_song(&song_id)
            .await
            .map_err(|e| e.to_string())?
            .is_none()
        {
            let title = item.title.clone();
            let artist = item.artist.clone().unwrap_or_else(|| "Unknown".to_string());
            let album = item.album.clone().unwrap_or_else(|| "Unknown".to_string());
            let song = Song {
                id: song_id.clone(),
                title,
                artist,
                album,
                tags: Vec::new(),
                sources: vec![Source {
                    platform: item.platform.clone(),
                    id: item.id.clone(),
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
        }

        entries.push(crate::models::playlist::PlaylistEntry {
            song_id,
            import_order: Some((i + 1) as u32),
        });
    }

    // Deterministic playlist id from the URL.
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let name = format!("在线导入 {:x}", hasher.finish()).to_string();
    let playlist = crate::models::playlist::Playlist {
        id: crate::models::playlist::PlaylistId(format!("online:{:x}", hasher.finish())),
        name,
        entries,
        source: crate::models::playlist::PlaylistSource::Online { url },
    };

    state
        .db
        .upsert_playlist(&playlist)
        .await
        .map_err(|e| e.to_string())?;

    Ok(playlist)
}

// ── Search Presets ───────────────────────────────────────────────

/// List all saved search presets.
#[tauri::command]
pub async fn list_search_presets(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::models::SearchPreset>, String> {
    state
        .db
        .list_search_presets()
        .await
        .map_err(|e| e.to_string())
}

/// Create or update a search preset. Returns the saved preset.
#[tauri::command]
pub async fn save_search_preset(
    state: tauri::State<'_, AppState>,
    name: String,
    query: String,
) -> Result<crate::models::SearchPreset, String> {
    let mut presets = state
        .db
        .list_search_presets()
        .await
        .map_err(|e| e.to_string())?;

    // Reuse the existing preset with the same name, else append a new one.
    if let Some(existing) = presets.iter_mut().find(|p| p.name == name) {
        existing.query = query;
        let saved = existing.clone();
        state
            .db
            .save_search_presets(&presets)
            .await
            .map_err(|e| e.to_string())?;
        return Ok(saved);
    }

    let preset = crate::models::SearchPreset {
        id: format!("preset_{}", presets.len() + 1),
        name,
        query,
    };
    presets.push(preset.clone());
    state
        .db
        .save_search_presets(&presets)
        .await
        .map_err(|e| e.to_string())?;
    Ok(preset)
}

/// Delete a search preset by id.
#[tauri::command]
pub async fn delete_search_preset(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let mut presets = state
        .db
        .list_search_presets()
        .await
        .map_err(|e| e.to_string())?;
    presets.retain(|p| p.id != id);
    state
        .db
        .save_search_presets(&presets)
        .await
        .map_err(|e| e.to_string())
}

/// Get the ID of the last-selected search preset.
#[tauri::command]
pub async fn get_last_search_preset(
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    state
        .db
        .get_last_search_preset()
        .await
        .map_err(|e| e.to_string())
}

/// Persist the last-selected search preset ID.
#[tauri::command]
pub async fn set_last_search_preset(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    state
        .db
        .set_last_search_preset(&id)
        .await
        .map_err(|e| e.to_string())
}

// ── App Settings ──────────────────────────────────────────────────

/// Get the full app settings object.
#[tauri::command]
pub async fn get_app_settings(
    state: tauri::State<'_, AppState>,
) -> Result<crate::models::AppSettings, String> {
    state
        .db
        .load_app_settings()
        .await
        .map_err(|e| e.to_string())
}

/// Save the full app settings object.
#[tauri::command]
pub async fn set_app_settings(
    state: tauri::State<'_, AppState>,
    settings: crate::models::AppSettings,
) -> Result<(), String> {
    state
        .db
        .save_app_settings(&settings)
        .await
        .map_err(|e| e.to_string())
}

/// Get the last-played song id + position snapshot.
#[tauri::command]
pub async fn get_last_played(
    state: tauri::State<'_, AppState>,
) -> Result<Option<(String, f64)>, String> {
    state
        .db
        .load_last_played()
        .await
        .map_err(|e| e.to_string())
}

// ── Caches ─────────────────────────────────────────────

/// One row of the cache manager list.
#[derive(Debug, Clone, Serialize)]
pub struct CacheStat {
    /// Stable id used by `clear_cache_kind`.
    pub kind: String,
    pub label: String,
    /// Number of cached items.
    pub count: u64,
    /// Bytes used on disk. `None` for caches that live as text inside SQLite.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

fn image_cache_dir() -> PathBuf {
    crate::app_data_dir().join("cache").join("images")
}

fn audio_cache_dir() -> PathBuf {
    crate::app_data_dir().join("cache")
}

/// Count the files directly inside `dir` and their total size.
async fn dir_stats(dir: &Path) -> (u64, u64) {
    let mut count = 0u64;
    let mut size = 0u64;
    if let Ok(mut rd) = tokio::fs::read_dir(dir).await {
        while let Ok(Some(entry)) = rd.next_entry().await {
            if let Ok(meta) = entry.metadata().await {
                if meta.is_file() {
                    count += 1;
                    size += meta.len();
                }
            }
        }
    }
    (count, size)
}

/// Does this metadata blob carry a non-empty lyrics field?
fn has_lyrics(json: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .as_ref()
        .and_then(|v| v.get("lyrics"))
        .and_then(|v| v.as_str())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}

/// Does this metadata blob carry any saved field at all?
fn has_metadata(json: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .as_ref()
        .and_then(|v| v.as_object())
        .map(|o| o.values().any(|v| !v.is_null()))
        .unwrap_or(false)
}

/// Size and usage of every cache the app keeps.
#[tauri::command]
pub async fn get_cache_stats(state: tauri::State<'_, AppState>) -> Result<Vec<CacheStat>, String> {
    let (image_count, image_size) = dir_stats(&image_cache_dir()).await;

    let entries = state
        .db
        .list_cache_entries()
        .await
        .map_err(|e| e.to_string())?;
    // Report what is really on disk; a recorded size can be stale if the
    // file was removed behind our back.
    let mut audio_size = 0u64;
    let mut audio_count = 0u64;
    for entry in &entries {
        if let Ok(meta) = tokio::fs::metadata(&entry.path).await {
            audio_count += 1;
            audio_size += meta.len();
        }
    }

    let blobs = state
        .db
        .list_metadata_json()
        .await
        .map_err(|e| e.to_string())?;
    let lyric_count = blobs.iter().filter(|b| has_lyrics(b)).count() as u64;
    let metadata_count = blobs.iter().filter(|b| has_metadata(b)).count() as u64;

    Ok(vec![
        CacheStat {
            kind: "image".to_string(),
            label: "图片缓存".to_string(),
            count: image_count,
            size: Some(image_size),
        },
        CacheStat {
            kind: "audio".to_string(),
            label: "音频缓存".to_string(),
            count: audio_count,
            size: Some(audio_size),
        },
        CacheStat {
            kind: "lyric".to_string(),
            label: "歌词缓存".to_string(),
            count: lyric_count,
            size: None,
        },
        CacheStat {
            kind: "metadata".to_string(),
            label: "元信息缓存".to_string(),
            count: metadata_count,
            size: None,
        },
    ])
}

/// Empty one cache. Returns the number of items that were removed.
#[tauri::command]
pub async fn clear_cache_kind(
    state: tauri::State<'_, AppState>,
    kind: String,
) -> Result<u64, String> {
    match kind.as_str() {
        "image" => {
            let dir = image_cache_dir();
            let (count, _) = dir_stats(&dir).await;
            let mut removed = 0u64;
            if let Ok(mut rd) = tokio::fs::read_dir(&dir).await {
                while let Ok(Some(entry)) = rd.next_entry().await {
                    if tokio::fs::remove_file(entry.path()).await.is_ok() {
                        removed += 1;
                    }
                }
            }
            Ok(removed.max(count))
        }
        "audio" => {
            let entries = state
                .db
                .list_cache_entries()
                .await
                .map_err(|e| e.to_string())?;
            let mut removed = 0u64;
            for entry in &entries {
                if tokio::fs::remove_file(&entry.path).await.is_ok() {
                    removed += 1;
                }
            }
            state
                .db
                .delete_all_cache_entries()
                .await
                .map_err(|e| e.to_string())?;
            Ok(removed)
        }
        "lyric" => {
            let songs = state
                .db
                .list_library_songs()
                .await
                .map_err(|e| e.to_string())?;
            let mut removed = 0u64;
            for song in songs {
                if song.metadata.lyrics.as_deref().map(str::is_empty) == Some(false) {
                    let mut updated = song.clone();
                    updated.metadata.lyrics = None;
                    state
                        .db
                        .upsert_song(&updated)
                        .await
                        .map_err(|e| e.to_string())?;
                    removed += 1;
                }
            }
            Ok(removed)
        }
        "metadata" => {
            let songs = state
                .db
                .list_library_songs()
                .await
                .map_err(|e| e.to_string())?;
            let mut removed = 0u64;
            for song in songs {
                let json = serde_json::to_string(&song.metadata).unwrap_or_default();
                if !has_metadata(&json) {
                    continue;
                }
                let mut updated = song.clone();
                updated.metadata = Default::default();
                state
                    .db
                    .upsert_song(&updated)
                    .await
                    .map_err(|e| e.to_string())?;
                removed += 1;
            }
            Ok(removed)
        }
        other => Err(format!("unknown cache kind: {other}")),
    }
}

/// One global shortcut to register: which action it triggers plus the
/// accelerator string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalShortcutItem {
    pub action: String,
    pub accelerator: String,
}

/// (Re)register the global shortcuts. Previously registered bindings are
/// dropped first, so this is safe to call whenever the settings change.
///
/// Returns the accelerators that could not be registered (badly formed, or
/// already taken by another application) — an empty list means all succeeded.
#[tauri::command]
pub fn set_global_shortcuts(
    app: tauri::AppHandle,
    items: Vec<GlobalShortcutItem>,
) -> Result<Vec<String>, String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let manager = app.global_shortcut();
    manager.unregister_all().map_err(|e| e.to_string())?;

    let mut failures = Vec::new();
    for item in items {
        let accelerator = item.accelerator.trim().to_string();
        if accelerator.is_empty() {
            continue;
        }
        let action = item.action.clone();
        match manager.on_shortcut(accelerator.as_str(), move |app, _shortcut, _event| {
            // The frontend owns the action itself; just announce which one.
            let _ = app.emit("shortcut", action.clone());
        }) {
            Ok(()) => {}
            Err(e) => failures.push(format!("{} ({})", accelerator, e)),
        }
    }

    if !failures.is_empty() {
        warn!("failed to register global shortcuts: {:?}", failures);
    }
    Ok(failures)
}

/// Persist the last-played song id + position.
#[tauri::command]
pub async fn save_last_played(
    state: tauri::State<'_, AppState>,
    song_id: String,
    position_secs: f64,
) -> Result<(), String> {
    state
        .db
        .save_last_played(&song_id, position_secs)
        .await
        .map_err(|e| e.to_string())
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
