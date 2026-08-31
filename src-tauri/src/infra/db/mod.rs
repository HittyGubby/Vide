use sea_orm::{
    sea_query::OnConflict, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr,
    EntityTrait, Order, QueryFilter as _, QueryOrder, QuerySelect, Schema, Set, Statement,
};
use serde_json::Value;
use std::path::Path;

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
            song_path::Entity::insert(song_path::ActiveModel {
                song_id: Set(song.id.0.clone()),
                path: Set(path.to_string_lossy().to_string()),
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

    /// Delete every song that has at least one local path.
    ///
    /// Used by the full-reshuffle library scan: the scan *replaces* the local
    /// result set rather than merging with stale entries, so songs that no
    /// longer exist on disk are removed too. Online-only songs (imported) are
    /// kept.
    pub async fn clear_local_songs(&self) -> Result<u64, DbErr> {
        use entity::{song, song_path, song_source, tag, cache_entry, playlist_entry};

        // Collect the ids that have a local path.
        let rows = song_path::Entity::find()
            .select_only()
            .column(song_path::Column::SongId)
            .into_tuple::<String>()
            .all(&self.conn)
            .await?;
        if rows.is_empty() {
            return Ok(0);
        }

        let ids: Vec<String> = rows;
        for id in &ids {
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
            cache_entry::Entity::delete_many()
                .filter(cache_entry::Column::SongId.eq(id))
                .exec(&self.conn)
                .await?;
            playlist_entry::Entity::delete_many()
                .filter(playlist_entry::Column::SongId.eq(id))
                .exec(&self.conn)
                .await?;
            song::Entity::delete_many()
                .filter(song::Column::SongId.eq(id))
                .exec(&self.conn)
                .await?;
        }
        Ok(ids.len() as u64)
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

        let songs: Vec<DomainSong> = db_songs
            .into_iter()
            .map(|s| {
                let metadata = serde_json::from_str(&s.metadata_json).unwrap_or_default();
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

        Ok(songs)
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

        Ok(db_songs
            .into_iter()
            .map(|s| {
                let metadata =
                    serde_json::from_str(&s.metadata_json).unwrap_or_default();
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
            .collect())
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
