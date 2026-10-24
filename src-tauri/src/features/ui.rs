use crate::app::AppState;
use crate::features::library::ScanResult;
use crate::features::player::PlaybackStatus;
use crate::features::plugin::types::{StreamDescriptor, Track};
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

fn visible_in_library(song: &Song, roots: &[std::path::PathBuf]) -> bool {
    if !song.tags.is_empty() {
        return true;
    }
    song.paths.iter().any(|path| {
        roots
            .iter()
            .any(|root| path == root || path.starts_with(root))
    })
}

#[tauri::command]
pub async fn list_library_songs(state: tauri::State<'_, AppState>) -> Result<Vec<Song>, String> {
    let roots = state.config.lock().await.library_roots.clone();
    let mut songs = state
        .db
        .list_library_songs()
        .await
        .map_err(|e| e.to_string())?;
    songs.retain(|song| visible_in_library(song, &roots));
    Ok(songs)
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

fn clean_override(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

/// Set or clear the user's display title / artist / album overrides and notes.
/// The raw scanned values stay intact; the list, player and search use the
/// overrides where present.
#[tauri::command]
pub async fn update_song_metadata(
    state: tauri::State<'_, AppState>,
    id: String,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    notes: Option<String>,
) -> Result<Song, String> {
    let mut song = state
        .db
        .get_song(&SongId(id))
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "song not found".to_string())?;

    song.metadata.display_title = clean_override(title);
    song.metadata.artist_override = clean_override(artist);
    song.metadata.album_override = clean_override(album);
    song.metadata.notes = clean_override(notes);

    state
        .db
        .upsert_song(&song)
        .await
        .map_err(|e| e.to_string())?;
    Ok(song)
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
fn detect_audio_extension(path: &std::path::Path) -> &'static str {
    use std::io::Read;
    let mut head = [0u8; 16];
    let read = std::fs::File::open(path)
        .and_then(|mut file| file.read(&mut head))
        .unwrap_or(0);
    let bytes = &head[..read];
    if bytes.starts_with(b"fLaC") {
        "flac"
    } else if bytes.starts_with(b"OggS") {
        "ogg"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE") {
        "wav"
    } else if bytes.get(4..8) == Some(b"ftyp") {
        "m4a"
    } else if bytes.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        "webm"
    } else {
        "mp3"
    }
}

fn safe_download_name(title: &str, artist: &str, extension: &str) -> String {
    let mut base = if artist.trim().is_empty() {
        title.trim().to_string()
    } else {
        format!("{} - {}", artist.trim(), title.trim())
    };
    if base.trim().is_empty() {
        base = "audio".to_string();
    }
    for bad in ['/', '\\', ':', '*', '?', '"', '<', '>', '|', '\0'] {
        base = base.replace(bad, "_");
    }
    let base = base.trim().trim_end_matches('.').to_string();
    format!("{}.{}", if base.is_empty() { "audio" } else { &base }, extension)
}

#[tauri::command]
pub fn get_default_download_dir() -> Result<String, String> {
    dirs::download_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join("Music")))
        .map(|dir| dir.to_string_lossy().to_string())
        .ok_or_else(|| "cannot determine a download directory".to_string())
}

#[tauri::command]
pub async fn download_remote_to(
    url: String,
    headers: Option<std::collections::HashMap<String, String>>,
    dest_dir: Option<String>,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let mut request = client.get(&url);
    if let Some(headers) = headers {
        for (key, value) in headers {
            request = request.header(key, value);
        }
    }
    let response = request.send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("download failed: {}", response.status()));
    }
    let disposition = response
        .headers()
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split("filename=\"").nth(1))
        .and_then(|value| value.split('"').next())
        .filter(|value| !value.is_empty())
        .unwrap_or("download.bin")
        .to_string();
    let directory = dest_dir
        .map(std::path::PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| dirs::download_dir())
        .or_else(|| dirs::home_dir().map(|home| home.join("Downloads")))
        .ok_or_else(|| "cannot determine a download directory".to_string())?;
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(|e| e.to_string())?;
    let mut destination = directory.join(&disposition);
    let stem = destination
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "download".to_string());
    let extension = destination
        .extension()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "bin".to_string());
    let mut suffix = 1;
    while destination.exists() {
        destination = directory.join(format!("{stem} ({suffix}).{extension}"));
        suffix += 1;
    }
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    tokio::fs::write(&destination, bytes)
        .await
        .map_err(|e| e.to_string())?;
    Ok(destination.to_string_lossy().to_string())
}

