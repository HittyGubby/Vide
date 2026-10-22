use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::models::tag::Tag;

/// Deterministic ID newtype (hex hash)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SongId(pub String);

/// Source platform identifier { platform, id }
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Source {
    pub platform: String,
    pub id: String,
}

/// SongMetadata combines built-in searchable attributes (§4.1) and user overrides (§4.2).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SongMetadata {
    // Built-in attributes (searchable)
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub source: Option<String>,
    pub path: Option<String>,
    pub added_time: Option<String>,
    pub duration: Option<f64>,
    /// Remote cover URL (usually from the source platform).
    pub cover_url: Option<String>,

    // User overrides
    pub file_name: Option<String>,
    pub display_title: Option<String>,
    pub artist_override: Option<String>,
    pub album_override: Option<String>,

    // Extended metadata
    pub lyrics: Option<String>,
    pub notes: Option<String>,
}

/// Core song entity — immutable object.
/// Updates produce a new Song rather than mutating in place.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Song {
    pub id: SongId,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub tags: Vec<Tag>,
    pub sources: Vec<Source>,
    pub paths: Vec<PathBuf>,
    pub metadata: SongMetadata,
}
