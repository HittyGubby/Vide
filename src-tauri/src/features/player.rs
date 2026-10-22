use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use tracing::{debug, warn};

use crate::features::cache::CacheManager;
use crate::features::plugin::types::StreamDescriptor;
use crate::infra::db::Database;
use crate::models::song::SongId;

/// Bound on concurrent background stream downloads. Old streams are never
/// cancelled on pause/switch; this just prevents a rapid skip burst from
/// starting an unbounded number of download processes.
const MAX_CONCURRENT_STREAMS: usize = 3;

fn stream_semaphore() -> &'static Arc<Semaphore> {
    static LIMIT: OnceLock<Arc<Semaphore>> = OnceLock::new();
    LIMIT.get_or_init(|| Arc::new(Semaphore::new(MAX_CONCURRENT_STREAMS)))
}

/// Play order mode — all decisions (next/prev/loop/shuffle) live in Rust so
/// mpv is only ever told what to play, not how to sequence the queue.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PlayMode {
    pub single: bool,   // repeat-one
    pub reverse: bool,  // backward order (ignored when shuffle)
    pub shuffle: bool,  // random order
    /// `repeat` (was `loop`, which is a Rust keyword) — wrap at the end.
    pub repeat: bool,
}

impl Default for PlayMode {
    fn default() -> Self {
        PlayMode { single: false, reverse: false, shuffle: false, repeat: true }
    }
}

/// Playback state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PlaybackState {
    Stopped,
    Playing,
    /// Playback is loaded but currently paused (song still selected).
    Paused,
    /// The media finished naturally (mpv exited on its own). The song is
    /// still selected so the frontend can advance to the next track.
    Ended,
}

/// Result of a playback operation.
#[derive(Debug, Clone, Serialize)]
pub struct PlaybackStatus {
    pub state: PlaybackState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_secs: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<f64>,
}

/// Playback engine wrapping an mpv subprocess.
///
/// Spawns mpv for `url_stream` or pipes a command to mpv's stdin.
/// Control (seek, pause, duration/progress queries) is done over mpv's
/// JSON-RPC IPC socket (`--input-ipc-server`). This replaces the old
/// `--input-file` approach, which is unsupported by modern mpv (>= 0.38)
/// and caused the player to exit immediately on startup.
pub struct PlayerManager {
    child: Option<tokio::process::Child>,
    /// Path to the mpv JSON-RPC IPC socket, when available.
    ipc_sock: Option<std::path::PathBuf>,
    /// Process-group id so we can kill mpv (and its pipe children) reliably.
    #[cfg(unix)]
    child_pgid: Option<i32>,
    state: PlaybackState,
    media_id: Option<String>,
    started_at: Option<Instant>,
    duration: Option<f64>,
    /// Current playback position in seconds (from mpv's time-pos).
    elapsed: f64,

    // ── Playback queue / sequencing (owned by Rust, not mpv) ──────────
    /// Ordered list of song ids for the active queue.
    queue: Vec<String>,
    /// Index of the currently playing song in `queue`; -1 = none.
    current_index: isize,
    /// Play order mode.
    play_mode: PlayMode,
    /// Set of ids already played in the current shuffle round.
    shuffle_played: std::collections::HashSet<String>,
    /// Database access to resolve a song id → playable descriptor.
    db: Option<Arc<Database>>,
    /// Plugin access to resolve online streams.
    plugins: Option<Arc<crate::features::plugin::PluginManager>>,
}

impl PlayerManager {
    /// Create a new stopped player. `db` and `plugins` let the player resolve
    /// a song id into a playable stream and own the queue/advance logic.
    pub fn new(
        db: Option<Arc<Database>>,
        plugins: Option<Arc<crate::features::plugin::PluginManager>>,
    ) -> Self {
        PlayerManager {
            child: None,
            ipc_sock: None,
            #[cfg(unix)]
            child_pgid: None,
            state: PlaybackState::Stopped,
            media_id: None,
            started_at: None,
            duration: None,
            elapsed: 0.0,
            queue: Vec::new(),
            current_index: -1,
            play_mode: PlayMode::default(),
            shuffle_played: std::collections::HashSet::new(),
            db,
            plugins,
        }
    }