/// Download a song to a user-visible folder without converting the online
/// song into a local library entry. The cache copy remains what playback uses.
#[tauri::command]
pub async fn download_song_to_path(
    state: tauri::State<'_, AppState>,
    id: String,
    path: String,
) -> Result<String, String> {
    let destination = std::path::PathBuf::from(&path);
    let directory = destination
        .parent()
        .ok_or_else(|| "invalid destination path".to_string())?
        .to_path_buf();
    let produced = download_song_to(state, id, Some(directory.to_string_lossy().to_string())).await?;
    let produced_path = std::path::PathBuf::from(&produced);
    if produced_path != destination {
        let _ = tokio::fs::remove_file(&destination).await;
        tokio::fs::rename(&produced_path, &destination)
            .await
            .map_err(|e| format!("move download: {e}"))?;
    }
    Ok(destination.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn download_remote_to_path(
    url: String,
    headers: Option<std::collections::HashMap<String, String>>,
    path: String,
) -> Result<String, String> {
    let destination = std::path::PathBuf::from(&path);
    let directory = destination
        .parent()
        .ok_or_else(|| "invalid destination path".to_string())?
        .to_path_buf();
    let produced =
        download_remote_to(url, headers, Some(directory.to_string_lossy().to_string())).await?;
    let produced_path = std::path::PathBuf::from(&produced);
    if produced_path != destination {
        let _ = tokio::fs::remove_file(&destination).await;
        tokio::fs::rename(&produced_path, &destination)
            .await
            .map_err(|e| format!("move download: {e}"))?;
    }
    Ok(destination.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn download_song_to(
    state: tauri::State<'_, AppState>,
    id: String,
    dest_dir: Option<String>,
) -> Result<String, String> {
    let song_id = SongId(id.clone());
    let song = state
        .db
        .get_song(&song_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "song not found".to_string())?;

    let data_dir = dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let cache = crate::features::cache::CacheManager::new(
        state.db.as_ref().clone(),
        &data_dir.join("vide"),
    )
    .await
    .map_err(|e| format!("cache init: {e}"))?;

    let cached_path = if let Some(cached) = cache
        .get_cached(&song_id)
        .await
        .map_err(|e| format!("cache lookup: {e}"))?
    {
        cached
    } else {
        let (platform, source_id) = song
            .sources
            .first()
            .map(|source| (source.platform.clone(), source.id.clone()))
            .ok_or_else(|| "no online source to download".to_string())?;
        let plugins = state
            .plugins
            .as_ref()
            .ok_or_else(|| "plugin system not available".to_string())?;
        let descriptor = plugins.stream(&platform, &source_id, &id).await?;
        cache
            .download_from_descriptor(&song_id, &descriptor)
            .await
            .map_err(|e| format!("download: {e}"))?
    };

    let directory = dest_dir
        .map(std::path::PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| dirs::download_dir())
        .or_else(|| dirs::home_dir().map(|home| home.join("Music")))
        .ok_or_else(|| "cannot determine a download directory".to_string())?;
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(|e| format!("create download dir: {e}"))?;

    let extension = detect_audio_extension(&cached_path);
    let mut destination = directory.join(safe_download_name(
        &song.metadata.display_title.clone().unwrap_or(song.title.clone()),
        &song.metadata.artist_override.clone().unwrap_or(song.artist.clone()),
        extension,
    ));
    let stem = destination
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "audio".to_string());
    let mut suffix = 1;
    while destination.exists() {
        destination = directory.join(format!("{stem} ({suffix}).{extension}"));
        suffix += 1;
    }

    tokio::fs::copy(&cached_path, &destination)
        .await
        .map_err(|e| format!("save download: {e}"))?;
    Ok(destination.to_string_lossy().to_string())
}

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

    // Already a local file (and the file really exists) — nothing to download.
    // Checking existence also avoids treating a deleted local path as valid.
    if song.paths.iter().any(|path| path.exists()) {
        return Ok(song.paths.len() as u64);
    }

    // Build a CacheManager rooted in the app data directory.
    let data_dir = dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let basic_dir = data_dir.join("vide");
    let cache = crate::features::cache::CacheManager::new(
        state.db.as_ref().clone(),
        &basic_dir,
    )
    .await
    .map_err(|e| format!("cache init: {e}"))?;

    // Prefer an existing, complete cache copy. Only fetch a new stream when
    // there is no valid cache entry (missing file, wrong size, or invalid).
    let cached_path = if let Some(cached) = cache
        .get_cached(&song_id)
        .await
        .map_err(|e| format!("cache lookup: {e}"))?
    {
        cached
    } else {
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

        cache
            .download_from_descriptor(&song_id, &descriptor)
            .await
            .map_err(|e| format!("download: {e}"))?
    };

    // Register the cached file as a local path (dedupe).
    let mut song = song;
    if !song.paths.contains(&cached_path) {
        song.paths.push(cached_path);
    }
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
    let roots = state.config.lock().await.library_roots.clone();
    let mut songs = crate::features::search::search(&*state.db, &query).await?;
    songs.retain(|song| visible_in_library(song, &roots));
    Ok(songs)
}

#[tauri::command]
pub async fn scan_library(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ScanResult, String> {
    let (roots, excludes) = {
        let config = state.config.lock().await;
        (config.library_roots.clone(), config.exclude_roots.clone())
    };

    let extensions = state
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

    // Merge instead of deleting every local song first: tags, playlist
    // entries and overrides on existing files must survive a rescan.
    let result =
        crate::features::library::scan_roots(&state.db, &roots, &excludes, &extensions).await;
    // Refresh any open list view (and pick up newly excluded folders).
    let _ = app.emit("vide:library-changed", ());
    Ok(result)
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
    if let Some(path) = song.paths.iter().find(|path| path.exists()) {
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

    // Online lyrics through the platform's lyrics provider.
    if let Some(plugins) = state.plugins.as_ref() {
        if let Some(source) = song.sources.first() {
            if let Ok(Some(lyrics)) = plugins.lyrics(&source.platform, &source.id).await {
                if !lyrics.text.is_empty() {
                    let mut song = song;
                    song.metadata.lyrics = Some(lyrics.text.clone());
                    let _ = state.db.upsert_song(&song).await;
                    return Ok(Some(lyrics.text));
                }
            }
        }
    }

    Ok(None)
}

/// Queue and play a list of online search results. Search results are created
/// as Song rows (without resolving each stream) so next/prev walks the search
/// or playlist result list rather than the local library query.
#[tauri::command]
pub async fn play_online_results(
    state: tauri::State<'_, AppState>,
    results: Vec<Track>,
    index: usize,
) -> Result<PlaybackStatus, String> {
    if results.is_empty() {
        return Err("empty result list".to_string());
    }
    let mut ids: Vec<String> = Vec::new();
    let mut start_index: isize = 0;

    for (position, item) in results.iter().enumerate() {
        if item.platform.is_empty() || item.source_id.is_empty() {
            continue;
        }
        let song_id = song_id_from_source(&item.platform, &item.source_id);
        if state
            .db
            .get_song(&song_id)
            .await
            .map_err(|e| e.to_string())?
            .is_none()
        {
            let song = Song {
                id: song_id.clone(),
                title: if item.title.is_empty() {
                    "Unknown".to_string()
                } else {
                    item.title.clone()
                },
                artist: item.artist.clone().unwrap_or_default(),
                album: item.album.clone().unwrap_or_default(),
                tags: Vec::new(),
                sources: vec![Source {
                    platform: item.platform.clone(),
                    id: item.source_id.clone(),
                }],
                paths: Vec::new(),
                metadata: SongMetadata {
                    duration: item.duration,
                    cover_url: item.cover_url.clone(),
                    added_time: Some(crate::infra::time::now_date()),
                    ..Default::default()
                },
            };
            state
                .db
                .upsert_song(&song)
                .await
                .map_err(|e| e.to_string())?;
        }
        if position == index {
            start_index = ids.len() as isize;
        }
        ids.push(song_id.0);
    }

    if ids.is_empty() {
        return Err("no valid results".to_string());
    }
    let mut player = state.player.lock().await;
    player.set_queue(ids, start_index).await
}

#[tauri::command]
pub async fn play_remote_media(
    state: tauri::State<'_, AppState>,
    id: String,
    url: String,
    headers: Option<std::collections::HashMap<String, String>>,
    position_secs: Option<f64>,
) -> Result<PlaybackStatus, String> {
    let descriptor = StreamDescriptor::UrlStream {
        id,
        url,
        headers,
    };
    let mut player = state.player.lock().await;
    player.play(&descriptor).await?;
    if let Some(position) = position_secs {
        if position > 0.0 {
            let _ = player.seek_when_ready(position).await;
        }
    }
    Ok(player.status())
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
    platform: String,
    limit: Option<u32>,
) -> Result<Vec<Track>, String> {
    if platform.trim().is_empty() {
        return Err("platform is required".to_string());
    }
    let plugins = state
        .plugins
        .as_ref()
        .ok_or_else(|| "plugin system not available".to_string())?;
    plugins
        .search(&platform, &query, limit.unwrap_or(20))
        .await
}

/// Resolve an online song by platform + id, creating a Song in the DB if
/// it does not already exist.
#[tauri::command]
pub async fn resolve_song(
    state: tauri::State<'_, AppState>,
    platform: String,
    source_id: String,
) -> Result<Song, String> {
    let song_id = song_id_from_source(&platform, &source_id);

    let plugins = state
        .plugins
        .as_ref()
        .ok_or_else(|| "plugin system not available".to_string())?;

    // Existing online songs may have been cached before cover/duration/date
    // metadata was available. Refresh only the missing fields.
    if let Some(mut song) = state
        .db
        .get_song(&song_id)
        .await
        .map_err(|e| e.to_string())?
    {
        let needs_metadata = song.metadata.cover_url.is_none()
            || song.metadata.duration.is_none()
            || song.metadata.added_time.is_none();
        if needs_metadata {
            if let Ok(item) = plugins.resolve(&platform, &source_id).await {
                if song.metadata.cover_url.is_none() {
                    song.metadata.cover_url = item.cover_url.clone();
                }
                if song.metadata.duration.is_none() {
                    song.metadata.duration = item.duration;
                }
                if song.metadata.added_time.is_none() {
                    song.metadata.added_time = Some(crate::infra::time::now_date());
                }
                let _ = state.db.upsert_song(&song).await;
            }
        }
        return Ok(song);
    }

    let item = plugins.resolve(&platform, &source_id).await?;

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
            id: source_id,
        }],
        paths: Vec::new(),
        metadata: SongMetadata {
            duration: item.duration,
            cover_url: item.cover_url.clone(),
            added_time: Some(crate::infra::time::now_date()),
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
        let song_id = crate::infra::hash::song_id_from_source(&item.platform, &item.source_id);

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
                    id: item.source_id.clone(),
                }],
                paths: Vec::new(),
                metadata: SongMetadata {
                    duration: item.duration,
                    cover_url: item.cover_url.clone(),
                    added_time: Some(crate::infra::time::now_date()),
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

    // len()+1 collides after deleting a middle item; take the highest existing
    // numeric suffix instead so ids stay unique.
    let next_id = presets
        .iter()
        .filter_map(|p| p.id.strip_prefix("preset_"))
        .filter_map(|suffix| suffix.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    let preset = crate::models::SearchPreset {
        id: format!("preset_{next_id}"),
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

// ── Library query history ────────────────────────────────────────

/// Recent library queries + the query the list should restore on launch.
/// Kept next to the saved-query commands and stored in SQLite, not
/// localStorage.
#[derive(Debug, Clone, Serialize)]
pub struct LibraryQueryState {
    pub history: Vec<String>,
    pub last_query: String,
}

fn decode_query_history(value: Option<serde_json::Value>) -> Vec<String> {
    value
        .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
        .unwrap_or_default()
}

async fn load_library_query_state(
    db: &crate::infra::db::Database,
) -> Result<LibraryQueryState, String> {
    let history = decode_query_history(
        db.get_setting("query_history")
            .await
            .map_err(|e| e.to_string())?,
    );
    let last_query = db
        .get_setting("last_library_query")
        .await
        .map_err(|e| e.to_string())?
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();
    Ok(LibraryQueryState {
        history,
        last_query,
    })
}

/// Load the query history and the last executed library query.
#[tauri::command]
pub async fn get_library_query_state(
    state: tauri::State<'_, AppState>,
) -> Result<LibraryQueryState, String> {
    load_library_query_state(&state.db).await
}

/// Set the restored query without touching the history (used by column-header
/// sorting, which would otherwise flood the history).
#[tauri::command]
pub async fn set_last_library_query(
    state: tauri::State<'_, AppState>,
    query: String,
) -> Result<(), String> {
    state
        .db
        .update_setting("last_library_query", &serde_json::json!(query.trim()))
        .await
        .map_err(|e| e.to_string())
}

/// Record a query as executed: it becomes the restored query and is pushed to
/// the front of the history list (deduplicated, capped at 30).
#[tauri::command]
pub async fn record_library_query(
    state: tauri::State<'_, AppState>,
    query: String,
) -> Result<LibraryQueryState, String> {
    let query = query.trim().to_string();
    let mut history = decode_query_history(
        state
            .db
            .get_setting("query_history")
            .await
            .map_err(|e| e.to_string())?,
    );
    history.retain(|entry| !entry.is_empty() && entry != &query);
    if !query.is_empty() {
        history.insert(0, query.clone());
        history.truncate(30);
    }
    state
        .db
        .update_setting("query_history", &serde_json::json!(history))
        .await
        .map_err(|e| e.to_string())?;
    state
        .db
        .update_setting("last_library_query", &serde_json::json!(query))
        .await
        .map_err(|e| e.to_string())?;
    Ok(LibraryQueryState {
        history,
        last_query: query,
    })
}

/// Remove one entry from the library query history.
#[tauri::command]
pub async fn remove_query_history(
    state: tauri::State<'_, AppState>,
    query: String,
) -> Result<LibraryQueryState, String> {
    let mut history = decode_query_history(
        state
            .db
            .get_setting("query_history")
            .await
            .map_err(|e| e.to_string())?,
    );
    history.retain(|entry| entry != &query);
    state
        .db
        .update_setting("query_history", &serde_json::json!(history))
        .await
        .map_err(|e| e.to_string())?;
    load_library_query_state(&state.db).await
}

/// Clear the query history without touching the last executed query.
#[tauri::command]
pub async fn clear_query_history(
    state: tauri::State<'_, AppState>,
) -> Result<LibraryQueryState, String> {
    state
        .db
        .update_setting("query_history", &serde_json::json!([]))
        .await
        .map_err(|e| e.to_string())?;
    load_library_query_state(&state.db).await
}

// Online search history is a separate list from the library query history.
// It is persisted in SQLite for the same reason: the search page is not the
// only place that may want to read it later.

#[tauri::command]
pub async fn get_online_search_history(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    Ok(decode_query_history(
        state
            .db
            .get_setting("online_search_history")
            .await
            .map_err(|e| e.to_string())?,
    ))
}

#[tauri::command]
pub async fn add_online_search_history(
    state: tauri::State<'_, AppState>,
    query: String,
) -> Result<Vec<String>, String> {
    let query = query.trim().to_string();
    let mut history = decode_query_history(
        state
            .db
            .get_setting("online_search_history")
            .await
            .map_err(|e| e.to_string())?,
    );
    history.retain(|entry| !entry.is_empty() && entry != &query);
    if !query.is_empty() {
        history.insert(0, query);
        history.truncate(20);
    }
    state
        .db
        .update_setting("online_search_history", &serde_json::json!(history))
        .await
        .map_err(|e| e.to_string())?;
    Ok(history)
}

#[tauri::command]
pub async fn set_online_search_history(
    state: tauri::State<'_, AppState>,
    history: Vec<String>,
) -> Result<(), String> {
    let history: Vec<String> = history
        .into_iter()
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
        .take(20)
        .collect();
    state
        .db
        .update_setting("online_search_history", &serde_json::json!(history))
        .await
        .map_err(|e| e.to_string())
}

// ── Generic client/UI state ──────────────────────────────────────

/// Get a namespaced UI-state blob (column layout, window geometry, scroll
/// position, ...). These live in the same SQLite settings table so the app no
/// longer treats localStorage as a second source of truth.
#[tauri::command]
pub async fn get_client_state(
    state: tauri::State<'_, AppState>,
    key: String,
) -> Result<Option<serde_json::Value>, String> {
    state
        .db
        .get_setting(&format!("client:{key}"))
        .await
        .map_err(|e| e.to_string())
}

/// Store a UI-state blob under a namespaced key.
#[tauri::command]
pub async fn set_client_state(
    state: tauri::State<'_, AppState>,
    key: String,
    value: serde_json::Value,
) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("empty client state key".to_string());
    }
    state
        .db
        .update_setting(&format!("client:{key}"), &value)
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

/// Reveal a local file in the system file manager, selecting it when the
/// platform supports it.
///
/// The opener plugin's Linux path relies entirely on the D-Bus
/// FileManager1/xdg-desktop-portal interfaces, which are missing on many
/// desktops (and then report a failure even though the file exists). This
/// command uses the platform's native facility and falls back to opening the
/// containing folder, so "details" never reports a false failure when the
/// folder could be opened.
#[tauri::command]
pub async fn reveal_local_file(path: String) -> Result<(), String> {
    // File-manager commands can block (D-Bus round-trip, xdg-open); keep that
    // off the async runtime's worker threads.
    tauri::async_runtime::spawn_blocking(move || {
        let requested = PathBuf::from(&path);
        if !requested.exists() {
            return Err(format!("file not found: {}", requested.display()));
        }
        // Resolve symlinks so the file manager selects the real entry.
        let resolved = std::fs::canonicalize(&requested).unwrap_or(requested);
        reveal_in_file_manager(&resolved)
    })
    .await
    .map_err(|e| format!("reveal task failed: {e}"))?
}

#[cfg(target_os = "windows")]
fn reveal_in_file_manager(path: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let mut cmd = std::process::Command::new("explorer");
    // Single command-line token, otherwise explorer treats the path as a
    // separate item to open instead of the item to select.
    cmd.raw_arg(format!("/select,\"{}\"", path.display()));
    // explorer.exe reports a non-zero exit code even on success; spawning it
    // is the success condition.
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
fn reveal_in_file_manager(path: &Path) -> Result<(), String> {
    let status = std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("open -R failed".to_string())
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn reveal_in_file_manager(path: &Path) -> Result<(), String> {
    // Best effort: org.freedesktop.FileManager1 can both open the folder and
    // select the item. dbus-send avoids pulling in a D-Bus client crate.
    if let Some(uri) = file_uri(path) {
        let ok = std::process::Command::new("dbus-send")
            .args([
                "--session",
                "--dest=org.freedesktop.FileManager1",
                "--type=method_call",
                "/org/freedesktop/FileManager1",
                "org.freedesktop.FileManager1.ShowItems",
                &format!("array:string:{uri}"),
                "string:",
            ])
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        if ok {
            return Ok(());
        }
    }

    // Fallback: open the containing folder. Selecting the item is nicer, but
    // opening the folder is still useful and must not be reported as failure.
    let dir = path.parent().unwrap_or(path);
    for opener in ["gio", "xdg-open"] {
        let ok = std::process::Command::new(opener)
            .arg(dir)
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        if ok {
            return Ok(());
        }
    }
    Err("could not open the file manager".to_string())
}

/// Percent-encode a filesystem path into a file:// URI for D-Bus.
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn file_uri(path: &Path) -> Option<String> {
    let raw = path.to_str()?;
    let mut out = String::from("file://");
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    Some(out)
}

// ── Song details (hover card) ─────────────────────────

/// Technical information about one local file of a song.
#[derive(Debug, Clone, Serialize)]
pub struct SongFileInfo {
    pub path: String,
    pub exists: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// Unix timestamp (seconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bit_depth: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
}

/// Everything the list's hover card needs: the song plus per-file technical
/// details that are not stored in the database.
#[derive(Debug, Clone, Serialize)]
pub struct SongDetails {
    pub song: Song,
    pub files: Vec<SongFileInfo>,
}

fn inspect_file(path: &Path) -> SongFileInfo {
    use lofty::prelude::AudioFile;

    let mut info = SongFileInfo {
        path: path.to_string_lossy().to_string(),
        exists: false,
        size: None,
        modified: None,
        format: path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase()),
        bitrate: None,
        sample_rate: None,
        channels: None,
        bit_depth: None,
        duration: None,
    };

    if let Ok(meta) = std::fs::metadata(path) {
        info.exists = true;
        info.size = Some(meta.len());
        info.modified = meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs());
    }

    if info.exists {
        if let Ok(tagged) = lofty::read_from_path(path) {
            let properties = tagged.properties();
            info.duration = Some(properties.duration().as_secs_f64());
            info.bitrate = properties.audio_bitrate();
            info.sample_rate = properties.sample_rate();
            info.channels = properties.channels();
            info.bit_depth = properties.bit_depth();
        }
    }

    info
}

/// Full details for the hover card: the song row data plus file stats and
/// audio properties for each local path.
#[tauri::command]
pub async fn get_song_details(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<SongDetails, String> {
    let song = state
        .db
        .get_song(&SongId(id))
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "song not found".to_string())?;

    let paths = song.paths.clone();
    let files = tauri::async_runtime::spawn_blocking(move || {
        paths.iter().map(|path| inspect_file(path)).collect::<Vec<_>>()
    })
    .await
    .map_err(|e| format!("song details task failed: {e}"))?;

    Ok(SongDetails { song, files })
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
        match manager.on_shortcut(accelerator.as_str(), move |app, _shortcut, event| {
            // global-hotkey reports both the press and the release of a
            // hotkey (on X11 and macOS explicitly, on Windows through a
            // release poll). Forwarding every event made each shortcut fire
            // twice — e.g. "next track" skipped two songs. Only the press is
            // the shortcut.
            if event.state() != tauri_plugin_global_shortcut::ShortcutState::Pressed {
                return;
            }
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

/// Distinct values for a CONTAINS field, used by the query editor's
/// completion. "song_metadata" is the union of title/artist/album.
#[tauri::command]
pub async fn query_field_values(
    state: tauri::State<'_, AppState>,
    field: String,
    prefix: String,
    limit: usize,
) -> Result<Vec<String>, String> {
    let limit = limit.clamp(1, 50);
    if field == "song_metadata" {
        let mut values = Vec::new();
        for name in ["title", "artist", "album"] {
            if let Ok(part) = state
                .db
                .query_field_values(name, &prefix, limit)
                .await
            {
                values.extend(part);
            }
        }
        values.sort_by_key(|value| value.to_lowercase());
        values.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        values.truncate(limit);
        return Ok(values);
    }
    state
        .db
        .query_field_values(&field, &prefix, limit)
        .await
        .map_err(|e| e.to_string())
}

/// Values of one tag namespace, used after "namespace:" in the query editor.
#[tauri::command]
pub async fn tag_values(
    state: tauri::State<'_, AppState>,
    namespace: String,
    prefix: String,
    limit: usize,
) -> Result<Vec<String>, String> {
    state
        .db
        .tag_values(&namespace, &prefix, limit.clamp(1, 50))
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

/// Fetch a remote cover image with browser-like headers and return a data URL.
/// Some CDNs (notably Bilibili) reject requests without a Referer/User-Agent.
#[tauri::command]
pub async fn fetch_remote_image(
    url: String,
    referer: Option<String>,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let mut request = client
        .get(&url)
        .header(reqwest::header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120 Safari/537.36");
    if let Some(referer) = referer {
        request = request.header(reqwest::header::REFERER, referer);
    }
    let response = request.send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(format!("data:{content_type};base64,{encoded}"))
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
