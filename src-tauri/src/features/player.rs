use std::time::Instant;

use serde::Serialize;
use tokio::io::AsyncWriteExt;
use tracing::{debug, warn};

use crate::features::plugin::types::StreamDescriptor;

/// Playback state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PlaybackState {
    Stopped,
    Playing,
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
/// When playing via URL, mpv's stdin is kept open for seek commands
/// using `--input-file=/dev/stdin`.
pub struct PlayerManager {
    child: Option<tokio::process::Child>,
    stdin: Option<tokio::process::ChildStdin>,
    state: PlaybackState,
    media_id: Option<String>,
    started_at: Option<Instant>,
    duration: Option<f64>,
}

impl PlayerManager {
    /// Create a new stopped player.
    pub fn new() -> Self {
        PlayerManager {
            child: None,
            stdin: None,
            state: PlaybackState::Stopped,
            media_id: None,
            started_at: None,
            duration: None,
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

    /// Play a URL via mpv, keeping stdin open for seek commands.
    async fn play_url(
        &mut self,
        id: &str,
        url: &str,
        headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<PlaybackStatus, String> {
        let mut cmd = tokio::process::Command::new("mpv");
        cmd.arg("--no-video");
        cmd.arg("--audio-display=no");
        cmd.arg("--cache=yes");
        cmd.arg("--msg-level=all=error");
        cmd.arg("--input-file=/dev/stdin");
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());

        if let Some(hdrs) = headers {
            for (key, value) in hdrs {
                cmd.arg("--http-header-fields");
                cmd.arg(format!("{key}: {value}"));
            }
        }

        cmd.arg(url);

        let mut child = cmd.spawn().map_err(|e| format!("failed to spawn mpv: {e}"))?;
        let stdin = child.stdin.take().ok_or_else(|| "stdin not available".to_string())?;

        self.child = Some(child);
        self.stdin = Some(stdin);
        self.state = PlaybackState::Playing;
        self.media_id = Some(id.to_string());
        self.started_at = Some(Instant::now());
        self.duration = None;

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

        let child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&shell_cmd)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to spawn pipe+mpv: {e}"))?;

        self.child = Some(child);
        self.stdin = None; // no stdin handle in pipe mode
        self.state = PlaybackState::Playing;
        self.media_id = Some(id.to_string());
        self.started_at = Some(Instant::now());
        self.duration = None;

        debug!("playing pipe: {} ({:?})", id, cmd_parts);
        Ok(self.status())
    }

    /// Seek to an absolute position (URL mode only).
    pub async fn seek(&mut self, position_secs: f64) -> Result<(), String> {
        if position_secs < 0.0 {
            return Err("negative position".to_string());
        }
        let mut stdin = self
            .stdin
            .take()
            .ok_or_else(|| "seek not available (pipe mode or no active playback)".to_string())?;

        let cmd = format!("set time-pos {}\n", position_secs);
        let result = stdin.write_all(cmd.as_bytes()).await;

        // Put stdin back regardless of result
        self.stdin = Some(stdin);

        result.map_err(|e| format!("seek error: {e}"))
    }

    /// Stop playback and kill the mpv process.
    pub async fn stop(&mut self) -> PlaybackStatus {
        // Drop stdin first so mpv gets EOF / stops blocking on input
        self.stdin.take();
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
            debug!("playback stopped");
        }
        self.state = PlaybackState::Stopped;
        self.media_id = None;
        self.started_at = None;
        self.duration = None;
        self.status()
    }

    /// Get the current playback status.
    pub fn status(&self) -> PlaybackStatus {
        let elapsed = self.started_at.map(|t| t.elapsed().as_secs_f64());

        PlaybackStatus {
            state: self.state,
            media_id: self.media_id.clone(),
            elapsed_secs: elapsed,
            duration_secs: self.duration,
        }
    }

    /// Check if the mpv process is still running.
    pub async fn check_alive(&mut self) {
        if self.state != PlaybackState::Playing {
            return;
        }
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(Some(_status)) => {
                    debug!("mpv process exited");
                    self.state = PlaybackState::Stopped;
                    self.media_id = None;
                    self.started_at = None;
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
        Self::new()
    }
}
