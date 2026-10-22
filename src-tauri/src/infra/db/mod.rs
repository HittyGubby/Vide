use sea_orm::{
    sea_query::OnConflict, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    DbErr, EntityTrait, Order, PaginatorTrait, QueryFilter as _, QueryOrder, QuerySelect, Schema,
    Set, Statement,
};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::models::{
    cache::CacheEntry as DomainCacheEntry,
    playlist::{Playlist as DomainPlaylist, PlaylistEntry as DomainPlaylistEntry, PlaylistId, PlaylistSource},
    song::{Song as DomainSong, SongId, Source},
    tag::Tag,
    SongMetadata,
};

pub mod entity;

// ── QueryFilter ────────────────────────────────────────────────────

/// Basic query filter for song queries. Phase 3 will expand this with
/// the full search parser.
#[derive(Debug, Clone, Default)]
pub struct QueryFilter {
    pub order_by: Option<String>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
    pub ids: Option<Vec<SongId>>,
    pub query: Option<String>,
}

// ── Table Initialization ──────────────────────────────────────────

macro_rules! create_table {
    ($db:expr, $schema:expr, $backend:expr, $entity:expr) => {{
        let mut stmt = $schema.create_table_from_entity($entity);
        stmt.if_not_exists();
        $db.execute($backend.build(&stmt)).await?;
    }};
}

async fn ensure_tables(db: &DatabaseConnection) -> Result<(), DbErr> {
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);

    create_table!(db, schema, backend, entity::song::Entity);
    create_table!(db, schema, backend, entity::song_path::Entity);
    create_table!(db, schema, backend, entity::song_source::Entity);
    create_table!(db, schema, backend, entity::tag::Entity);
    create_table!(db, schema, backend, entity::playlist::Entity);
    create_table!(db, schema, backend, entity::playlist_entry::Entity);
    create_table!(db, schema, backend, entity::cache_entry::Entity);
    create_table!(db, schema, backend, entity::setting::Entity);

    Ok(())
}

// ── Database ──────────────────────────────────────────────────────

/// Database wrapper providing an async SQLite connection via SeaORM.
/// `Clone` shares the underlying SeaORM connection pool (which is itself
/// an `Arc`), so cloning is cheap and safe.
#[derive(Clone)]
pub struct Database {
    pub(crate) conn: DatabaseConnection,
}

impl Database {
    /// Open or create the database at the given path, run schema setup, enable WAL mode.
    pub async fn open(path: &Path) -> Result<Self, DbErr> {
        let url = format!("sqlite://{}?mode=rwc", path.display());
        let conn = sea_orm::Database::connect(&url).await?;

        conn.execute(Statement::from_string(
            conn.get_database_backend(),
            "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;".to_owned(),
        ))
        .await?;

        ensure_tables(&conn).await?;

        Ok(Database { conn })
    }

    /// Open an in-memory database (for testing).
    pub async fn open_in_memory() -> Result<Self, DbErr> {
        let conn = sea_orm::Database::connect("sqlite::memory:").await?;
        conn.execute(Statement::from_string(
            conn.get_database_backend(),
            "PRAGMA foreign_keys=ON;".to_owned(),
        ))
        .await?;
        ensure_tables(&conn).await?;
        Ok(Database { conn })
    }

    // ── Songs ─────────────────────────────────────────────────────

    /// Insert or replace a song and all its associated data.
    pub async fn upsert_song(&self, song: &DomainSong) -> Result<(), DbErr> {
        use entity::{song, song_path, song_source, tag};

        let meta_json = serde_json::to_string(&song.metadata).unwrap_or_default();

        // Upsert main song row
        let active = song::ActiveModel {
            song_id: Set(song.id.0.clone()),
            title: Set(song.title.clone()),
            artist: Set(song.artist.clone()),
            album: Set(song.album.clone()),
            duration: Set(song.metadata.duration),
            added_time: Set(song.metadata.added_time.clone()),
            metadata_json: Set(meta_json),
        };

        song::Entity::insert(active)
            .on_conflict(
                OnConflict::columns([song::Column::SongId])
                    .update_columns([
                        song::Column::Title,
                        song::Column::Artist,
                        song::Column::Album,
                        song::Column::Duration,
                        song::Column::AddedTime,
                        song::Column::MetadataJson,
                    ])
                    .to_owned(),
            )
            .exec(&self.conn)
            .await?;

        // Replace paths
        song_path::Entity::delete_many()
            .filter(song_path::Column::SongId.eq(&song.id.0))
            .exec(&self.conn)
            .await?;
        for path in &song.paths {
            let path_str = path.to_string_lossy().to_string();
            // A path belongs to exactly one song. If a file at this path was
            // re-indexed under a new id, move the path instead of leaving two
            // songs claiming it.
            song_path::Entity::delete_many()
                .filter(song_path::Column::Path.eq(&path_str))
                .filter(song_path::Column::SongId.ne(&song.id.0))
                .exec(&self.conn)
                .await?;
            song_path::Entity::insert(song_path::ActiveModel {
                song_id: Set(song.id.0.clone()),
                path: Set(path_str),
            })
            .exec(&self.conn)
            .await?;
        }

        // Replace sources
        song_source::Entity::delete_many()
            .filter(song_source::Column::SongId.eq(&song.id.0))
            .exec(&self.conn)
            .await?;
        for src in &song.sources {
            song_source::Entity::insert(song_source::ActiveModel {
                song_id: Set(song.id.0.clone()),
                platform: Set(src.platform.clone()),
                source_id: Set(src.id.clone()),
            })
            .exec(&self.conn)
            .await?;
        }

        // Replace tags
        tag::Entity::delete_many()
            .filter(tag::Column::SongId.eq(&song.id.0))
            .exec(&self.conn)
            .await?;
        for tag_item in &song.tags {
            tag::Entity::insert(tag::ActiveModel {
                song_id: Set(song.id.0.clone()),
                namespace: Set(tag_item.namespace.clone()),
                value: Set(tag_item.value.clone()),
            })
            .exec(&self.conn)
            .await?;
        }

        Ok(())
    }

