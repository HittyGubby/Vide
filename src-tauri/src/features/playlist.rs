use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use tracing::warn;

use crate::infra::db::Database;
use crate::models::{
    playlist::{Playlist, PlaylistEntry, PlaylistId, PlaylistSource},
    song::SongId,
};

/// A single entry parsed from a M3U file.
#[derive(Debug, Clone)]
pub struct M3uEntry {
    /// Resolved file path or URL.
    pub target: String,
    /// Optional duration in seconds from #EXTINF (parsed but available for importers).
    #[allow(dead_code)]
    pub duration: Option<u64>,
    /// Optional title from #EXTINF (parsed but available for importers).
    #[allow(dead_code)]
    pub title: Option<String>,
}

/// Errors that can occur during M3U processing.
#[derive(Debug)]
pub enum M3uError {
    Io(io::Error),
    /// File is an HLS manifest, not a regular M3U playlist.
    HlsManifest,
    /// No entries found in the file.
    Empty,
}

impl std::fmt::Display for M3uError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            M3uError::Io(e) => write!(f, "io error: {e}"),
            M3uError::HlsManifest => write!(f, "file is an HLS manifest, skipping"),
            M3uError::Empty => write!(f, "no entries found"),
        }
    }
}

impl std::error::Error for M3uError {}

impl From<io::Error> for M3uError {
    fn from(e: io::Error) -> Self {
        M3uError::Io(e)
    }
}

/// Quick content sniff: check if a file is an HLS manifest.
/// Scans the first few lines for `#EXT-X-` directives.
pub fn is_hls_manifest(path: &Path) -> Result<bool, M3uError> {
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    for line in reader.lines().take(20) {
        let line = line?.trim().to_string();
        if line.starts_with("#EXT-X-") {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Parse a M3U file into a list of entries.
///
/// Resolves relative paths against the M3U file's parent directory.
/// Skips HLS manifests (returns `HlsManifest` error).
pub fn parse_m3u(path: &Path) -> Result<Vec<M3uEntry>, M3uError> {
    if is_hls_manifest(path)? {
        return Err(M3uError::HlsManifest);
    }

    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let base_dir = path.parent().unwrap_or(Path::new("."));

    let mut entries: Vec<M3uEntry> = Vec::new();
    let mut current_extinf: Option<(u64, String)> = None;

    for line in reader.lines() {
        let line = line?.trim().to_string();
        if line.is_empty() || line == "#EXTM3U" {
            continue;
        }
        if line.starts_with("#EXTINF:") {
            let rest = &line[8..];
            if let Some(comma_pos) = rest.find(',') {
                let secs = rest[..comma_pos].trim().parse::<f64>().unwrap_or(0.0);
                let title = rest[comma_pos + 1..].trim().to_string();
                current_extinf = Some((secs as u64, title));
            }
            continue;
        }
        if line.starts_with('#') {
            continue;
        }

        // Resolve path or URL
        let resolved = if line.starts_with("http://") || line.starts_with("https://") {
            line.clone()
        } else if Path::new(&line).is_absolute() {
            line
        } else {
            base_dir.join(&line).to_string_lossy().to_string()
        };

        entries.push(M3uEntry {
            target: resolved,
            duration: current_extinf.as_ref().map(|(d, _)| *d),
            title: current_extinf.as_ref().map(|(_, t)| t.clone()),
        });
        current_extinf = None;
    }

    if entries.is_empty() {
        return Err(M3uError::Empty);
    }

    Ok(entries)
}

// ── Database Helpers ──────────────────────────────────────────────

/// Find a song ID by matching an absolute file path against the song_paths table.
async fn resolve_song_id(db: &Database, target: &str) -> Option<SongId> {
    use crate::infra::db::entity::song_path;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    // Exact match first
    let result = song_path::Entity::find()
        .filter(song_path::Column::Path.eq(target))
        .one(&db.conn)
        .await;

    if let Ok(Some(row)) = result {
        return Some(SongId(row.song_id));
    }

    // If it's a URL, try matching as a source
    if target.starts_with("http://") || target.starts_with("https://") {
        use crate::infra::db::entity::song_source;
        let result = song_source::Entity::find()
            .filter(song_source::Column::SourceId.eq(target))
            .one(&db.conn)
            .await;

        if let Ok(Some(row)) = result {
            return Some(SongId(row.song_id));
        }
    }

    None
}

// ── Import / Export ──────────────────────────────────────────────

/// Import a M3U file into the database as a Playlist.
///
/// Steps:
/// 1. Content sniff → reject HLS manifests
/// 2. Parse M3U entries (resolve relative paths)
/// 3. Match each entry to an existing song_id (by path or source URL)
/// 4. Create a Playlist with import_order and import_source
/// 5. Persist to database
pub async fn import_m3u(db: &Database, path: &Path) -> Result<Playlist, M3uError> {
    let entries = parse_m3u(path)?;
    let file_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("imported")
        .to_string();

    // Build playlist entries with resolved song IDs
    let mut playlist_entries: Vec<PlaylistEntry> = Vec::with_capacity(entries.len());
    for (i, entry) in entries.iter().enumerate() {
        let song_id = resolve_song_id(db, &entry.target).await;
        match song_id {
            Some(id) => {
                playlist_entries.push(PlaylistEntry {
                    song_id: id,
                    import_order: Some((i + 1) as u32),
                });
            }
            None => {
                warn!(
                    "unresolved M3U entry {}: {}",
                    i + 1,
                    entry.target
                );
            }
        }
    }

    // Deterministic ID from the file path
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    let id = PlaylistId(format!("m3u:{:x}", hasher.finish()));

    let playlist = Playlist {
        id,
        name: file_name,
        entries: playlist_entries,
        source: PlaylistSource::M3U {
            file_path: path.to_path_buf(),
        },
    };

    db.upsert_playlist(&playlist)
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    Ok(playlist)
}

/// Export a playlist to M3U format.
///
/// For each entry, looks up the song's metadata and writes an
/// `#EXTINF` line followed by the first available file path.
pub async fn export_playlist(
    db: &Database,
    playlist_id: &PlaylistId,
    output: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let playlist = db.get_playlist(playlist_id).await?.ok_or("playlist not found")?;

    use std::io::Write;
    let mut file = std::fs::File::create(output)?;
    writeln!(file, "#EXTM3U")?;

    for entry in &playlist.entries {
        let song = db.get_song(&entry.song_id).await?;
        if let Some(song) = song {
            let title = song
                .metadata
                .display_title
                .as_deref()
                .or(song.metadata.title.as_deref())
                .unwrap_or(&song.title);
            let duration = song.metadata.duration.unwrap_or(0.0);

            writeln!(file, "#EXTINF:{},{}", duration as u64, title)?;

            // Use the first available local path
            if let Some(path) = song.paths.first() {
                writeln!(file, "{}", path.display())?;
            }
        } else {
            writeln!(file, "# {}", entry.song_id.0)?;
        }
    }

    Ok(())
}
