use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::features::plugin::types::StreamDescriptor;
use crate::infra::db::Database;
use crate::models::song::SongId;

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
            StreamDescriptor::Pipe { id, exec } => {
                self.play_pipe(id, &exec.cmd).await
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

    /// Play via a pipe command (no seek support in this mode).
    async fn play_pipe(&mut self, id: &str, cmd_parts: &[String]) -> Result<PlaybackStatus, String> {
        if cmd_parts.is_empty() {
            return Err("empty pipe command".to_string());
        }

        let cmd_str = cmd_parts.join(" ");
        let shell_cmd = format!("{} | mpv --no-video --audio-display=no --msg-level=all=error -", cmd_str);

        let mut pcmd = tokio::process::Command::new("sh");
        pcmd.arg("-c")
            .arg(&shell_cmd)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            pcmd.process_group(0);
        }
        let child = pcmd.spawn().map_err(|e| format!("failed to spawn pipe+mpv: {e}"))?;
        #[cfg(unix)]
        {
            self.child_pgid = Some(child.id().unwrap_or(0) as i32);
        }

        self.child = Some(child);
        self.ipc_sock = None; // pipe mode has no IPC socket
        self.state = PlaybackState::Playing;
        self.media_id = Some(id.to_string());
        self.started_at = Some(Instant::now());
        self.duration = None;
        self.elapsed = 0.0;

        debug!("playing pipe: {} ({:?})", id, cmd_parts);
        Ok(self.status())
    }

    /// Send a JSON-RPC command to mpv's IPC socket and return the raw response.
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

    /// Query the current duration (seconds) from mpv.
    async fn fetch_duration(&self) -> Option<f64> {
        if self.ipc_sock.is_none() {
            return None;
        }
        match self
            .ipc_command(serde_json::json!(["get_property", "duration"]))
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
            .ipc_command(serde_json::json!(["get_property", "time-pos"]))
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
        self.ipc_command(serde_json::json!(["seek", position_secs, "absolute"]))
            .await
            .map(|_| ())?;
        // Keep the frontend position in sync immediately so the progress
        // bar does not snap back to the pre-seek time.
        self.elapsed = position_secs;
        Ok(())
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

        if let Some(path) = song.paths.first() {
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
        self.play_song_id(&id, None).await
    }

    /// Play the given song id (resolves descriptor, spawns mpv, updates state).
    /// If `position_secs` is Some and > 0, waits for the mpv IPC socket to be
    /// ready and seeks there — so a startup resume lands at the saved position
    /// instead of starting from 0.
    pub async fn play_song_id(&mut self, id: &str, position_secs: Option<f64>) -> Result<PlaybackStatus, String> {
        let descriptor = self.descriptor_for(&SongId(id.to_string())).await?;
        let status = self.play(&descriptor).await?;
        if let Some(pos) = position_secs {
            if pos > 0.0 {
                // Give mpv a moment to create the IPC socket before seeking.
                for _ in 0..20 {
                    if self.ipc_sock.is_some() && self.ipc_command(serde_json::json!(["get_property", "time-pos"])).await.is_ok() {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
                let _ = self.seek(pos).await;
                self.elapsed = pos;
            }
        }
        Ok(status)
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
        self.ipc_command(serde_json::json!(["set_property", "pause", true]))
            .await
            .map_err(|e| format!("pause error: {e}"))?;
        self.state = PlaybackState::Paused;
        Ok(self.status())
    }

    /// Resume playback from the current position.
    pub async fn resume(&mut self) -> Result<PlaybackStatus, String> {
        if self.state != PlaybackState::Paused {
            return Ok(self.status());
        }
        self.ipc_command(serde_json::json!(["set_property", "pause", false]))
            .await
            .map_err(|e| format!("resume error: {e}"))?;
        self.state = PlaybackState::Playing;
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

    /// Advance to the next song in the queue per the play mode. In shuffle
    /// mode picks a random unplayed track; otherwise steps +1 (or -1 in reverse)
    /// and wraps when `loop` is set. Returns the new status.
    pub async fn play_next(&mut self) -> Result<PlaybackStatus, String> {
        if self.queue.is_empty() {
            return Ok(self.status());
        }
        let mode = self.play_mode;

        // Repeat-one: replay the current track.
        if mode.single {
            let id = if self.current_index >= 0 {
                self.queue[self.current_index as usize].clone()
            } else {
                self.queue[0].clone()
            };
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
        if self.queue.is_empty() {
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
                        let id = if self.current_index >= 0 {
                            self.queue[self.current_index as usize].clone()
                        } else {
                            self.media_id.clone().unwrap_or_default()
                        };
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
