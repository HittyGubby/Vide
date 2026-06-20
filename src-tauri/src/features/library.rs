use std::path::Path;

use tracing::{info, warn};

use crate::features::metadata::extract_file_metadata;
use crate::infra::db::Database;
use crate::infra::fs::scan_directory;
use crate::infra::hash::{quick_hash, song_id_from_file, QuickHash};
use crate::models::song::Song;
use crate::models::SongMetadata;

/// Supported audio file extensions.
const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "ogg", "wav", "m4a", "aac", "opus", "wma", "ape", "aiff", "wv",
];

/// Result of a library scan.
#[derive(Debug, Default)]
pub struct ScanResult {
    pub scanned: u64,
    pub added: u64,
    pub updated: u64,
    pub skipped: u64,
    pub errors: Vec<String>,
}

/// Scan a single root directory and add/update songs in the database.
///
/// For each audio file:
/// 1. Compute deterministic SongId (hash of size + mtime + first/last chunk)
/// 2. Check if a song with this ID already exists
/// 3. If exists → merge paths (dedup)
/// 4. If not exists → extract metadata and create new Song
pub async fn scan_root(db: &Database, root: &Path) -> ScanResult {
    let mut result = ScanResult::default();

    for entry in scan_directory(root, AUDIO_EXTENSIONS) {
        let path = match entry {
            Ok(p) => p,
            Err(e) => {
                warn!("scan error: {e}");
                result.errors.push(e.to_string());
                continue;
            }
        };

        result.scanned += 1;

        // Compute deterministic ID (reads first/last 4KB chunks)
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
                    result.updated += 1;
                    info!("updated (new path): {} at {:?}", song_id.0, path);
                } else {
                    result.skipped += 1;
                }
            }
            Ok(None) => {
                // New song – extract metadata and create
                let metadata = match extract_file_metadata(&path) {
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
