use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::models::song::SongId;

/// Cache entry tracking a cached media file on disk.
/// last_used is stored as a Unix timestamp (seconds).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub song_id: SongId,
    pub path: PathBuf,
    pub size: u64,
    pub source: String,
    pub last_used: u64,
    pub valid: bool,
}
