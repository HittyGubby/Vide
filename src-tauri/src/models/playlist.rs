use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::models::song::SongId;

/// Playlist ID newtype
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlaylistId(pub String);

/// A single entry in a playlist
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistEntry {
    pub song_id: SongId,
    pub import_order: Option<u32>,
}

/// Source of a playlist
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlaylistSource {
    M3U { file_path: PathBuf },
    UserCreated,
}

/// A playlist / import
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: PlaylistId,
    pub name: String,
    pub entries: Vec<PlaylistEntry>,
    pub source: PlaylistSource,
}
