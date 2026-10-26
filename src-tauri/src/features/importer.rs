use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use crate::infra::db::Database;
use crate::infra::hash::{song_id_from_file, song_id_from_source};
use crate::models::playlist::{Playlist, PlaylistEntry, PlaylistId, PlaylistSource};
use crate::models::song::{Song, SongId, Source};
use crate::models::tag::Tag;
use crate::models::SongMetadata;

#[derive(Debug, Clone, Default, Serialize)]
pub struct LxImportReport {
    pub playlists: u64,
    pub entries: u64,
    pub unique_songs: u64,
    pub local_linked: u64,
    pub local_created: u64,
    pub local_missing: u64,
    pub online_created: u64,
    pub online_existing: u64,
    pub skipped_mg: u64,
    pub skipped_playlists: u64,
    pub skipped_invalid: u64,
    pub tags_added: u64,
    pub playlists_saved: u64,
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(|item| match item {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    })
}

fn id_text(value: &Value) -> Option<String> {
    value.as_str().map(ToString::to_string).or_else(|| {
        value
            .as_i64()
            .map(|number| number.to_string())
            .or_else(|| value.as_u64().map(|number| number.to_string()))
    })
}

fn parse_interval(value: Option<String>) -> Option<f64> {
    let text = value?;
    let mut total = 0.0;
    for part in text.split(':') {
        let number: f64 = part.parse().ok()?;
        total = total * 60.0 + number;
    }
    Some(total)
}

fn playlist_name(raw: &str) -> String {
    match raw {
        "list__name_default" => "默认列表".to_string(),
        "list__name_love" => "我喜欢".to_string(),
        other => other.to_string(),
    }
}

fn platform_for(source: &str) -> Option<&'static str> {
    match source {
        "wy" => Some("netease"),
        "tx" => Some("tencent"),
        "kg" => Some("kugou"),
        "kw" => Some("kuwo"),
        _ => None,
    }
}

fn online_source_id(item: &Value, meta: &Value, source: &str) -> Option<String> {
    if source == "kg" {
        if let Some(hash) = text(meta, "hash") {
            return Some(hash);
        }
    }
    if let Some(song_id) = meta.get("songId").and_then(id_text) {
        return Some(song_id);
    }
    text(item, "id").map(|id| id.split_once('_').map(|(_, value)| value.to_string()).unwrap_or(id))
}

fn merge_import_metadata(song: &mut Song, meta: &Value, duration: Option<f64>) -> bool {
    let mut changed = false;
    if song.metadata.cover_url.is_none() {
        if let Some(cover) = text(meta, "picUrl") {
            song.metadata.cover_url = Some(cover);
            changed = true;
        }
    }
    if song.metadata.duration.is_none() && duration.is_some() {
        song.metadata.duration = duration;
        changed = true;
    }
    changed
}

async fn upsert_tags(
    db: &Database,
    mut song: Song,
    tags: &[Tag],
    report: &mut LxImportReport,
) -> Result<SongId, String> {
    let id = song.id.clone();
    let mut changed = false;
    for tag in tags {
        if !song.tags.contains(tag) {
            song.tags.push(tag.clone());
            report.tags_added += 1;
            changed = true;
        }
    }
    if changed {
        db.upsert_song(&song).await.map_err(|e| e.to_string())?;
    }
    Ok(id)
}