    /// Remove path rows whose file no longer exists, then drop songs that have
    /// no paths, no sources and no user tags left.
    ///
    /// This replaces the old "delete every local song before rescanning"
    /// behaviour, which also wiped tags, playlist entries and cache rows for
    /// files that still existed. Songs that only lost a path keep their tags
    /// and can be re-linked when a drive is mounted again.
    ///
    /// Returns a pair: stale paths removed, then empty songs removed.
    pub async fn prune_missing_paths(&self, excludes: &[std::path::PathBuf]) -> Result<(u64, u64), DbErr> {
        use entity::{song_path, song_source};

        let rows = song_path::Entity::find().all(&self.conn).await?;
        let mut removed_paths = 0u64;
        // Songs whose path was removed because the user explicitly excluded
        // it are removed as a whole (including tags); a merely missing drive
        // keeps tagged songs around for re-linking.
        let mut excluded_songs = std::collections::HashSet::new();
        for row in rows {
            let path = Path::new(&row.path);
            let excluded = excludes.iter().any(|exclude| path.starts_with(exclude));
            if excluded || !path.exists() {
                song_path::Entity::delete_many()
                    .filter(song_path::Column::SongId.eq(&row.song_id))
                    .filter(song_path::Column::Path.eq(&row.path))
                    .exec(&self.conn)
                    .await?;
                if excluded {
                    excluded_songs.insert(row.song_id.clone());
                }
                removed_paths += 1;
            }
        }

        let mut removed_ids = std::collections::HashSet::new();
        let mut removed_songs = 0u64;

        // Explicitly excluded songs: remove even when they still have tags.
        for id in excluded_songs {
            let has_path = song_path::Entity::find()
                .filter(song_path::Column::SongId.eq(&id))
                .count(&self.conn)
                .await?
                > 0;
            let has_source = song_source::Entity::find()
                .filter(song_source::Column::SongId.eq(&id))
                .count(&self.conn)
                .await?
                > 0;
            if has_path || has_source {
                continue;
            }
            self.delete_song_cascade(&id).await?;
            removed_ids.insert(id);
            removed_songs += 1;
        }

        // Songs that lost their last path and have no source/tag. One query
        // instead of three count queries per song keeps the startup scan light.
        let statement = Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT song_id FROM songs WHERE NOT EXISTS (SELECT 1 FROM song_paths WHERE song_paths.song_id = songs.song_id) AND NOT EXISTS (SELECT 1 FROM song_sources WHERE song_sources.song_id = songs.song_id) AND NOT EXISTS (SELECT 1 FROM tags WHERE tags.song_id = songs.song_id)".to_string(),
        );
        let orphan_rows = self.conn.query_all(statement).await?;
        for row in orphan_rows {
            let id: String = row.try_get_by_index(0)?;
            if removed_ids.contains(&id) {
                continue;
            }
            self.delete_song_cascade(&id).await?;
            removed_songs += 1;
        }

