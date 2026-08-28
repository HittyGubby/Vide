use std::path::Path;

use lofty::file::TaggedFileExt;
use lofty::prelude::Accessor;
use lofty::prelude::AudioFile;
use lofty::read_from_path;
use tracing::warn;

use lofty::error::LoftyError;
use crate::models::SongMetadata;

/// Errors that can occur during metadata extraction.
#[derive(Debug)]
pub enum MetadataError {
    Lofty(LoftyError),
    Io(std::io::Error),
}

impl std::fmt::Display for MetadataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MetadataError::Lofty(e) => write!(f, "metadata parse error: {e}"),
            MetadataError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for MetadataError {}

impl From<LoftyError> for MetadataError {
    fn from(e: LoftyError) -> Self {
        MetadataError::Lofty(e)
    }
}

impl From<std::io::Error> for MetadataError {
    fn from(e: std::io::Error) -> Self {
        MetadataError::Io(e)
    }
}

/// Extract embedded metadata from a local audio file.
/// Reads ID3v2/Vorbis/MP4 tags and audio properties.
/// Returns a SongMetadata with whatever fields are found (others remain None).
pub fn extract_file_metadata(path: &Path) -> Result<SongMetadata, MetadataError> {
    let tagged_file = read_from_path(path)?;
    let properties = tagged_file.properties();

    let mut metadata = SongMetadata::default();

    // Duration from audio properties
    let duration = properties.duration().as_secs_f64();
    if duration > 0.0 {
        metadata.duration = Some(duration);
    }

    // Tags from the primary tag (ID3v2, Vorbis, etc.)
    if let Some(tag) = tagged_file.primary_tag() {
        metadata.title = tag.title().map(|s| s.to_string());
        metadata.artist = tag.artist().map(|s| s.to_string());
        metadata.album = tag.album().map(|s| s.to_string());

        // Embedded lyrics (ID3 USLT / Vorbis LYRICS / MP4 ©lyr)
        use lofty::tag::ItemKey;
        if let Some(lyrics) = tag.get_string(&ItemKey::Lyrics) {
            if !lyrics.trim().is_empty() {
                metadata.lyrics = Some(lyrics.to_string());
            }
        }
    }

    // File name fallback for title
    if metadata.title.is_none() {
        if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
            metadata.file_name = Some(name.to_string());
            metadata.title = Some(name.to_string());
        }
    }

    Ok(metadata)
}

/// Update a song's metadata from a local file via the database.
/// Extracts embedded metadata and upserts the song.
pub async fn update_from_file(
    db: &crate::infra::db::Database,
    song_id: &crate::models::SongId,
    path: &Path,
) {
    match extract_file_metadata(path) {
        Ok(mut file_meta) => {
            if let Ok(Some(mut song)) = db.get_song(song_id).await {
                if song.metadata.title.is_none() {
                    song.metadata.title = file_meta.title.take();
                }
                if song.metadata.artist.is_none() {
                    song.metadata.artist = file_meta.artist.take();
                }
                if song.metadata.album.is_none() {
                    song.metadata.album = file_meta.album.take();
                }
                if song.metadata.duration.is_none() {
                    song.metadata.duration = file_meta.duration;
                }
                if let Err(e) = db.upsert_song(&song).await {
                    warn!("failed to upsert song metadata for {}: {e}", song_id.0);
                }
            }
        }
        Err(e) => {
            warn!("failed to extract metadata from {:?}: {e}", path);
        }
    }
}
