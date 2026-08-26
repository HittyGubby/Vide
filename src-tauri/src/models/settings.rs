use serde::{Deserialize, Serialize};

/// All user-facing settings. Stored as a single JSON object in the SQLite
/// settings table under the key "video_settings".
///
/// Persistence policy (SPEC+user):
/// - All *settings* persist in SQLite (settings table).
/// - Pure UI-only tweaks (column width/order) persist in localStorage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Close the window → minimize to system tray instead of quitting.
    pub close_to_tray: bool,
    /// Auto-play music when the app starts.
    pub auto_play_on_start: bool,
    /// Pause playback when the audio device changes.
    pub pause_on_device_change: bool,
    /// Window scale multiplier (e.g. 100 = 100%).
    pub window_scale: u32,
    /// Font family list (CSS fallback), e.g. "Segoe UI, sans-serif".
    pub font_family: String,
    /// Lyrics alignment in the detail page: left / center / right.
    pub lyrics_align: String,
    /// Lyrics font size (px) in the detail page.
    pub lyrics_font_size: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            close_to_tray: false,
            auto_play_on_start: false,
            pause_on_device_change: false,
            window_scale: 100,
            font_family: "Segoe UI, 'SF Pro Display', -apple-system, sans-serif".to_string(),
            lyrics_align: "center".to_string(),
            lyrics_font_size: 14,
        }
    }
}
