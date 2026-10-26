use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tracing::{info, warn};

use crate::features::metadata::extract_file_metadata;
use crate::infra::db::Database;
use crate::infra::fs::scan_directory;
use crate::infra::hash::{quick_hash, song_id_from_file, QuickHash};
use crate::models::song::{Song, SongId};
use serde::Serialize;
use crate::models::SongMetadata;

/// Result of a library scan.
#[derive(Debug, Default, Serialize)]
pub struct ScanResult {
    pub scanned: u64,
    pub added: u64,
    pub updated: u64,
    pub skipped: u64,
    /// Path rows removed because the file no longer exists.
    pub pruned_paths: u64,
    /// Songs removed because they had no paths, sources or tags left.
    pub pruned_songs: u64,
    pub errors: Vec<String>,
}

/// Scan a single root directory and add/update songs in the database.
///
/// For each audio file:
/// 1. Compute deterministic SongId (hash of size + mtime + first/last chunk)
/// 2. Check if a song with this ID already exists
/// 3. If exists → merge paths (dedup)
/// 4. If not exists → extract metadata and create new Song
pub async fn scan_root(
    db: &Database,
    root: &Path,
    excludes: &[PathBuf],
    extensions: &[String],
    existing_paths: &mut HashMap<String, SongId>,
) -> ScanResult {
    let mut result = ScanResult::default();

    // Resolve symlinks/relative roots so exclude-prefix checks compare paths in
    // the same form as the walker's entries.
    let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let excludes: Vec<PathBuf> = excludes
        .iter()
        .map(|path| std::fs::canonicalize(path).unwrap_or_else(|_| path.clone()))
        .collect();

    let extensions: Vec<&str> = extensions.iter().map(String::as_str).collect();
    for entry in scan_directory(&root, &extensions) {
        let path = match entry {
            Ok(p) => p,
            Err(e) => {
                warn!("scan error: {e}");
                result.errors.push(e.to_string());
                continue;
            }
        };

        // Excluded folders are part of the config but were never consulted by
        // the walker; skip them before the (expensive) hash/metadata work.
        if excludes.iter().any(|exclude| path.starts_with(exclude)) {
            result.skipped += 1;
            continue;
        }

        result.scanned += 1;

        let path_str = path.to_string_lossy().to_string();

        // The path map is loaded once per scan. An already-indexed path needs
        // no work at all; this keeps the startup scan cheap. (Reusing the
        // existing id also means a tag edit that changes mtime does not create
        // a second song and lose its tags.)
        if existing_paths.contains_key(&path_str) {
            result.skipped += 1;
            continue;
        }

        // New path: compute the deterministic ID (reads first/last 4KB).
        let song_id = match song_id_from_file(&path) {
            Ok(id) => id,
            Err(e) => {
                warn!("hash error for {:?}: {e}", path);
                result.errors.push(format!("hash failed for {:?}: {e}", path));
                result.skipped += 1;
                continue;
            }
        };

        // Check if song already exists
        match db.get_song(&song_id).await {
            Ok(Some(mut existing)) => {
                // Dedup: add path if not already present
                let path_str = path.to_string_lossy().to_string();
                let already_present = existing
                    .paths
                    .iter()
                    .any(|p| p.to_string_lossy().as_ref() == path_str);

                if !already_present {
                    existing.paths.push(path.clone());
                    if let Err(e) = db.upsert_song(&existing).await {
                        warn!("upsert error for {}: {e}", song_id.0);
                        result.errors.push(format!("upsert error: {e}"));
                        continue;
                    }
                    existing_paths.insert(path_str.clone(), song_id.clone());
                    result.updated += 1;
                    info!("updated (new path): {} at {:?}", song_id.0, path);
                } else {
                    result.skipped += 1;
                }
            }
            Ok(None) => {
                // New song – extract metadata and create
                let mut metadata = match extract_file_metadata(&path) {
                    Ok(m) => m,
                    Err(e) => {
                        warn!("metadata error for {:?}: {e}", path);
                        // Still create the song with default metadata
                        let mut m = SongMetadata::default();
                        if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                            m.title = Some(name.to_string());
                        }
                        m
                    }
                };

                if metadata.added_time.is_none() {
                    metadata.added_time = Some(
                        crate::infra::time::timestamp_from_path(&path)
                            .unwrap_or_else(crate::infra::time::now_timestamp),
                    );
                }

                let title = metadata
                    .title
                    .clone()
                    .unwrap_or_else(|| {
                        path.file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Unknown")
                            .to_string()
                    });

                let artist = metadata.artist.clone().unwrap_or_else(|| "Unknown".to_string());
                let album = metadata.album.clone().unwrap_or_else(|| "Unknown".to_string());

                let song = Song {
                    id: song_id.clone(),
                    title,
                    artist,
                    album,
                    tags: Vec::new(),
                    sources: Vec::new(),
                    paths: vec![path.clone()],
                    metadata,
                };

                if let Err(e) = db.upsert_song(&song).await {
                    warn!("insert error for {}: {e}", song_id.0);
                    result.errors.push(format!("insert error: {e}"));
                    continue;
                }
                existing_paths.insert(path_str, song_id.clone());
                result.added += 1;
                info!("added: {} at {:?}", song_id.0, path);
            }
            Err(e) => {
                warn!("db query error for {}: {e}", song_id.0);
                result.errors.push(format!("db query error: {e}"));
                continue;
            }
        }
    }

    result
}

