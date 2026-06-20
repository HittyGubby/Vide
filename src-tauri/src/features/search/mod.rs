use crate::infra::db::Database;
use crate::models::song::Song;
use crate::models::tag::Tag;
use crate::models::SongId;

pub mod fuzzy;
pub mod parser;

use fuzzy::FuzzyMatch;
use parser::{evaluate, parse_query, build_tag_index};

/// Structured search: parse a query string and return matching songs.
///
/// Supports boolean operators (AND/OR/NOT), tag filters (ns:value),
/// and text field filters (field CONTAINS "value").
pub async fn search(db: &Database, query: &str) -> Result<Vec<Song>, String> {
    // Fetch all songs (without associated data)
    let songs = db
        .query_songs(&crate::infra::db::QueryFilter::default())
        .await
        .map_err(|e| e.to_string())?;

    if songs.is_empty() {
        return Ok(Vec::new());
    }

    // Fetch all tags for building the tag index
    let all_tags = fetch_all_tags(db).await.map_err(|e| e.to_string())?;
    let tag_index = build_tag_index(&all_tags);

    // Parse and evaluate
    let ast = parse_query(query).map_err(|e| e.to_string())?;
    let results = evaluate(&ast, &songs, &tag_index);

    Ok(results)
}

/// Secondary fuzzy search: match songs by loose text matching on
/// title, artist, and album. Returns results sorted by relevance.
pub async fn fuzzy_search_songs(
    db: &Database,
    text: &str,
) -> Result<Vec<FuzzyMatch>, String> {
    let songs = db
        .query_songs(&crate::infra::db::QueryFilter::default())
        .await
        .map_err(|e| e.to_string())?;

    Ok(fuzzy::fuzzy_search(text, &songs))
}

/// Autocomplete suggestions for the search bar.
///
/// Returns up to `limit` suggestions matching the given prefix.
/// Suggestions include tag namespaces, field names, and known values.
pub async fn autocomplete(
    db: &Database,
    prefix: &str,
    limit: usize,
) -> Result<Vec<String>, String> {
    let mut suggestions: Vec<String> = Vec::new();
    let prefix = prefix.to_lowercase();

    // Field names
    let fields = [
        "title", "artist", "album", "source", "path",
        "added_time", "duration", "import_order", "import_source",
    ];
    for f in &fields {
        if f.starts_with(&prefix) {
            suggestions.push(f.to_string());
        }
    }

    // Tag namespaces from the database
    if let Ok(namespaces) = crate::features::tags::get_namespaces(db).await {
        for ns in &namespaces {
            if ns.to_lowercase().starts_with(&prefix) {
                suggestions.push(format!("{}:", ns));
            }
        }
    }

    // Song title prefix matches
    let mut title_filter = crate::infra::db::QueryFilter::default();
    title_filter.query = Some(prefix.clone());
    title_filter.limit = Some(limit as u64);
    if let Ok(songs) = db.query_songs(&title_filter).await {
        for song in &songs {
            if suggestions.len() >= limit {
                break;
            }
            let s = format!("title CONTAINS \"{}\"", song.title);
            if !suggestions.contains(&s) {
                suggestions.push(s);
            }
        }
    }

    suggestions.truncate(limit);
    Ok(suggestions)
}

/// Fetch all tag entries from the database.
async fn fetch_all_tags(db: &Database) -> Result<Vec<(SongId, Tag)>, sea_orm::DbErr> {
    use crate::infra::db::entity::tag;
    use sea_orm::EntityTrait;

    let rows = tag::Entity::find().all(&db.conn).await?;
    Ok(rows
        .into_iter()
        .map(|t| {
            (
                SongId(t.song_id),
                Tag {
                    namespace: t.namespace,
                    value: t.value,
                },
            )
        })
        .collect())
}