        Ok((removed_paths, removed_songs))
    }



    /// Delete a song and every association row that points at it.
    async fn delete_song_cascade(&self, id: &str) -> Result<(), DbErr> {
        use entity::{cache_entry, playlist_entry, song, song_path, song_source, tag};
        song_path::Entity::delete_many()
            .filter(song_path::Column::SongId.eq(id))
            .exec(&self.conn)
            .await?;
        song_source::Entity::delete_many()
            .filter(song_source::Column::SongId.eq(id))
            .exec(&self.conn)
            .await?;
        tag::Entity::delete_many()
            .filter(tag::Column::SongId.eq(id))
            .exec(&self.conn)
            .await?;
        playlist_entry::Entity::delete_many()
            .filter(playlist_entry::Column::SongId.eq(id))
            .exec(&self.conn)
            .await?;
        cache_entry::Entity::delete_many()
            .filter(cache_entry::Column::SongId.eq(id))
            .exec(&self.conn)
            .await?;
        song::Entity::delete_many()
            .filter(song::Column::SongId.eq(id))
            .exec(&self.conn)
            .await?;
        Ok(())
    }

    /// Load every local path → song id in one query (used by the scanner to
    /// avoid a database round-trip for every already-indexed file).
    pub async fn all_song_paths(&self) -> Result<HashMap<String, SongId>, DbErr> {
        use entity::song_path;
        let rows = song_path::Entity::find().all(&self.conn).await?;
        Ok(rows
            .into_iter()
            .map(|row| (row.path, SongId(row.song_id)))
            .collect())
    }

    /// Find the song that currently owns a local path.
    pub async fn find_song_id_by_path(&self, path: &str) -> Result<Option<SongId>, DbErr> {
        use entity::song_path;
        let row = song_path::Entity::find()
            .filter(song_path::Column::Path.eq(path))
            .one(&self.conn)
            .await?;
        Ok(row.map(|row| SongId(row.song_id)))
    }

    /// Retrieve a single song by ID, returning None if not found.
    pub async fn get_song(&self, id: &SongId) -> Result<Option<DomainSong>, DbErr> {
        use entity::{song, song_path, song_source, tag};

        let db_song = song::Entity::find()
            .filter(song::Column::SongId.eq(&id.0))
            .one(&self.conn)
            .await?;

        let db_song = match db_song {
            Some(s) => s,
            None => return Ok(None),
        };

        // Load paths
        let paths: Vec<std::path::PathBuf> = song_path::Entity::find()
            .filter(song_path::Column::SongId.eq(&id.0))
            .all(&self.conn)
            .await?
            .into_iter()
            .map(|row| std::path::PathBuf::from(row.path))
            .collect();

        // Load sources
        let sources: Vec<Source> = song_source::Entity::find()
            .filter(song_source::Column::SongId.eq(&id.0))
            .all(&self.conn)
            .await?
            .into_iter()
            .map(|row| Source {
                platform: row.platform,
                id: row.source_id,
            })
            .collect();

        // Load tags
        let tags: Vec<Tag> = tag::Entity::find()
            .filter(tag::Column::SongId.eq(&id.0))
            .all(&self.conn)
            .await?
            .into_iter()
            .map(|row| Tag {
                namespace: row.namespace,
                value: row.value,
            })
            .collect();

        let mut metadata: SongMetadata =
            serde_json::from_str(&db_song.metadata_json).unwrap_or_default();
        if metadata.duration.is_none() {
            metadata.duration = db_song.duration;
        }
        if metadata.added_time.is_none() {
            metadata.added_time = db_song.added_time;
        }

        Ok(Some(DomainSong {
            id: SongId(db_song.song_id),
            title: db_song.title,
            artist: db_song.artist,
            album: db_song.album,
            tags,
            sources,
            paths,
            metadata,
        }))
    }


    /// Load tags, sources and paths for a batch of songs.
    ///
    /// The list/search commands used to return songs with these fields empty
    /// (only `get_song` loaded them), which broke local-file detection in the
    /// UI — "details" saw no paths and treated a local file as an unplayable
    /// online entry. Three batched queries keep this cheap for the full list.
    async fn attach_associations(&self, songs: &mut [DomainSong]) -> Result<(), DbErr> {
        use entity::{song_path, song_source, tag};

        if songs.is_empty() {
            return Ok(());
        }
        let ids: Vec<String> = songs.iter().map(|song| song.id.0.clone()).collect();
        // SQLite has a bound-parameter limit; chunk the id lists so a very
        // large library cannot overflow it.
        const CHUNK: usize = 500;

        let mut paths: HashMap<String, Vec<PathBuf>> = HashMap::new();
        for chunk in ids.chunks(CHUNK) {
            for row in song_path::Entity::find()
                .filter(song_path::Column::SongId.is_in(chunk.to_vec()))
                .all(&self.conn)
                .await?
            {
                paths.entry(row.song_id).or_default().push(PathBuf::from(row.path));
            }
        }

        let mut sources: HashMap<String, Vec<Source>> = HashMap::new();
        for chunk in ids.chunks(CHUNK) {
            for row in song_source::Entity::find()
                .filter(song_source::Column::SongId.is_in(chunk.to_vec()))
                .all(&self.conn)
                .await?
            {
                sources.entry(row.song_id).or_default().push(Source {
                    platform: row.platform,
                    id: row.source_id,
                });
            }
        }

        let mut tags: HashMap<String, Vec<Tag>> = HashMap::new();
        for chunk in ids.chunks(CHUNK) {
            for row in tag::Entity::find()
                .filter(tag::Column::SongId.is_in(chunk.to_vec()))
                .all(&self.conn)
                .await?
            {
                tags.entry(row.song_id).or_default().push(Tag {
                    namespace: row.namespace,
                    value: row.value,
                });
            }
        }

        for song in songs.iter_mut() {
            if let Some(found) = paths.remove(&song.id.0) {
                song.paths = found;
            }
            if let Some(found) = sources.remove(&song.id.0) {
                song.sources = found;
            }
            if let Some(found) = tags.remove(&song.id.0) {
                song.tags = found;
            }
        }
        Ok(())
    }

    /// Query songs with an optional filter.
    pub async fn query_songs(&self, filter: &QueryFilter) -> Result<Vec<DomainSong>, DbErr> {
        use entity::song;

        let mut select = song::Entity::find();

        if let Some(ids) = &filter.ids {
            let id_strs: Vec<&str> = ids.iter().map(|id| id.0.as_str()).collect();
            select = select.filter(song::Column::SongId.is_in(id_strs));
        } else if let Some(q) = &filter.query {
            select = select.filter(
                sea_orm::Condition::any()
                    .add(song::Column::Title.contains(q))
                    .add(song::Column::Artist.contains(q))
                    .add(song::Column::Album.contains(q)),
            );
        }

        // Default ordering
        select = select.order_by(song::Column::AddedTime, Order::Desc);

        if let Some(l) = filter.limit {
            select = select.limit(l);
        }
        if let Some(o) = filter.offset {
            select = select.offset(o);
        }

        let db_songs = select.all(&self.conn).await?;

        let mut songs: Vec<DomainSong> = db_songs
            .into_iter()
            .map(|s| {
                let mut metadata: SongMetadata =
                    serde_json::from_str(&s.metadata_json).unwrap_or_default();
                if metadata.duration.is_none() {
                    metadata.duration = s.duration;
                }
                if metadata.added_time.is_none() {
                    metadata.added_time = s.added_time;
                }
                DomainSong {
                    id: SongId(s.song_id),
                    title: s.title,
                    artist: s.artist,
                    album: s.album,
                    tags: Vec::new(),
                    sources: Vec::new(),
                    paths: Vec::new(),
                    metadata,
                }
            })
            .collect();

        self.attach_associations(&mut songs).await?;
        Ok(songs)
    }


    /// Distinct values of a text-searchable field, for query autocomplete.
    /// Field names are matched against a fixed list; the prefix is always a
    /// bound parameter.
    pub async fn query_field_values(
        &self,
        field: &str,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<String>, DbErr> {
        let sql = match field {
            "id" => "SELECT DISTINCT song_id AS value FROM songs WHERE song_id LIKE ? ORDER BY song_id LIMIT ?",
            "title" => "SELECT DISTINCT title AS value FROM songs WHERE title LIKE ? ORDER BY title LIMIT ?",
            "artist" => "SELECT DISTINCT artist AS value FROM songs WHERE artist LIKE ? ORDER BY artist LIMIT ?",
            "album" => "SELECT DISTINCT album AS value FROM songs WHERE album LIKE ? ORDER BY album LIMIT ?",
            "source" => "SELECT DISTINCT platform AS value FROM song_sources WHERE platform LIKE ? ORDER BY platform LIMIT ?",
            "source_id" => "SELECT DISTINCT source_id AS value FROM song_sources WHERE source_id LIKE ? ORDER BY source_id LIMIT ?",
            "path" => "SELECT DISTINCT path AS value FROM song_paths WHERE path LIKE ? ORDER BY path LIMIT ?",
            "tags" => "SELECT DISTINCT namespace || ':' || value AS value FROM tags WHERE namespace || ':' || value LIKE ? ORDER BY value LIMIT ?",
            "file_name" => "SELECT DISTINCT json_extract(metadata_json, '$.file_name') AS value FROM songs WHERE json_extract(metadata_json, '$.file_name') LIKE ? ORDER BY value LIMIT ?",
            "notes" => "SELECT DISTINCT json_extract(metadata_json, '$.notes') AS value FROM songs WHERE json_extract(metadata_json, '$.notes') LIKE ? ORDER BY value LIMIT ?",
            "added_time" => "SELECT DISTINCT added_time AS value FROM songs WHERE added_time LIKE ? ORDER BY value LIMIT ?",
            _ => return Ok(Vec::new()),
        };
        let like = format!("%{}%", prefix);
        let statement = Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            sql,
            vec![like.into(), (limit as i64).into()],
        );
        let rows = self.conn.query_all(statement).await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| row.try_get_by_index::<String>(0).ok())
            .filter(|value| !value.trim().is_empty())
            .collect())
    }

    /// Distinct values for a tag namespace, for namespace:value autocomplete.
    pub async fn tag_values(
        &self,
        namespace: &str,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<String>, DbErr> {
        let statement = Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT DISTINCT value FROM tags WHERE namespace = ? AND value LIKE ? ORDER BY value LIMIT ?",
            vec![
                namespace.to_string().into(),
                format!("%{}%", prefix).into(),
                (limit as i64).into(),
            ],
        );
        let rows = self.conn.query_all(statement).await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| row.try_get_by_index::<String>(0).ok())
            .collect())
    }

    /// List songs in "My Library" — those with at least one local path
    /// OR at least one user tag. External search results without paths
    /// or tags are excluded.
    pub async fn list_library_songs(&self) -> Result<Vec<DomainSong>, DbErr> {
        use entity::{song, song_path, tag};
        use sea_orm::FromQueryResult;

        #[derive(FromQueryResult)]
        struct IdRow {
            song_id: String,
        }

        let path_ids: Vec<String> = song_path::Entity::find()
            .select_only()
            .column(song_path::Column::SongId)
            .distinct()
            .into_model::<IdRow>()
            .all(&self.conn)
            .await?
            .into_iter()
            .map(|r| r.song_id)
            .collect();

        let tag_ids: Vec<String> = tag::Entity::find()
            .select_only()
            .column(tag::Column::SongId)
            .distinct()
            .into_model::<IdRow>()
            .all(&self.conn)
            .await?
            .into_iter()
            .map(|r| r.song_id)
            .collect();

        let mut all_ids: Vec<String> = Vec::with_capacity(path_ids.len() + tag_ids.len());
        all_ids.extend(path_ids);
        all_ids.extend(tag_ids);
        all_ids.sort();
        all_ids.dedup();

        if all_ids.is_empty() {
            return Ok(Vec::new());
        }

        let db_songs = song::Entity::find()
            .filter(song::Column::SongId.is_in(all_ids))
            .order_by(song::Column::AddedTime, Order::Desc)
            .all(&self.conn)
            .await?;

        let mut songs: Vec<DomainSong> = db_songs
            .into_iter()
            .map(|s| {
                let mut metadata: SongMetadata =
                    serde_json::from_str(&s.metadata_json).unwrap_or_default();
                if metadata.duration.is_none() {
                    metadata.duration = s.duration;
                }
                if metadata.added_time.is_none() {
                    metadata.added_time = s.added_time;
                }
                DomainSong {
                    id: SongId(s.song_id),
                    title: s.title,
                    artist: s.artist,
                    album: s.album,
                    tags: Vec::new(),
                    sources: Vec::new(),
                    paths: Vec::new(),
                    metadata,
                }
            })
            .collect();

        self.attach_associations(&mut songs).await?;
        Ok(songs)
    }

    // ── Playlists ─────────────────────────────────────────────────

    /// Save a playlist (insert or replace).
    pub async fn upsert_playlist(&self, playlist: &DomainPlaylist) -> Result<(), DbErr> {
        use entity::{playlist, playlist_entry};

        let (source_type, source_path) = match &playlist.source {
            PlaylistSource::M3U { file_path } => {
                ("M3U".to_string(), Some(file_path.to_string_lossy().to_string()))
            }
            PlaylistSource::Online { url } => {
                ("Online".to_string(), Some(url.clone()))
            }
            PlaylistSource::UserCreated => ("UserCreated".to_string(), None),
        };

        playlist::Entity::insert(playlist::ActiveModel {
            playlist_id: Set(playlist.id.0.clone()),
            name: Set(playlist.name.clone()),
            source_type: Set(source_type),
            source_path: Set(source_path),
        })
        .on_conflict(
            OnConflict::columns([playlist::Column::PlaylistId])
                .update_columns([
                    playlist::Column::Name,
                    playlist::Column::SourceType,
                    playlist::Column::SourcePath,
                ])
                .to_owned(),
        )
        .exec(&self.conn)
        .await?;

        // Replace entries
        playlist_entry::Entity::delete_many()
            .filter(playlist_entry::Column::PlaylistId.eq(&playlist.id.0))
            .exec(&self.conn)
            .await?;
        for entry in &playlist.entries {
            playlist_entry::Entity::insert(playlist_entry::ActiveModel {
                playlist_id: Set(playlist.id.0.clone()),
                song_id: Set(entry.song_id.0.clone()),
                import_order: Set(entry.import_order.map(|o| o as i32)),
            })
            .exec(&self.conn)
            .await?;
        }

        Ok(())
    }

    /// Get a playlist by ID.
    pub async fn get_playlist(&self, id: &PlaylistId) -> Result<Option<DomainPlaylist>, DbErr> {
        use entity::{playlist, playlist_entry};

        let db_pl = playlist::Entity::find()
            .filter(playlist::Column::PlaylistId.eq(&id.0))
            .one(&self.conn)
            .await?;

        let db_pl = match db_pl {
            Some(p) => p,
            None => return Ok(None),
        };

        let entries = playlist_entry::Entity::find()
            .filter(playlist_entry::Column::PlaylistId.eq(&db_pl.playlist_id))
            .order_by(playlist_entry::Column::ImportOrder, Order::Asc)
            .all(&self.conn)
            .await?
            .into_iter()
            .map(|e| DomainPlaylistEntry {
                song_id: SongId(e.song_id),
                import_order: e.import_order.map(|o| o as u32),
            })
            .collect();

        let source = match db_pl.source_type.as_str() {
            "M3U" => PlaylistSource::M3U {
                file_path: db_pl.source_path.map(std::path::PathBuf::from).unwrap_or_default(),
            },
            "Online" => PlaylistSource::Online {
                url: db_pl.source_path.clone().unwrap_or_default(),
            },
            _ => PlaylistSource::UserCreated,
        };

        Ok(Some(DomainPlaylist {
            id: PlaylistId(db_pl.playlist_id),
            name: db_pl.name,
            entries,
            source,
        }))
    }

    // ── Playlist listing ───────────────────────────────────────────

    /// List all playlists (without their entries — use get_playlist for details).
    pub async fn list_playlists(&self) -> Result<Vec<DomainPlaylist>, DbErr> {
        use entity::playlist;

        let db_pls = playlist::Entity::find()
            .all(&self.conn)
            .await?;

        Ok(db_pls
            .into_iter()
            .map(|db_pl| {
                let source = match db_pl.source_type.as_str() {
                    "M3U" => PlaylistSource::M3U {
                        file_path: db_pl
                            .source_path
                            .map(std::path::PathBuf::from)
                            .unwrap_or_default(),
                    },
                    "Online" => PlaylistSource::Online {
                        url: db_pl.source_path.clone().unwrap_or_default(),
                    },
                    _ => PlaylistSource::UserCreated,
                };
                DomainPlaylist {
                    id: PlaylistId(db_pl.playlist_id),
                    name: db_pl.name,
                    entries: Vec::new(),
                    source,
                }
            })
            .collect())
    }

    // ── Cache ─────────────────────────────────────────────────────

    /// Insert or replace a cache entry.
    pub async fn upsert_cache_entry(&self, entry: &DomainCacheEntry) -> Result<(), DbErr> {
        use entity::cache_entry;

        cache_entry::Entity::insert(cache_entry::ActiveModel {
            song_id: Set(entry.song_id.0.clone()),
            path: Set(entry.path.to_string_lossy().to_string()),
            size: Set(entry.size as i64),
            source: Set(entry.source.clone()),
            last_used: Set(entry.last_used as i64),
            valid: Set(if entry.valid { 1 } else { 0 }),
        })
        .on_conflict(
            OnConflict::columns([cache_entry::Column::SongId, cache_entry::Column::Path])
                .update_columns([
                    cache_entry::Column::Size,
                    cache_entry::Column::Source,
                    cache_entry::Column::LastUsed,
                    cache_entry::Column::Valid,
                ])
                .to_owned(),
        )
        .exec(&self.conn)
        .await?;

        Ok(())
    }

    /// Get cache entries for a song.
    pub async fn get_cache_entries(&self, song_id: &SongId) -> Result<Vec<DomainCacheEntry>, DbErr> {
        use entity::cache_entry;

        let entries = cache_entry::Entity::find()
            .filter(cache_entry::Column::SongId.eq(&song_id.0))
            .all(&self.conn)
            .await?
            .into_iter()
            .map(|e| DomainCacheEntry {
                song_id: SongId(e.song_id),
                path: std::path::PathBuf::from(e.path),
                size: e.size as u64,
                source: e.source,
                last_used: e.last_used as u64,
                valid: e.valid != 0,
            })
            .collect();

        Ok(entries)
    }

    // ── Settings ──────────────────────────────────────────────────

    /// Update a setting value. Value is serialized as JSON.
    pub async fn update_setting(&self, key: &str, value: &Value) -> Result<(), DbErr> {
        use entity::setting;

        let val_str = serde_json::to_string(value).unwrap_or_default();

        setting::Entity::insert(setting::ActiveModel {
            key: Set(key.to_string()),
            value: Set(val_str),
        })
        .on_conflict(
            OnConflict::columns([setting::Column::Key])
                .update_columns([setting::Column::Value])
                .to_owned(),
        )
        .exec(&self.conn)
        .await?;

        Ok(())
    }

    /// Get a setting value, deserialized from JSON.
    pub async fn get_setting(&self, key: &str) -> Result<Option<Value>, DbErr> {
        use entity::setting;

        let result = setting::Entity::find()
            .filter(setting::Column::Key.eq(key))
            .one(&self.conn)
            .await?;

        match result {
            Some(row) => Ok(serde_json::from_str(&row.value).ok()),
            None => Ok(None),
        }
    }

    // ── Cache bulk helpers ────────────────────────────────────────

    /// Delete all cache entries for a song (regardless of path).
    pub async fn delete_cache_entry(&self, song_id: &SongId) -> Result<(), DbErr> {
        use entity::cache_entry;
        cache_entry::Entity::delete_many()
            .filter(cache_entry::Column::SongId.eq(&song_id.0))
            .exec(&self.conn)
            .await?;
        Ok(())
    }

    /// List all cache entries (all sources, all songs).
    pub async fn list_cache_entries(&self) -> Result<Vec<DomainCacheEntry>, DbErr> {
        use entity::cache_entry;
        let entries = cache_entry::Entity::find()
            .all(&self.conn)
            .await?
            .into_iter()
            .map(|e| DomainCacheEntry {
                song_id: SongId(e.song_id),
                path: std::path::PathBuf::from(e.path),
                size: e.size as u64,
                source: e.source,
                last_used: e.last_used as u64,
                valid: e.valid != 0,
            })
            .collect();
        Ok(entries)
    }

    /// List valid cache entries sorted by last_used ascending (LRU first).
    pub async fn list_cache_entries_lru(&self) -> Result<Vec<DomainCacheEntry>, DbErr> {
        use entity::cache_entry;
        let entries = cache_entry::Entity::find()
            .filter(cache_entry::Column::Valid.eq(1))
            .order_by(cache_entry::Column::LastUsed, Order::Asc)
            .all(&self.conn)
            .await?
            .into_iter()
            .map(|e| DomainCacheEntry {
                song_id: SongId(e.song_id),
                path: std::path::PathBuf::from(e.path),
                size: e.size as u64,
                source: e.source,
                last_used: e.last_used as u64,
                valid: e.valid != 0,
            })
            .collect();
        Ok(entries)
    }

    /// Drop every cache entry (used by "clear audio cache").
    pub async fn delete_all_cache_entries(&self) -> Result<u64, DbErr> {
        use entity::cache_entry;
        let res = cache_entry::Entity::delete_many().exec(&self.conn).await?;
        Ok(res.rows_affected)
    }

    /// Raw `metadata_json` for every song — enough to count the text caches
    /// without loading tags, sources and paths for the whole library.
    pub async fn list_metadata_json(&self) -> Result<Vec<String>, DbErr> {
        use entity::song;
        let rows = song::Entity::find()
            .select_only()
            .column(song::Column::MetadataJson)
            .into_tuple::<(String,)>()
            .all(&self.conn)
            .await?;
        Ok(rows.into_iter().map(|(json,)| json).collect())
    }

    /// Delete all invalid cache entries from the database.
    pub async fn delete_invalid_cache_entries(&self) -> Result<(), DbErr> {
        use entity::cache_entry;
        cache_entry::Entity::delete_many()
            .filter(cache_entry::Column::Valid.eq(0))
            .exec(&self.conn)
            .await?;
        Ok(())
    }

    // ── Search Presets ─────────────────────────────────────────────

    const SEARCH_PRESETS_KEY: &'static str = "search_presets";

    /// List all saved search presets, in insertion order.
    pub async fn list_search_presets(&self) -> Result<Vec<crate::models::SearchPreset>, DbErr> {
        match self.get_setting(Self::SEARCH_PRESETS_KEY).await? {
            Some(val) => Ok(serde_json::from_value(val).unwrap_or_default()),
            None => Ok(Vec::new()),
        }
    }

    /// Replace the full set of search presets.
    pub async fn save_search_presets(
        &self,
        presets: &[crate::models::SearchPreset],
    ) -> Result<(), DbErr> {
        self.update_setting(Self::SEARCH_PRESETS_KEY, &serde_json::to_value(presets).unwrap())
            .await
    }

    /// Get the ID of the last-selected search preset, if any.
    pub async fn get_last_search_preset(&self) -> Result<Option<String>, DbErr> {
        match self.get_setting("last_search_preset").await? {
            Some(val) => Ok(val.as_str().map(str::to_string)),
            None => Ok(None),
        }
    }

    /// Persist the last-selected search preset ID.
    pub async fn set_last_search_preset(&self, id: &str) -> Result<(), DbErr> {
        self.update_setting("last_search_preset", &serde_json::json!(id)).await
    }

    // ── App Settings ──────────────────────────────────────────────

    const SETTINGS_KEY: &'static str = "video_settings";

    /// Load the full app settings, merging stored value over defaults.
    pub async fn load_app_settings(&self) -> Result<crate::models::AppSettings, DbErr> {
        let defaults = crate::models::AppSettings::default();
        match self.get_setting(Self::SETTINGS_KEY).await? {
            Some(val) => {
                let mut s: crate::models::AppSettings = serde_json::from_value(val).unwrap_or_default();
                // Backfill any fields missing from older saved data.
                if s.font_family.is_empty() {
                    s.font_family = defaults.font_family;
                }
                if s.desktop_lyrics_position.is_empty() {
                    s.desktop_lyrics_position = defaults.desktop_lyrics_position;
                }
                Ok(s)
            }
            None => Ok(defaults),
        }
    }

    /// Persist the full app settings object.
    pub async fn save_app_settings(&self, settings: &crate::models::AppSettings) -> Result<(), DbErr> {
        self.update_setting(Self::SETTINGS_KEY, &serde_json::to_value(settings).unwrap())
            .await
    }

    // ── Playback memory (remember progress / last played song) ──────

    const LAST_PLAYED_KEY: &'static str = "last_played";

    /// Save a snapshot of the last-played song and its position (seconds).
    pub async fn save_last_played(&self, song_id: &str, position_secs: f64) -> Result<(), DbErr> {
        let value = serde_json::json!({ "song_id": song_id, "position": position_secs });
        self.update_setting(Self::LAST_PLAYED_KEY, &value).await
    }

    /// Load the last-played snapshot, if any.
    pub async fn load_last_played(&self) -> Result<Option<(String, f64)>, DbErr> {
        match self.get_setting(Self::LAST_PLAYED_KEY).await? {
            Some(val) => {
                let song_id = val.get("song_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let position = val.get("position").and_then(|v| v.as_f64()).unwrap_or(0.0);
                if song_id.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some((song_id, position)))
                }
            }
            None => Ok(None),
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SongMetadata;

    #[tokio::test]
    async fn test_upsert_and_get_song() {
        let db = Database::open_in_memory().await.unwrap();

        let song = DomainSong {
            id: SongId("test123".to_string()),
            title: "Test Song".to_string(),
            artist: "Test Artist".to_string(),
            album: "Test Album".to_string(),
            tags: vec![Tag {
                namespace: "genre".to_string(),
                value: "pop".to_string(),
            }],
            sources: vec![Source {
                platform: "youtube".to_string(),
                id: "abc123".to_string(),
            }],
            paths: vec![std::path::PathBuf::from("/music/test.mp3")],
            metadata: SongMetadata::default(),
        };

        db.upsert_song(&song).await.unwrap();
        let loaded = db
            .get_song(&SongId("test123".to_string()))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(loaded.id, song.id);
        assert_eq!(loaded.title, "Test Song");
        assert_eq!(loaded.tags.len(), 1);
        assert_eq!(loaded.sources.len(), 1);
        assert_eq!(loaded.paths.len(), 1);
    }

    #[tokio::test]
    async fn test_query_and_library_lists_attach_associations() {
        let db = Database::open_in_memory().await.unwrap();
        let song = DomainSong {
            id: SongId("assoc1".to_string()),
            title: "Local Track".to_string(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            tags: vec![Tag {
                namespace: "mood".to_string(),
                value: "dark".to_string(),
            }],
            sources: vec![Source {
                platform: "local".to_string(),
                id: "assoc1".to_string(),
            }],
            paths: vec![std::path::PathBuf::from("/music/assoc1.ogg")],
            metadata: SongMetadata::default(),
        };
        db.upsert_song(&song).await.unwrap();

        for songs in [
            db.query_songs(&QueryFilter::default()).await.unwrap(),
            db.list_library_songs().await.unwrap(),
        ] {
            assert_eq!(songs.len(), 1);
            assert_eq!(songs[0].paths, song.paths);
            assert_eq!(songs[0].sources.len(), 1);
            assert_eq!(songs[0].tags.len(), 1);
        }
    }

    #[tokio::test]
    async fn test_path_belongs_to_one_song() {
        let db = Database::open_in_memory().await.unwrap();
        let path = std::path::PathBuf::from("/music/shared.mp3");
        let make = |id: &str| DomainSong {
            id: SongId(id.to_string()),
            title: id.to_string(),
            artist: String::new(),
            album: String::new(),
            tags: vec![],
            sources: vec![],
            paths: vec![path.clone()],
            metadata: SongMetadata::default(),
        };
        db.upsert_song(&make("old")).await.unwrap();
        db.upsert_song(&make("new")).await.unwrap();

        assert_eq!(
            db.find_song_id_by_path("/music/shared.mp3").await.unwrap(),
            Some(SongId("new".to_string()))
        );
        let old = db.get_song(&SongId("old".to_string())).await.unwrap().unwrap();
        assert!(old.paths.is_empty());
    }

    #[tokio::test]
    async fn test_prune_missing_paths_keeps_metadata() {
        let db = Database::open_in_memory().await.unwrap();

        let existing_path = std::env::temp_dir().join(format!(
            "vide-prune-test-{}.mp3",
            std::process::id()
        ));
        std::fs::write(&existing_path, b"not really audio").unwrap();

        let make_song = |id: &str, paths: Vec<std::path::PathBuf>, tags: Vec<Tag>| DomainSong {
            id: SongId(id.to_string()),
            title: id.to_string(),
            artist: String::new(),
            album: String::new(),
            tags,
            sources: vec![],
            paths,
            metadata: SongMetadata::default(),
        };

        db.upsert_song(&make_song("keep", vec![existing_path.clone()], vec![]))
            .await
            .unwrap();
        db.upsert_song(&make_song(
            "orphan",
            vec![std::path::PathBuf::from("/definitely/missing/orphan.mp3")],
            vec![],
        ))
        .await
        .unwrap();
        db.upsert_song(&make_song(
            "tagged-ghost",
            vec![std::path::PathBuf::from("/definitely/missing/ghost.mp3")],
            vec![Tag {
                namespace: "mood".to_string(),
                value: "dark".to_string(),
            }],
        ))
        .await
        .unwrap();

        let (paths_removed, songs_removed) = db.prune_missing_paths(&[]).await.unwrap();
        assert_eq!(paths_removed, 2);
        assert_eq!(songs_removed, 1);

        let keep = db.get_song(&SongId("keep".to_string())).await.unwrap().unwrap();
        assert_eq!(keep.paths, vec![existing_path.clone()]);

        assert!(db
            .get_song(&SongId("orphan".to_string()))
            .await
            .unwrap()
            .is_none());

        let ghost = db
            .get_song(&SongId("tagged-ghost".to_string()))
            .await
            .unwrap()
            .unwrap();
        assert!(ghost.paths.is_empty());
        assert_eq!(ghost.tags.len(), 1);

        let _ = std::fs::remove_file(existing_path);
    }

    #[tokio::test]
    async fn test_excluded_paths_remove_tagged_songs() {
        let db = Database::open_in_memory().await.unwrap();
        let song = DomainSong {
            id: SongId("excluded".to_string()),
            title: "Excluded".to_string(),
            artist: String::new(),
            album: String::new(),
            tags: vec![Tag {
                namespace: "mood".to_string(),
                value: "dark".to_string(),
            }],
            sources: vec![],
            paths: vec![std::path::PathBuf::from(
                "/tmp/vide-excluded-dir/song.mp3",
            )],
            metadata: SongMetadata::default(),
        };
        db.upsert_song(&song).await.unwrap();

        let (paths, songs) = db
            .prune_missing_paths(&[std::path::PathBuf::from("/tmp/vide-excluded-dir")])
            .await
            .unwrap();
        assert_eq!(paths, 1);
        assert_eq!(songs, 1);
        assert!(db
            .get_song(&SongId("excluded".to_string()))
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn test_field_and_tag_value_queries() {
        let db = Database::open_in_memory().await.unwrap();
        let song = DomainSong {
            id: SongId("values1".to_string()),
            title: "Hello World".to_string(),
            artist: "Björk".to_string(),
            album: "Album".to_string(),
            tags: vec![Tag {
                namespace: "genre".to_string(),
                value: "Folk".to_string(),
            }],
            sources: vec![],
            paths: vec![],
            metadata: SongMetadata::default(),
        };
        db.upsert_song(&song).await.unwrap();

        let titles = db.query_field_values("title", "Hello", 10).await.unwrap();
        assert_eq!(titles, vec!["Hello World"]);

        let artists = db.query_field_values("artist", "jörk", 10).await.unwrap();
        assert_eq!(artists, vec!["Björk"]);

        let tags = db.tag_values("genre", "fo", 10).await.unwrap();
        assert_eq!(tags, vec!["Folk"]);
    }

    #[tokio::test]
    async fn test_get_song_not_found() {
        let db = Database::open_in_memory().await.unwrap();
        let result = db.get_song(&SongId("nonexistent".to_string())).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_settings_roundtrip() {
        let db = Database::open_in_memory().await.unwrap();
        let val = serde_json::json!({"key": "value"});

        db.update_setting("test_key", &val).await.unwrap();
        let loaded = db.get_setting("test_key").await.unwrap().unwrap();
        assert_eq!(loaded, val);
    }

    #[tokio::test]
    async fn test_setting_not_found() {
        let db = Database::open_in_memory().await.unwrap();
        let result = db.get_setting("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_query_songs_with_text_search() {
        let db = Database::open_in_memory().await.unwrap();

        let song1 = DomainSong {
            id: SongId("s1".to_string()),
            title: "Hello World".to_string(),
            artist: "Artist A".to_string(),
            album: "Album X".to_string(),
            tags: vec![],
            sources: vec![],
            paths: vec![],
            metadata: SongMetadata::default(),
        };
        let song2 = DomainSong {
            id: SongId("s2".to_string()),
            title: "Goodbye World".to_string(),
            artist: "Artist B".to_string(),
            album: "Album Y".to_string(),
            tags: vec![],
            sources: vec![],
            paths: vec![],
            metadata: SongMetadata::default(),
        };

        db.upsert_song(&song1).await.unwrap();
        db.upsert_song(&song2).await.unwrap();

        let filter = QueryFilter {
            query: Some("Hello".to_string()),
            ..Default::default()
        };
        let results = db.query_songs(&filter).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id.0, "s1");
    }

    #[tokio::test]
    async fn test_upsert_and_get_playlist() {
        let db = Database::open_in_memory().await.unwrap();

        let playlist = DomainPlaylist {
            id: PlaylistId("pl1".to_string()),
            name: "My Playlist".to_string(),
            entries: vec![DomainPlaylistEntry {
                song_id: SongId("s1".to_string()),
                import_order: Some(1),
            }],
            source: PlaylistSource::UserCreated,
        };

        db.upsert_playlist(&playlist).await.unwrap();
        let loaded = db
            .get_playlist(&PlaylistId("pl1".to_string()))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(loaded.name, "My Playlist");
        assert_eq!(loaded.entries.len(), 1);
    }

    #[tokio::test]
    async fn test_search_presets_roundtrip() {
        let db = Database::open_in_memory().await.unwrap();

        let presets = vec![
            crate::models::SearchPreset {
                id: "preset_1".to_string(),
                name: "Russian Folk".to_string(),
                query: "country:russia AND genre:folk".to_string(),
            },
            crate::models::SearchPreset {
                id: "preset_2".to_string(),
                name: "Pop".to_string(),
                query: "genre:pop".to_string(),
            },
        ];

        db.save_search_presets(&presets).await.unwrap();
        let loaded = db.list_search_presets().await.unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, "Russian Folk");
        assert_eq!(loaded[1].query, "genre:pop");

        // Last-selected round-trip
        assert!(db.get_last_search_preset().await.unwrap().is_none());
        db.set_last_search_preset("preset_1").await.unwrap();
        assert_eq!(
            db.get_last_search_preset().await.unwrap().unwrap(),
            "preset_1"
        );
    }
}