    /// Start playing a stream from a descriptor.
    pub async fn play(&mut self, descriptor: &StreamDescriptor) -> Result<PlaybackStatus, String> {
        self.stop().await;

        let result = match descriptor {
            StreamDescriptor::UrlStream { id, url, headers } => {
                self.play_url(id, url, headers.as_ref()).await
            }
            StreamDescriptor::Pipe { id, cmd } => {
                self.play_plugin_pipe(id, cmd).await
            }
        };

        match result {
            Ok(status) => Ok(status),
            Err(e) => {
                self.state = PlaybackState::Stopped;
                Err(e)
            }
        }
    }

    /// Build a unique IPC socket path for an mpv instance.
    fn ipc_path() -> std::path::PathBuf {
        let nonce = std::process::id() as u64
            ^ (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0));
        std::env::temp_dir().join(format!("vide-mpv-{nonce:x}.sock"))
    }

    /// Play a URL via mpv, controlling it over a JSON-RPC IPC socket.
    async fn play_url(
        &mut self,
        id: &str,
        url: &str,
        headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<PlaybackStatus, String> {
        let sock = Self::ipc_path();
        let _ = std::fs::remove_file(&sock);

        let mut cmd = tokio::process::Command::new("mpv");
        cmd.arg("--no-video");
        cmd.arg("--audio-display=no");
        cmd.arg("--cache=yes");
        cmd.arg("--force-seekable=yes");
        cmd.arg("--msg-level=all=error");
        // mpv requires `--input-ipc-server=<path>` as a *single* argv
        // (a space-separated form "option value" is rejected with
        // "option requires parameter" and mpv exits immediately).
        cmd.arg(format!("--input-ipc-server={}", sock.display()));
        // Keep stdin /dev/null so mpv does not wait on us and stays quiet.
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());

        if let Some(hdrs) = headers {
            for (key, value) in hdrs {
                cmd.arg("--http-header-fields");
                cmd.arg(format!("{key}: {value}"));
            }
        }

        cmd.arg(url);

        // Ensure the child is killed if this handle is dropped without an
        // explicit stop (e.g. the app exits and AppState is released).
        cmd.kill_on_drop(true);
        // Put mpv in its own process group so we can kill the whole group
        // (including any pipe children) on stop / app exit.
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        let child = cmd.spawn().map_err(|e| format!("failed to spawn mpv: {e}"))?;
        #[cfg(unix)]
        {
            self.child_pgid = Some(child.id().unwrap_or(0) as i32);
        }

        self.child = Some(child);
        self.ipc_sock = Some(sock);
        self.state = PlaybackState::Playing;
        self.media_id = Some(id.to_string());
        self.started_at = Some(Instant::now());
        self.duration = None;
        self.elapsed = 0.0;

        debug!("playing url: {} ({})", id, url);
        Ok(self.status())
    }

    /// Play an online pipe by writing it into the cache and letting mpv play
    /// the growing file. Pausing and switching do not kill the download; old
    /// streams finish in the background and become normal cache entries.
    async fn play_plugin_pipe(
        &mut self,
        id: &str,
        cmd_parts: &[String],
    ) -> Result<PlaybackStatus, String> {
        if cmd_parts.is_empty() {
            return Err("empty pipe command".to_string());
        }
        let db = self
            .db
            .as_ref()
            .ok_or_else(|| "database unavailable".to_string())?;
        let base = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("vide");
        let cache = CacheManager::new(db.as_ref().clone(), &base)
            .await
            .map_err(|e| format!("cache init: {e}"))?;
        let song_id = SongId(id.to_string());

        // A completed cache copy is preferred over a new stream.
        if let Some(path) = cache
            .get_cached(&song_id)
            .await
            .map_err(|e| format!("cache lookup: {e}"))?
        {
            return self.play_url(id, &path.to_string_lossy(), None).await;
        }

        // Limit simultaneous background downloads. If the cap is reached, the
        // new track waits for an older stream to finish rather than killing it.
        let permit = stream_semaphore()
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| "stream limiter closed".to_string())?;
        let started = cache
            .start_pipe_stream(&song_id, cmd_parts, id, Some(permit))
            .await
            .map_err(|e| format!("stream start: {e}"))?;

        tokio::time::timeout(Duration::from_secs(15), started.ready)
            .await
            .map_err(|_| "timed out waiting for stream data".to_string())?
            .map_err(|_| "stream ended before writing data".to_string())?;

        debug!("playing cached stream: {} ({:?})", id, started.path);
        self.play_url(id, &started.path.to_string_lossy(), None).await
    }

    /// Send a JSON-RPC command to mpv's IPC socket and return the raw response.
    #[cfg(unix)]
    async fn ipc_command(&self, command: serde_json::Value) -> Result<serde_json::Value, String> {
        let sock = self
            .ipc_sock
            .as_ref()
            .ok_or_else(|| "no active IPC socket".to_string())?;

        // Use a short connect timeout so a dead socket does not hang the UI.
        let stream = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            tokio::net::UnixStream::connect(sock),
        )
        .await
        .map_err(|_| "IPC connect timeout".to_string())?
        .map_err(|e| format!("IPC connect error: {e}"))?;

        let mut request = serde_json::json!({ "command": command });
        if request.get("request_id").is_none() {
            request["request_id"] = serde_json::json!(0);
        }
        let line = serde_json::to_string(&request)
            .map_err(|e| format!("IPC serialize error: {e}"))?;

        stream
            .writable()
            .await
            .map_err(|e| format!("IPC wait writable error: {e}"))?;
        stream
            .try_write(format!("{line}\n").as_bytes())
            .map_err(|e| format!("IPC write error: {e}"))?;

        // Read one line response.
        let mut buf = Vec::with_capacity(256);
        loop {
            stream
                .readable()
                .await
                .map_err(|e| format!("IPC wait readable error: {e}"))?;
            let mut chunk = [0u8; 4096];
            match stream.try_read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&chunk[..n]);
                    if buf.contains(&b'\n') {
                        break;
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => return Err(format!("IPC read error: {e}")),
            }
        }

        let text = String::from_utf8_lossy(&buf).trim().to_string();
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("IPC parse error: {e} ({text})"))?;

        Ok(value)
    }

    #[cfg(not(unix))]
    async fn ipc_command(&self, _command: serde_json::Value) -> Result<serde_json::Value, String> {
        Err("mpv IPC is not supported on this platform".to_string())
    }

    /// Send a command and require mpv to report success.
    ///
    /// `ipc_command` only reports transport failures, but mpv also answers
    /// with an *error payload* (e.g. `property unavailable` while it is still
    /// opening a file, or `error running command` when a command is rejected).
    /// Treating those as success is what made startup resume start from 0: the
    /// readiness probe looked "ok" before mpv was ready and the seek was then
    /// rejected. Every control command goes through here instead.
    async fn ipc_command_ok(&self, command: serde_json::Value) -> Result<serde_json::Value, String> {
        let value = self.ipc_command(command).await?;
        match value.get("error").and_then(|e| e.as_str()) {
            Some("success") => Ok(value),
            Some(e) => Err(format!("mpv error: {e}")),
            None => Err("mpv response has no error field".to_string()),
        }
    }

    /// Query the current duration (seconds) from mpv.
    async fn fetch_duration(&self) -> Option<f64> {
        if self.ipc_sock.is_none() {
            return None;
        }
        match self
            .ipc_command_ok(serde_json::json!(["get_property", "duration"]))
            .await
        {
            Ok(v) => v.get("data").and_then(|d| d.as_f64()),
            Err(e) => {
                debug!("duration query failed: {e}");
                None
            }
        }
    }

    /// Query the current time-pos (seconds) from mpv.
    async fn fetch_time_pos(&self) -> Option<f64> {
        if self.ipc_sock.is_none() {
            return None;
        }
        match self
            .ipc_command_ok(serde_json::json!(["get_property", "time-pos"]))
            .await
        {
            Ok(v) => v.get("data").and_then(|d| d.as_f64()),
            Err(e) => {
                debug!("time-pos query failed: {e}");
                None
            }
        }
    }

    /// Seek to an absolute position (URL/IPC mode only).
    pub async fn seek(&mut self, position_secs: f64) -> Result<(), String> {
        if position_secs < 0.0 {
            return Err("negative position".to_string());
        }
        self.ipc_command_ok(serde_json::json!(["seek", position_secs, "absolute"]))
            .await
            .map(|_| ())?;
        // Keep the frontend position in sync immediately so the progress
        // bar does not snap back to the pre-seek time.
        self.elapsed = position_secs;
        Ok(())
    }

    /// Seek to `position_secs` once mpv is actually able to accept it.
    ///
    /// Right after spawn the IPC socket exists but mpv has not loaded the file
    /// yet: `time-pos` answers "property unavailable" and a `seek` sent at that
    /// point is rejected outright. Poll `time-pos` until it is answerable, then
    /// retry the seek until mpv accepts it.
    pub async fn seek_when_ready(&mut self, position_secs: f64) -> Result<(), String> {
        if self.ipc_sock.is_none() {
            // Pipe mode has no IPC socket at all; seeking is impossible.
            return Err("no IPC socket for this stream".to_string());
        }
        // Wait for mpv to finish opening the file (bounded to ~5s).
        for _ in 0..100 {
            if self.fetch_time_pos().await.is_some() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        let mut last_err = String::from("seek was never applied");
        for _ in 0..5 {
            match self.seek(position_secs).await {
                Ok(()) => {
                    // mpv answers the command before the new position is
                    // actually reported. Read it back, otherwise the next
                    // status poll returns the pre-seek time and the progress
                    // bar visibly jumps backwards and then forwards again.
                    let mut previous: Option<f64> = None;
                    for _ in 0..10 {
                        if let Some(t) = self.fetch_time_pos().await {
                            if (t - position_secs).abs() <= 1.5 {
                                self.elapsed = t;
                                return Ok(());
                            }
                            // The position stopped moving: mpv settled
                            // elsewhere (e.g. the target was past the end).
                            if previous.is_some_and(|p| (p - t).abs() < 0.05) {
                                self.elapsed = t;
                                return Ok(());
                            }
                            previous = Some(t);
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                    last_err = "seek was never reflected in time-pos".to_string();
                }
                Err(e) => last_err = e,
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        Err(last_err)
    }

    /// Set playback speed (rate). 1.0 = normal.
    pub async fn set_speed(&mut self, speed: f64) -> Result<(), String> {
        if speed <= 0.0 {
            return Err("speed must be > 0".to_string());
        }
        self.ipc_command(serde_json::json!(["set_property", "speed", speed]))
            .await
            .map(|_| ())
    }

    /// Set playback volume (0..=100, where 100 = mpv softmax/lowest-volume=off).
    pub async fn set_volume(&mut self, volume: f64) -> Result<(), String> {
        let v = volume.clamp(0.0, 100.0);
        self.ipc_command(serde_json::json!(["set_property", "volume", v]))
            .await
            .map(|_| ())
    }

    /// Mute / unmute playback.
    pub async fn set_mute(&mut self, muted: bool) -> Result<(), String> {
        self.ipc_command(serde_json::json!(["set_property", "mute", muted]))
            .await
            .map(|_| ())
    }

    /// Resolve a song id into a playable stream descriptor using local paths
    /// or the plugin system for online sources.
    async fn descriptor_for(&self, song_id: &SongId) -> Result<StreamDescriptor, String> {
        let db = self
            .db
            .as_ref()
            .ok_or_else(|| "database unavailable".to_string())?;
        let song = db
            .get_song(song_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("song not found: {}", song_id.0))?;

        // Use the first path that still exists; a stale first entry (moved
        // file, unmounted drive) must not hide a valid duplicate.
        if let Some(path) = song.paths.iter().find(|path| path.exists()) {
            Ok(StreamDescriptor::UrlStream {
                id: song_id.0.clone(),
                url: path.to_string_lossy().to_string(),
                headers: None,
            })
        } else if let Some(source) = song.sources.first() {
            let plugins = self
                .plugins
                .as_ref()
                .ok_or_else(|| "plugin system not available".to_string())?;
            plugins.stream(&source.platform, &source.id, &song_id.0).await
        } else {
            Err("no playable source found".to_string())
        }
    }

    /// Persist the active queue + cursor in the settings table.
    ///
    /// The queue is runtime state, but losing it on restart made next/prev
    /// silently do nothing after the startup auto-resume (play_song started
    /// the track without registering a queue). Keeping it in SQLite means the
    /// backend is ready before the frontend restores anything.
    async fn persist_queue(&self) {
        let Some(db) = &self.db else {
            return;
        };
        let value = serde_json::json!({
            "ids": self.queue,
            "current_index": self.current_index,
        });
        if let Err(e) = db.update_setting("play_queue", &value).await {
            warn!("failed to persist play queue: {e}");
        }
    }

    /// Load the queue persisted by the previous session. Playback is not
    /// started here; the frontend's last-played restore (or the user) decides
    /// what to play, and play_song_id syncs the cursor to that song.
    pub async fn restore_queue(&mut self) {
        let Some(db) = &self.db else {
            return;
        };
        let value = match db.get_setting("play_queue").await {
            Ok(Some(value)) => value,
            Ok(None) => return,
            Err(e) => {
                warn!("failed to load persisted play queue: {e}");
                return;
            }
        };
        let Some(ids) = value
            .get("ids")
            .and_then(|v| serde_json::from_value::<Vec<String>>(v.clone()).ok())
        else {
            return;
        };
        if ids.is_empty() {
            return;
        }
        let index = value
            .get("current_index")
            .and_then(|v| v.as_i64())
            .unwrap_or(-1) as isize;
        self.queue = ids;
        self.current_index = index.clamp(-1, self.queue.len() as isize - 1);
        debug!(
            "restored play queue: {} tracks, index {}",
            self.queue.len(),
            self.current_index
        );
    }

    /// Set the playback queue and start playing `start_id` (or index -1 to just
    /// store the queue). All sequencing (next/prev/loop) is decided here in Rust.
    pub async fn set_queue(
        &mut self,
        ids: Vec<String>,
        start_index: isize,
    ) -> Result<PlaybackStatus, String> {
        self.queue = ids;
        self.current_index = start_index;
        self.shuffle_played.clear();
        self.persist_queue().await;
        if start_index >= 0 && start_index < self.queue.len() as isize {
            let id = self.queue[start_index as usize].clone();
            return self.play_song_id(&id, None).await;
        }
        Ok(self.status())
    }

    /// Play the song at a specific index in the queue (used by next/prev).
    async fn play_index(&mut self, index: isize) -> Result<PlaybackStatus, String> {
        if self.queue.is_empty() {
            return Ok(self.status());
        }
        let n = self.queue.len() as isize;
        let idx = ((index % n) + n) % n;
        let id = self.queue[idx as usize].clone();
        self.current_index = idx;
        self.shuffle_played.insert(id.clone());
        self.persist_queue().await;
        self.play_song_id(&id, None).await
    }

    /// Play the given song id (resolves descriptor, spawns mpv, updates state).
    /// If `position_secs` is Some and > 0, waits for the mpv IPC socket to be
    /// ready and seeks there — so a startup resume lands at the saved position
    /// instead of starting from 0.
    pub async fn play_song_id(&mut self, id: &str, position_secs: Option<f64>) -> Result<PlaybackStatus, String> {
        // Keep the queue cursor in sync when a song is started directly (the
        // startup resume and the search page call play_song without going
        // through play_index). Otherwise next/prev would advance from a stale
        // index — or do nothing when the cursor was still -1.
        let mut cursor_changed = false;
        if let Some(idx) = self.queue.iter().position(|queued| queued == id) {
            if self.current_index != idx as isize {
                self.current_index = idx as isize;
                cursor_changed = true;
            }
        } else if self.current_index >= 0 {
            // A song started directly may not belong to the persisted queue.
            // Do not leave a stale cursor behind for repeat-one/shuffle.
            self.current_index = -1;
        }
        let descriptor = self.descriptor_for(&SongId(id.to_string())).await?;
        self.play(&descriptor).await?;
        // Only write when the cursor actually moved. Playing a song that is
        // outside the current queue must not overwrite the saved queue.
        if cursor_changed {
            self.persist_queue().await;
        }
        if let Some(pos) = position_secs {
            if pos > 0.0 {
                // A failed resume must not fail the whole play request — warn
                // and keep playing from the start instead.
                if let Err(e) = self.seek_when_ready(pos).await {
                    warn!("resume seek to {pos}s failed: {e}");
                }
            }
        }
        // Build the status *after* the seek so the frontend is told the resume
        // position instead of 0 (which made the progress bar snap back).
        Ok(self.status())
    }

    /// Set the play order mode.
    pub fn set_play_mode(&mut self, mode: PlayMode) {
        self.play_mode = mode;
        if mode.shuffle {
            self.shuffle_played.clear();
        }
    }

    /// Pause playback (keeps the song loaded and its position).
    pub async fn pause(&mut self) -> Result<PlaybackStatus, String> {
        if self.state != PlaybackState::Playing {
            // Nothing to pause; treat as already paused if a song is loaded.
            if self.media_id.is_some() {
                self.state = PlaybackState::Paused;
            }
            return Ok(self.status());
        }
        // Snapshot the real playhead before pausing. The UI freezes the
        // progress bar here; using the last polled value (up to a second old)
        // made the bar snap backwards when playback resumed.
        if let Some(t) = self.fetch_time_pos().await {
            self.elapsed = t;
        }
        self.ipc_command_ok(serde_json::json!(["set_property", "pause", true]))
            .await
            .map_err(|e| format!("pause error: {e}"))?;
        self.state = PlaybackState::Paused;
        if let Some(t) = self.fetch_time_pos().await {
            self.elapsed = t;
        }
        Ok(self.status())
    }

    /// Resume playback from the current position.
    pub async fn resume(&mut self) -> Result<PlaybackStatus, String> {
        if self.state != PlaybackState::Paused {
            return Ok(self.status());
        }
        // The position is authoritative while paused; report the same value to
        // the frontend before and after resuming so its interpolated progress
        // bar does not jump.
        if let Some(t) = self.fetch_time_pos().await {
            self.elapsed = t;
        }
        self.ipc_command_ok(serde_json::json!(["set_property", "pause", false]))
            .await
            .map_err(|e| format!("resume error: {e}"))?;
        self.state = PlaybackState::Playing;
        if let Some(t) = self.fetch_time_pos().await {
            self.elapsed = t;
        }
        Ok(self.status())
    }

    /// Toggle between playing and paused with one call.
    pub async fn toggle_pause(&mut self) -> Result<PlaybackStatus, String> {
        match self.state {
            PlaybackState::Playing => self.pause().await,
            PlaybackState::Paused => self.resume().await,
            // Ended already has no live mpv; the frontend restarts the track.
            PlaybackState::Ended | PlaybackState::Stopped => Ok(self.status()),
        }
    }

    /// Last-resort queue for a play session that started without one (e.g.
    /// the very first launch after this feature, before any queue has been
    /// persisted). If the current track belongs to the library, build the
    /// queue from the library so next/prev still do something useful.
    async fn ensure_queue_for_current(&mut self) -> bool {
        if !self.queue.is_empty() {
            return true;
        }
        let (Some(db), Some(media_id)) = (&self.db, self.media_id.clone()) else {
            return false;
        };
        let Ok(songs) = db.list_library_songs().await else {
            return false;
        };
        let Some(index) = songs.iter().position(|song| song.id.0 == media_id) else {
            return false;
        };
        self.queue = songs.into_iter().map(|song| song.id.0).collect();
        self.current_index = index as isize;
        self.shuffle_played.clear();
        self.persist_queue().await;
        true
    }

    /// Advance to the next song in the queue per the play mode. In shuffle
    /// mode picks a random unplayed track; otherwise steps +1 (or -1 in reverse)
    /// and wraps when `loop` is set. Returns the new status.
    pub async fn play_next(&mut self) -> Result<PlaybackStatus, String> {
        if !self.ensure_queue_for_current().await {
            return Ok(self.status());
        }
        let mode = self.play_mode;

        // Repeat-one: replay the current track, even when it was started
        // outside the persisted queue.
        if mode.single {
            let id = self
                .media_id
                .clone()
                .or_else(|| {
                    (self.current_index >= 0)
                        .then(|| self.queue.get(self.current_index as usize).cloned())
                        .flatten()
                })
                .or_else(|| self.queue.first().cloned())
                .unwrap_or_default();
            if id.is_empty() {
                return Ok(self.status());
            }
            return self.play_song_id(&id, None).await;
        }

        // Shuffle: choose a random unplayed track; reset the round when done.
        if mode.shuffle && self.queue.len() > 1 {
            let pool: Vec<usize> = (0..self.queue.len())
                .filter(|i| *i as isize != self.current_index && !self.shuffle_played.contains(&self.queue[*i]))
                .collect();
            if pool.is_empty() {
                self.shuffle_played.clear();
                let pool2: Vec<usize> = (0..self.queue.len())
                    .filter(|i| *i as isize != self.current_index)
                    .collect();
                if pool2.is_empty() {
                    return Ok(self.status());
                }
                let pick = pool2[rand_index(pool2.len())];
                return self.play_index(pick as isize).await;
            }
            let pick = pool[rand_index(pool.len())];
            return self.play_index(pick as isize).await;
        }

        if self.current_index < 0 {
            let start = if mode.reverse { self.queue.len() - 1 } else { 0 };
            return self.play_index(start as isize).await;
        }

        let mut next = self.current_index + if mode.reverse { -1 } else { 1 };
        if next < 0 || next >= self.queue.len() as isize {
            if mode.repeat {
                next = if mode.reverse { self.queue.len() as isize - 1 } else { 0 };
            } else {
                // End of queue without loop: stop.
                self.stop().await;
                return Ok(self.status());
            }
        }
        self.play_index(next).await
    }

    /// Play the previous song in the queue.
    pub async fn play_prev(&mut self) -> Result<PlaybackStatus, String> {
        if !self.ensure_queue_for_current().await {
            return Ok(self.status());
        }
        if self.current_index <= 0 {
            return self.play_index(self.queue.len() as isize - 1).await;
        }
        self.play_index(self.current_index - 1).await
    }

    /// Kill the mpv process group (mpv plus any pipe children) immediately.
    #[cfg(unix)]
    fn kill_process_group(&mut self) {
        if let Some(pgid) = self.child_pgid.take() {
            unsafe {
                libc::kill(-pgid, libc::SIGKILL);
            }
        }
        if let Some(ref mut child) = self.child {
            let _ = child.start_kill();
        }
    }

    /// Stop playback and kill the mpv process.
    pub async fn stop(&mut self) -> PlaybackStatus {
        #[cfg(unix)]
        self.kill_process_group();
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
            debug!("playback stopped");
        }
        // Clean up the IPC socket file.
        if let Some(sock) = self.ipc_sock.take() {
            let _ = std::fs::remove_file(&sock);
        }
        self.state = PlaybackState::Stopped;
        self.media_id = None;
        self.started_at = None;
        self.duration = None;
        self.elapsed = 0.0;
        self.status()
    }

    /// Get the current playback status.
    pub fn queue_snapshot(&self) -> (Vec<String>, isize) {
        (self.queue.clone(), self.current_index)
    }

    pub fn status(&self) -> PlaybackStatus {
        PlaybackStatus {
            state: self.state,
            media_id: self.media_id.clone(),
            elapsed_secs: Some(self.elapsed),
            duration_secs: self.duration,
        }
    }

    /// Refresh playback metrics (duration + position) from mpv.
    ///
    /// Uses mpv's `time-pos` so the reported position is authoritative and
    /// survives seeks (a wall-clock tick would snap back after seeking).
    /// Called periodically by the frontend status poll.
    pub async fn refresh(&mut self) {
        if self.state == PlaybackState::Stopped {
            return;
        }
        if self.state == PlaybackState::Playing {
            if let Some(t) = self.fetch_time_pos().await {
                self.elapsed = t;
            }
        }
        if self.duration.is_none() {
            self.duration = self.fetch_duration().await;
        }
    }

    /// Check if the mpv process is still running.
    ///
    /// If mpv exited *on its own* (the media finished), Rust owns the queue and
    /// auto-advances to the next song (repeat-one replays the current track;
    /// otherwise it steps per the play mode). This keeps mpv as a dumb player
    /// and all sequencing logic here. Explicit user stops go through `stop()`.
    pub async fn check_alive(&mut self) {
        if self.state != PlaybackState::Playing && self.state != PlaybackState::Paused {
            return;
        }
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(Some(_status)) => {
                    debug!("mpv process exited (natural end)");
                    self.state = PlaybackState::Ended;
                    self.started_at = None;
                    // The media ended naturally: Rust decides what plays next.
                    // In repeat-one mode restart the current track; otherwise
                    // advance the queue (looping is handled inside play_next).
                    if self.play_mode.single {
                        let id = self
                            .media_id
                            .clone()
                            .or_else(|| {
                                (self.current_index >= 0)
                                    .then(|| self.queue.get(self.current_index as usize).cloned())
                                    .flatten()
                            })
                            .unwrap_or_default();
                        if !id.is_empty() {
                            let _ = self.play_song_id(&id, None).await;
                        }
                    } else {
                        let _ = self.play_next().await;
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    warn!("error checking mpv status: {e}");
                }
            }
        }
    }
}

impl Default for PlayerManager {
    fn default() -> Self {
        Self::new(None, None)
    }
}

/// A tiny non-cryptographic random index helper (no external crates).
#[inline]
fn rand_index(len: usize) -> usize {
    use std::time::{SystemTime, UNIX_EPOCH};
    if len <= 1 {
        return 0;
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    // xorshift for a quick pseudo-random value.
    let mut x = nanos ^ 0x9E3779B97F4A7C15;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    (x as usize) % len
}

impl Drop for PlayerManager {
    /// When the app closes, make sure the mpv subprocess (and its process
    /// group) is terminated so it does not keep playing after Vide exits.
    fn drop(&mut self) {
        #[cfg(unix)]
        self.kill_process_group();
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
            // Poll-wait briefly so the OS reaps the child. This is a sync
            // context; a short blocking wait is acceptable at shutdown.
            let _ = child.try_wait();
        }
        // Clean up the IPC socket file.
        if let Some(sock) = self.ipc_sock.take() {
            let _ = std::fs::remove_file(&sock);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn play_queue_roundtrips_through_settings() {
        let db = Arc::new(Database::open_in_memory().await.unwrap());

        let mut player = PlayerManager::new(Some(db.clone()), None);
        player
            .set_queue(vec!["a".into(), "b".into(), "c".into()], -1)
            .await
            .unwrap();
        // Simulate the user advancing to the second track.
        player.current_index = 1;
        player.persist_queue().await;

        let mut restored = PlayerManager::new(Some(db), None);
        restored.restore_queue().await;
        assert_eq!(restored.queue, vec!["a", "b", "c"]);
        assert_eq!(restored.current_index, 1);
    }
}
