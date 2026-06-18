use std::io;
use std::path::Path;

use crate::models::SongId;

/// Quick hash result: just (size, mtime) for fast comparison during scanning.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QuickHash {
    pub size: u64,
    pub mtime: u64,
}

/// Compute a deterministic SongId from a platform source.
/// formula: hash(platform + source_id)
pub fn song_id_from_source(platform: &str, source_id: &str) -> SongId {
    let input = format!("{}:{}", platform, source_id);
    let hash = blake3::hash(input.as_bytes());
    SongId(hash.to_hex().to_string())
}

/// Compute a deterministic SongId from a local file.
/// formula: hash(size + mtime + first_chunk + last_chunk)
pub fn song_id_from_file(path: &Path) -> io::Result<SongId> {
    let meta = std::fs::metadata(path)?;
    let size = meta.len();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let (first, last) = crate::infra::fs::read_first_last_chunks(path, 4096)?;

    let mut hasher = blake3::Hasher::new();
    hasher.update(&size.to_le_bytes());
    hasher.update(&mtime.to_le_bytes());
    hasher.update(&first);
    hasher.update(&last);

    Ok(SongId(hasher.finalize().to_hex().to_string()))
}

/// Fast hash using only size and mtime for scanning comparisons.
pub fn quick_hash(path: &Path) -> io::Result<QuickHash> {
    let meta = std::fs::metadata(path)?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    Ok(QuickHash {
        size: meta.len(),
        mtime,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_song_id_from_source_is_deterministic() {
        let id1 = song_id_from_source("youtube", "abc123");
        let id2 = song_id_from_source("youtube", "abc123");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_song_id_from_source_differs_for_different_sources() {
        let id1 = song_id_from_source("youtube", "abc123");
        let id2 = song_id_from_source("bilibili", "abc123");
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_song_id_from_source_is_hex() {
        let id = song_id_from_source("youtube", "abc123");
        assert_eq!(id.0.len(), 64); // blake3 hex = 64 chars
        assert!(id.0.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
