use std::io::{self, Read};
use std::path::{Path, PathBuf};

use tokio::sync::{oneshot, OwnedSemaphorePermit};
use tracing::warn;

use crate::infra::db::Database;
use crate::infra::fs::ensure_cache_dir;
use crate::models::cache::CacheEntry;
use crate::models::song::SongId;

/// Manages on-disk media cache.
///
/// Media files are stored as `cache/<song_id>.media`.
/// Metadata (size, source, last_used, valid) is tracked in SQLite.
#[derive(Clone)]
pub struct CacheManager {
    db: Database,
    cache_dir: PathBuf,
}

/// Handle returned when a pipe stream starts writing into the cache.
pub struct StartedStream {
    pub path: PathBuf,
    /// Resolves after the first bytes are written, so playback can start on a
    /// non-empty file. The download task keeps running after this.
    pub ready: oneshot::Receiver<()>,
}

impl CacheManager {
    /// Create a new CacheManager rooted at `base/cache/`.
    /// The cache directory is created if it doesn't exist.
    pub async fn new(db: Database, base: &Path) -> io::Result<Self> {
        let cache_dir = ensure_cache_dir(base)?;
        Ok(CacheManager { db, cache_dir })
    }

    /// Return the underlying database reference.
    pub fn db(&self) -> &Database {
        &self.db
    }

    /// Path to the cached media file for a given song.
    fn media_path(&self, song_id: &SongId) -> PathBuf {
        self.cache_dir.join(format!("{}.media", song_id.0))
    }

    /// Write media data into the cache and create a DB entry.
    ///
    /// The data stream is consumed entirely. On success returns the
    /// path to the cached file.
    pub async fn cache_media(
        &self,
        song_id: &SongId,
        mut data: impl Read,
        source: &str,
    ) -> io::Result<PathBuf> {
        let dest = self.media_path(song_id);

        // Write to a temp file first, then rename to avoid partial writes
        let tmp = dest.with_extension("tmp");
        {
            let mut out = std::fs::File::create(&tmp)?;
            let copied = io::copy(&mut data, &mut out)?;

            use std::time::SystemTime;
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let entry = CacheEntry {
                song_id: song_id.clone(),
                path: dest.clone(),
                size: copied,
                source: source.to_string(),
                last_used: now,
                valid: true,
            };

            self.db
                .upsert_cache_entry(&entry)
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        }

        std::fs::rename(&tmp, &dest)?;
        Ok(dest)
    }

    /// Check if a song is cached and the entry is still valid.
    /// Returns the path to the cached file when available.
    pub async fn get_cached(&self, song_id: &SongId) -> io::Result<Option<PathBuf>> {
        let entries = self
            .db
            .get_cache_entries(song_id)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        if let Some(entry) = entries.into_iter().find(|e| e.valid) {
            // A cache entry is only useful if the file exists, is non-empty,
            // and still has the size recorded when it was written. This makes
            // "download" fall back to a fresh request for truncated or
            // half-written cache files instead of registering them as local.
            let stat = std::fs::metadata(&entry.path).ok();
            let complete = stat
                .as_ref()
                .map(|meta| {
                    meta.is_file()
                        && meta.len() > 0
                        && (entry.size == 0 || meta.len() == entry.size)
                })
                .unwrap_or(false);
            if complete {
                self.touch(&entry).await;
                return Ok(Some(entry.path));
            }
            self.invalidate(song_id).await;
        }

        Ok(None)
    }