pub async fn import_lx_list(db: &Database, root: &Value) -> Result<LxImportReport, String> {
    let playlists = root
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| "not an LX playList_v2 file: missing data array".to_string())?;

    let mut report = LxImportReport::default();
    let mut unique: HashSet<SongId> = HashSet::new();

    for playlist_value in playlists {
        let playlist_id = text(playlist_value, "id").unwrap_or_else(|| "default".to_string());
        let raw_name = text(playlist_value, "name").unwrap_or_else(|| playlist_id.clone());
        if playlist_id == "default"
            || playlist_id == "love"
            || raw_name == "list__name_default"
            || raw_name == "list__name_love"
        {
            report.skipped_playlists += 1;
            continue;
        }
        let name = playlist_name(&raw_name);
        let items = playlist_value
            .get("list")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let playlist_tag = Tag {
            namespace: "playlist".to_string(),
            value: name.clone(),
        };
        let mut entries = Vec::with_capacity(items.len());
        report.playlists += 1;
        report.entries += items.len() as u64;

        for (index, item) in items.iter().enumerate() {
            let source = text(item, "source").unwrap_or_default();
            let meta = item.get("meta").cloned().unwrap_or(Value::Null);
            let duration = parse_interval(text(item, "interval"));
            let title = text(item, "name").unwrap_or_else(|| "Unknown".to_string());
            let artist = text(item, "singer").unwrap_or_default();
            let album = text(&meta, "albumName").unwrap_or_default();

            let song_id = if source == "local" {
                let path = text(&meta, "filePath")
                    .or_else(|| text(item, "id"))
                    .unwrap_or_default();
                let path = PathBuf::from(path);
                if path.as_os_str().is_empty() || !path.exists() {
                    report.local_missing += 1;
                    continue;
                }
                let path_text = path.to_string_lossy().to_string();
                let existing_id = db
                    .find_song_id_by_path(&path_text)
                    .await
                    .map_err(|e| e.to_string())?;
                if let Some(existing_id) = existing_id {
                    let Some(mut existing) = db.get_song(&existing_id).await.map_err(|e| e.to_string())? else {
                        report.skipped_invalid += 1;
                        continue;
                    };
                    if merge_import_metadata(&mut existing, &meta, duration) {
                        db.upsert_song(&existing).await.map_err(|e| e.to_string())?;
                    }
                    report.local_linked += 1;
                    upsert_tags(db, existing, &[playlist_tag.clone()], &mut report).await?
                } else {
                    let song_id = match song_id_from_file(&path) {
                        Ok(id) => id,
                        Err(_) => {
                            report.skipped_invalid += 1;
                            continue;
                        }
                    };
                    if let Some(mut existing) = db.get_song(&song_id).await.map_err(|e| e.to_string())? {
                        let mut path_changed = merge_import_metadata(&mut existing, &meta, duration);
                        if !existing.paths.iter().any(|value| value == &path) {
                            existing.paths.push(path.clone());
                            path_changed = true;
                        }
                        if path_changed {
                            db.upsert_song(&existing).await.map_err(|e| e.to_string())?;
                        }
                        report.local_linked += 1;
                        upsert_tags(db, existing, &[playlist_tag.clone()], &mut report).await?
                    } else {
                        let song = Song {
                            id: song_id,
                            title,
                            artist,
                            album,
                            tags: vec![playlist_tag.clone()],
                            sources: Vec::new(),
                            paths: vec![path.clone()],
                            metadata: SongMetadata {
                                duration,
                                cover_url: text(&meta, "picUrl"),
                                file_name: path
                                    .file_name()
                                    .map(|name| name.to_string_lossy().to_string()),
                                // Local files are stamped with their own mtime.
                                added_time: Some(
                                    crate::infra::time::timestamp_from_path(&path)
                                        .unwrap_or_else(crate::infra::time::now_timestamp),
                                ),
                                ..Default::default()
                            },
                        };
                        db.upsert_song(&song).await.map_err(|e| e.to_string())?;
                        report.local_created += 1;
                        report.tags_added += 1;
                        song.id
                    }
                }
            } else if source == "mg" {
                report.skipped_mg += 1;
                continue;
            } else if let Some(platform) = platform_for(&source) {
                let Some(source_id) = online_source_id(item, &meta, &source) else {
                    report.skipped_invalid += 1;
                    continue;
                };
                let song_id = song_id_from_source(platform, &source_id);
                if let Some(mut existing) = db.get_song(&song_id).await.map_err(|e| e.to_string())? {
                    report.online_existing += 1;
                    if merge_import_metadata(&mut existing, &meta, duration) {
                        db.upsert_song(&existing).await.map_err(|e| e.to_string())?;
                    }
                    upsert_tags(
                        db,
                        existing,
                        &[playlist_tag.clone()],
                        &mut report,
                    )
                    .await?
                } else {
                    let song = Song {
                        id: song_id,
                        title,
                        artist,
                        album,
                        tags: vec![playlist_tag.clone()],
                        sources: vec![Source {
                            platform: platform.to_string(),
                            id: source_id,
                        }],
                        paths: Vec::new(),
                        metadata: SongMetadata {
                            duration,
                            cover_url: text(&meta, "picUrl"),
                            added_time: Some(crate::infra::time::now_timestamp()),
                            ..Default::default()
                        },
                    };
                    db.upsert_song(&song).await.map_err(|e| e.to_string())?;
                    report.online_created += 1;
                    report.tags_added += 1;
                    song.id
                }
            } else {
                report.skipped_invalid += 1;
                continue;
            };

            unique.insert(song_id.clone());
            entries.push(PlaylistEntry {
                song_id,
                import_order: Some((index + 1) as u32),
            });
        }

        db.upsert_playlist(&Playlist {
            id: PlaylistId("lx:".to_string() + &playlist_id),
            name,
            entries,
            source: PlaylistSource::UserCreated,
        })
        .await
        .map_err(|e| e.to_string())?;
        report.playlists_saved += 1;
    }

    report.unique_songs = unique.len() as u64;
    Ok(report)
}

pub async fn import_lx_list_at(db_path: &Path, root: &Value) -> Result<LxImportReport, String> {
    let db = Database::open(db_path)
        .await
        .map_err(|e| format!("open {}: {e}", db_path.display()))?;
    import_lx_list(&db, root).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::db::Database;

    #[tokio::test]
    async fn imports_local_online_and_playlist_tag() {
        let dir = std::env::temp_dir().join(format!(
            "vide-lx-import-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("sample.mp3");
        std::fs::write(&file, b"first-last-chunk").unwrap();
        let data = serde_json::json!({
            "type": "playList_v2",
            "data": [{
                "id": "test",
                "name": "Test",
                "list": [
                    {"id": "local", "name": "Local", "singer": "Artist", "source": "local", "interval": "02:04", "meta": {"filePath": file.to_string_lossy(), "albumName": "Album"}},
                    {"id": "wy_5257138", "name": "Online", "singer": "Artist", "source": "wy", "interval": "03:00", "meta": {"songId": 5257138, "albumName": "Album"}},
                    {"id": "mg_1", "name": "Migu", "singer": "", "source": "mg", "interval": "01:00", "meta": {"songId": 1}}
                ]
            }]
        });

        let db = Database::open_in_memory().await.unwrap();
        let report = import_lx_list(&db, &data).await.unwrap();
        assert_eq!(report.playlists, 1);
        assert_eq!(report.entries, 3);
        assert_eq!(report.unique_songs, 2);
        assert_eq!(report.local_created, 1);
        assert_eq!(report.online_created, 1);
        assert_eq!(report.skipped_mg, 1);
        assert_eq!(report.tags_added, 2);
        assert_eq!(report.skipped_playlists, 0);

        let songs = db.list_library_songs().await.unwrap();
        assert_eq!(songs.len(), 2);
        assert!(songs.iter().all(|song| song.tags.iter().all(|tag| tag.namespace != "source")));
        let playlist = db
            .get_playlist(&PlaylistId("lx:test".to_string()))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(playlist.name, "Test");
        assert_eq!(playlist.entries.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