/// Scan every configured root, then prune stale/excluded paths. Shared by the
/// manual "scan now" command and the background scan on app startup.
pub async fn scan_roots(
    db: &Database,
    roots: &[PathBuf],
    excludes: &[PathBuf],
    extensions: &[String],
) -> ScanResult {
    let mut total = ScanResult::default();

    // Normalize excludes once so path-prefix checks are consistent with the
    // absolute paths the scanner stores.
    let cwd = std::env::current_dir().unwrap_or_default();
    let excludes: Vec<PathBuf> = excludes
        .iter()
        .map(|path| {
            let absolute = if path.is_absolute() {
                path.clone()
            } else {
                cwd.join(path)
            };
            std::fs::canonicalize(&absolute).unwrap_or(absolute)
        })
        .collect();

    let mut existing_paths = db.all_song_paths().await.unwrap_or_default();
    for root in roots {
        let result = scan_root(db, root, &excludes, extensions, &mut existing_paths).await;
        total.scanned += result.scanned;
        total.added += result.added;
        total.updated += result.updated;
        total.skipped += result.skipped;
        total.errors.extend(result.errors);
    }

    match db.prune_missing_paths(&excludes).await {
        Ok((paths, songs)) => {
            total.pruned_paths = paths;
            total.pruned_songs = songs;
        }
        Err(e) => total.errors.push(format!("prune failed: {e}")),
    }

    total
}

/// Find the first available (existing on disk) path from a song's path list.
/// Implements §16.4 local file playback fallback.
pub fn find_first_available_path(song: &Song) -> Option<std::path::PathBuf> {
    for p in &song.paths {
        if p.exists() {
            return Some(p.clone());
        }
    }
    None
}

/// Check if a file has been modified since it was last indexed.
/// Returns `true` if the file needs re-indexing.
pub fn is_file_changed(path: &Path, last_hash: &QuickHash) -> bool {
    match quick_hash(path) {
        Ok(current) => current != *last_hash,
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn excluded_directories_are_not_indexed() {
        let root = std::env::temp_dir().join(format!(
            "vide-scan-test-{}",
            std::process::id()
        ));
        let included = root.join("included");
        let excluded = root.join("excluded");
        std::fs::create_dir_all(&included).unwrap();
        std::fs::create_dir_all(&excluded).unwrap();
        std::fs::write(included.join("keep.mp3"), b"first-last-chunk").unwrap();
        std::fs::write(excluded.join("skip.mp3"), b"first-last-chunk").unwrap();

        let db = Database::open_in_memory().await.unwrap();
        let mut existing = HashMap::new();
        let extensions = crate::models::settings::default_scan_extensions();
        let result = scan_root(&db, &root, &[excluded.clone()], &extensions, &mut existing).await;

        assert_eq!(result.scanned, 1);
        assert_eq!(result.skipped, 1);
        let songs = db.list_library_songs().await.unwrap();
        assert_eq!(songs.len(), 1);
        assert!(songs[0].paths[0].starts_with(&included));

        let _ = std::fs::remove_dir_all(&root);
    }
}