    /// Start a pipe download that writes directly to the final cache path.
    ///
    /// The returned receiver resolves after the first bytes are on disk. The
    /// task keeps downloading after the caller starts playback; it does not
    /// need to be killed when the user pauses or switches songs.
    pub async fn start_pipe_stream(
        &self,
        song_id: &SongId,
        cmd: &[String],
        source: &str,
        permit: Option<OwnedSemaphorePermit>,
    ) -> io::Result<StartedStream> {
        if cmd.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "empty pipe cmd",
            ));
        }
        let dest = self.media_path(song_id);
        let mut child = tokio::process::Command::new(&cmd[0])
            .args(&cmd[1..])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("spawn: {e}")))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "no stdout"))?;

        let (ready_tx, ready_rx) = oneshot::channel();
        let db = self.db.clone();
        let song = song_id.clone();
        let source = source.to_string();
        let dest_for_task = dest.clone();

        tokio::spawn(async move {
            let _permit = permit;
            use tokio::io::{AsyncReadExt, AsyncWriteExt};

            let mut output = match tokio::fs::File::create(&dest_for_task).await {
                Ok(file) => tokio::io::BufWriter::new(file),
                Err(_) => return,
            };
            let mut reader = tokio::io::BufReader::new(stdout);
            let mut buf = vec![0u8; 64 * 1024];
            let mut total: u64 = 0;
            let mut ready_tx = Some(ready_tx);

            loop {
                let n = match reader.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(_) => break,
                };
                if output.write_all(&buf[..n]).await.is_err() {
                    break;
                }
                total += n as u64;
                if let Some(tx) = ready_tx.take() {
                    let _ = output.flush().await;
                    let _ = tx.send(());
                }
            }
            let _ = output.flush().await;
            drop(reader);

            let status = child.wait().await;
            let ok = status.map(|status| status.success()).unwrap_or(false);
            if !ok || total == 0 {
                return;
            }

            use std::time::SystemTime;
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let entry = CacheEntry {
                song_id: song,
                path: dest_for_task,
                size: total,
                source,
                last_used: now,
                valid: true,
            };
            let _ = db.upsert_cache_entry(&entry).await;
        });

        Ok(StartedStream {
            path: dest,
            ready: ready_rx,
        })
    }

    /// Download a plugin stream descriptor's bytes into the cache.
    ///
    /// Handles both descriptor types:
    /// - `url_stream`: fetch via HTTP (reqwest)
    /// - `stream` (pipe): spawn the process and read stdout
    ///
    /// Returns the cached file path on success.
    pub async fn download_from_descriptor(
        &self,
        song_id: &SongId,
        descriptor: &crate::features::plugin::types::StreamDescriptor,
    ) -> io::Result<PathBuf> {
        match descriptor {
            crate::features::plugin::types::StreamDescriptor::UrlStream {
                id, url, headers, ..
            } => {
                // Fetch the URL and stream bytes into the cache.
                let client = reqwest::Client::new();
                let mut req = client.get(url);
                if let Some(hdrs) = headers {
                    for (k, v) in hdrs {
                        req = req.header(k.as_str(), v.as_str());
                    }
                }
                let resp = req
                    .send()
                    .await
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("fetch {url}: {e}")))?;
                if !resp.status().is_success() {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("fetch {url}: HTTP {}", resp.status()),
                    ));
                }
                let bytes = resp
                    .bytes()
                    .await
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("read body: {e}")))?;
                self.cache_media(song_id, &bytes[..], id).await
            }
            crate::features::plugin::types::StreamDescriptor::Pipe { id, cmd } => {
                let dest = self.media_path(song_id);
                let tmp = dest.with_extension("tmp");

                if cmd.is_empty() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "empty pipe cmd",
                    ));
                }
                let mut child = tokio::process::Command::new(&cmd[0])
                    .args(&cmd[1..])
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("spawn: {e}")))?;

                let stdout = child
                    .stdout
                    .take()
                    .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "no stdout"))?;

                let mut writer = tokio::io::BufWriter::new(
                    tokio::fs::File::create(&tmp).await?,
                );
                use tokio::io::AsyncWriteExt;
                use tokio::io::AsyncReadExt;
                let mut reader = tokio::io::BufReader::new(stdout);
                let mut buf = vec![0u8; 64 * 1024];
                let mut total: u64 = 0;
                loop {
                    let n = reader.read(&mut buf).await?;
                    if n == 0 {
                        break;
                    }
                    writer.write_all(&buf[..n]).await?;
                    total += n as u64;
                }
                writer.flush().await?;
                drop(reader);
                let _ = child.wait().await;

                use std::time::SystemTime;
                let now = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let entry = CacheEntry {
                    song_id: song_id.clone(),
                    path: dest.clone(),
                    size: total,
                    source: id.clone(),
                    last_used: now,
                    valid: true,
                };
                self.db
                    .upsert_cache_entry(&entry)
                    .await
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

                std::fs::rename(&tmp, &dest)?;
                Ok(dest)
            }
        }
    }

    /// Mark a cache entry as invalid (stale or corrupted).
    /// Does not delete the file — the eviction cycle handles that.
    pub async fn invalidate(&self, song_id: &SongId) {
        let entries = self
            .db
            .get_cache_entries(song_id)
            .await;

        if let Ok(entries) = entries {
            for entry in entries {
                let mut e = entry;
                e.valid = false;
                let _ = self
                    .db
                    .upsert_cache_entry(&e)
                    .await
                    .map_err(|e| warn!("failed to update cache entry: {e}"));
            }
        }
    }

    /// Remove a cached file and its database entry.
    pub async fn remove(&self, song_id: &SongId) -> io::Result<()> {
        let path = self.media_path(song_id);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("tmp"));

        self.db
            .delete_cache_entry(song_id)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
    }

    /// Update `last_used` for a cache entry.
    async fn touch(&self, entry: &CacheEntry) {
        use std::time::SystemTime;
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut e = entry.clone();
        e.last_used = now;
        let _ = self
            .db
            .upsert_cache_entry(&e)
            .await
            .map_err(|e| warn!("failed to touch cache entry: {e}"));
    }

    /// Evict cache entries until total size is ≤ `target_bytes`.
    ///
    /// Eviction order: invalid entries first, then LRU among valid entries.
    /// Returns the number of bytes freed.
    pub async fn evict(&self, target_bytes: u64) -> io::Result<u64> {
        let entries = self
            .db
            .list_cache_entries()
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        let current_total: u64 = entries.iter().map(|e| e.size).sum();
        if current_total <= target_bytes {
            return Ok(0);
        }

        let mut freed: u64 = 0;
        let to_free = current_total - target_bytes;

        // Phase 1: remove invalid entries (delete files + DB records)
        for entry in &entries {
            if !entry.valid {
                let _ = std::fs::remove_file(&entry.path);
                freed += entry.size;
            }
        }
        if freed > 0 {
            self.db
                .delete_invalid_cache_entries()
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        }
        if freed >= to_free {
            return Ok(freed);
        }

        // Phase 2: LRU eviction among valid entries
        let valid_entries = self
            .db
            .list_cache_entries_lru()
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        for entry in &valid_entries {
            if freed >= to_free {
                break;
            }
            let _ = std::fs::remove_file(&entry.path);
            freed += entry.size;

            self.db
                .delete_cache_entry(&entry.song_id)
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        }

        Ok(freed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::song::SongId;

    #[tokio::test]
    async fn cached_file_must_match_recorded_size() {
        let base = std::env::temp_dir().join(format!(
            "vide-cache-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        let db = Database::open_in_memory().await.unwrap();
        let cache = CacheManager::new(db, &base).await.unwrap();
        let id = SongId("cachetest".to_string());

        let path = cache
            .cache_media(&id, &b"complete audio bytes"[..], "test")
            .await
            .unwrap();
        assert!(cache.get_cached(&id).await.unwrap().is_some());

        // Simulate a truncated/partial cache file.
        std::fs::write(&path, b"x").unwrap();
        assert!(cache.get_cached(&id).await.unwrap().is_none());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn progressive_pipe_stream_becomes_cache_entry() {
        let base = std::env::temp_dir().join(format!(
            "vide-cache-stream-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        let db = Database::open_in_memory().await.unwrap();
        let cache = CacheManager::new(db, &base).await.unwrap();
        let id = SongId("streamtest".to_string());

        let started = cache
            .start_pipe_stream(
                &id,
                &["sh".to_string(), "-c".to_string(), "printf 'abcdef'".to_string()],
                "test",
                None,
            )
            .await
            .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), started.ready)
            .await
            .unwrap()
            .unwrap();
        assert!(std::fs::metadata(&started.path).unwrap().len() > 0);

        let mut cached = None;
        for _ in 0..20 {
            cached = cache.get_cached(&id).await.unwrap();
            if cached.is_some() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert!(cached.is_some());

        let _ = std::fs::remove_dir_all(&base);
    }
}
