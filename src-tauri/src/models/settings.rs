use serde::{Deserialize, Serialize};

/// All user-facing settings. Stored as a single JSON object in the SQLite
/// settings table under the key "video_settings".
///
/// Persistence policy (SPEC+user):
/// - All *settings* persist in SQLite (settings table).
/// - Pure UI-only tweaks (column width/order) persist in localStorage.
/// One shortcut action bound to an in-app and/or a global key combination.
///
/// Accelerators use the `global-hotkey` / `Code` spelling so a single string
/// works for both sides: modifiers `Ctrl`/`Alt`/`Shift`/`Super` followed by a
/// key such as `Space`, `PageUp`, `ArrowUp`, `F4`, `A`. An empty string means
/// "no shortcut".
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ShortcutBinding {
    /// Fires while the app window has focus.
    #[serde(default)]
    pub in_app: String,
    /// Fires system-wide, even while another app has focus.
    #[serde(default)]
    pub global: String,
}

fn shortcut(in_app: &str, global: &str) -> ShortcutBinding {
    ShortcutBinding {
        in_app: in_app.to_string(),
        global: global.to_string(),
    }
}

/// The set of shortcut actions the user can bind.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShortcutSettings {
    #[serde(default)]
    pub toggle_window: ShortcutBinding,
    #[serde(default)]
    pub toggle_lyrics: ShortcutBinding,
    #[serde(default)]
    pub quit: ShortcutBinding,
    #[serde(default)]
    pub toggle_play: ShortcutBinding,
    #[serde(default)]
    pub prev_track: ShortcutBinding,
    #[serde(default)]
    pub next_track: ShortcutBinding,
    #[serde(default)]
    pub volume_down: ShortcutBinding,
    #[serde(default)]
    pub volume_up: ShortcutBinding,
    #[serde(default)]
    pub seek_back: ShortcutBinding,
    #[serde(default)]
    pub seek_forward: ShortcutBinding,
}

impl Default for ShortcutSettings {
    fn default() -> Self {
        // Global bindings are the in-app ones with Shift added.
        ShortcutSettings {
            toggle_window: shortcut("", ""),
            toggle_lyrics: shortcut("", ""),
            quit: shortcut("Alt+F4", "Shift+Alt+F4"),
            toggle_play: shortcut("Space", "Shift+Space"),
            prev_track: shortcut("PageUp", "Shift+PageUp"),
            next_track: shortcut("PageDown", "Shift+PageDown"),
            volume_down: shortcut("ArrowDown", "Shift+ArrowDown"),
            volume_up: shortcut("ArrowUp", "Shift+ArrowUp"),
            seek_back: shortcut("ArrowLeft", "Shift+ArrowLeft"),
            seek_forward: shortcut("ArrowRight", "Shift+ArrowRight"),
        }
    }
}

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
    /// Show the separate desktop lyrics window.
    #[serde(default)]
    pub desktop_lyrics_enabled: bool,
    /// Lock the desktop lyrics window: no dragging/resizing and every input
    /// event passes through to the window underneath.
    #[serde(default)]
    pub desktop_lyrics_locked: bool,
    /// Keep the desktop lyrics window above other windows.
    #[serde(default = "default_true")]
    pub desktop_lyrics_always_on_top: bool,
    /// Where the active line rests: "top" / "center" / "bottom".
    #[serde(default = "default_lyrics_position")]
    pub desktop_lyrics_position: String,
    /// "dark" or "light"; the web client stores guest choices locally.
    #[serde(default = "default_theme_mode")]
    pub theme_mode: String,
    /// Accent colour (hex, e.g. "#5f5dfd") applied as the UI primary colour.
    #[serde(default = "default_theme_primary")]
    pub theme_primary: String,
    /// User-authored CSS for the floating lyrics window.
    #[serde(default = "default_desktop_lyrics_css")]
    pub desktop_lyrics_css: String,
    /// User-authored CSS for the lyrics block on the player detail page.
    #[serde(default = "default_detail_lyrics_css")]
    pub detail_lyrics_css: String,
    /// In-app and global key bindings per action.
    #[serde(default)]
    pub shortcuts: ShortcutSettings,
    /// Seconds moved by the seek-forward / seek-back shortcuts.
    #[serde(default = "default_seek_step_secs")]
    pub seek_step_secs: f64,
    /// Volume units changed by the volume-up / volume-down shortcuts.
    #[serde(default = "default_volume_step")]
    pub volume_step: u32,
    /// File extensions included by the media scanner (without a dot).
    #[serde(default = "default_scan_extensions")]
    pub scan_extensions: Vec<String>,
    /// Skip leading/trailing silence using the server-side audio analysis.
    #[serde(default)]
    pub skip_silence: bool,
    /// Apply a constant gain derived from integrated loudness.
    #[serde(default)]
    pub normalize_loudness: bool,
    /// Target integrated loudness in LUFS (EBU R128).
    #[serde(default = "default_loudness_target")]
    pub loudness_target_lufs: f64,
}

pub fn default_scan_extensions() -> Vec<String> {
    [
        "mp3", "flac", "ogg", "wav", "m4a", "aac", "opus", "wma", "ape", "aiff", "wv",
    ]
    .iter()
    .map(|value| value.to_string())
    .collect()
}

fn default_true() -> bool {
    true
}

fn default_lyrics_position() -> String {
    "center".to_string()
}

/// Starter stylesheet handed to the Monaco editor. It documents the lyrics DOM
/// so users can see exactly which nodes they can style.
fn default_theme_mode() -> String {
    "dark".to_string()
}

fn default_loudness_target() -> f64 {
    -16.0
}

fn default_theme_primary() -> String {
    "#5f5dfd".to_string()
}

fn default_desktop_lyrics_css() -> String {
    DEFAULT_DESKTOP_LYRICS_CSS.to_string()
}

fn default_detail_lyrics_css() -> String {
    DEFAULT_DETAIL_LYRICS_CSS.to_string()
}

/// Starter stylesheet for the floating lyrics window.
pub const DEFAULT_DESKTOP_LYRICS_CSS: &str = r#"/* 歌词区域 DOM 结构
<div class="lyrics-root" data-position="center">   根节点：铺满歌词区域，背景透明
  <div class="lyrics-viewport">                可视区：负责裁剪与自动滚动
    <div class="lyrics-lines">                 行容器
      <div class="line">                       一行歌词（可含多种语言）
        <p class="lyric-1">english lyric</p>    同一时间的第 1 行
        <p class="lyric-2">中文歌词</p>          同一时间的第 2 行
      </div>
      <div class="line playing">               正在播放的歌词行
        <p class="lyric-1">ruskiy tekst 2</p>
        <p class="lyric-2">english lyric 2</p>
      </div>
    </div>
  </div>
</div>

下面所有规则都可以随意修改或删除。
*/

.lyrics-root {
  width: 100%;
  height: 100%;
  overflow: hidden;
  font-family: inherit;
}

.lyrics-viewport {
  width: 100%;
  height: 100%;
  overflow: hidden;
}

.lyrics-lines {
  text-align: center;
  padding: 8px 16px;
}

.line {
  margin: 6px 0;
  transition: 0.2s;
}

.line p {
  margin: 0;
  font-size: 14px;
  line-height: 1.5;
  color: rgba(255, 255, 255, 0.55);
  text-shadow: 0 1px 3px rgba(0, 0, 0, 0.8);
  transition: 0.2s;
}

/* 同一时间的多行：lyric-1、lyric-2、lyric-3… 按出现顺序 */
.line .lyric-1 { color: rgba(255, 255, 255, 0.8); }
.line .lyric-2 { color: rgba(255, 220, 180, 0.75); }

.playing p {
  font-size: 16px;
  color: green;
}
"#;

/// Starter stylesheet for the lyrics block on the player detail page.
pub const DEFAULT_DETAIL_LYRICS_CSS: &str = r#"/* 歌词区域 DOM 结构（与悬浮歌词一致）
<div class="lyrics-root" data-position="center">
  <div class="lyrics-viewport">
    <div class="lyrics-lines">
      <div class="line">
        <p class="lyric-1">english lyric</p>
        <p class="lyric-2">中文歌词</p>
      </div>
      <div class="line playing">
        <p class="lyric-1">english lyric 2</p>
      </div>
    </div>
  </div>
</div>

下面所有规则都可以随意修改或删除。
*/

.lyrics-root {
  width: 100%;
  height: 100%;
  overflow: hidden;
  font-family: inherit;
}

.lyrics-viewport {
  width: 100%;
  height: 100%;
  overflow: hidden;
}

.lyrics-lines {
  text-align: center;
  padding: 8px 16px;
}

.line {
  margin: 10px 0;
  transition: 0.2s;
}

.line p {
  margin: 0;
  font-size: 14px;
  line-height: 1.6;
  color: var(--color-base-content) / 0.6;
  transition: 0.2s;
}

/* 同一时间的多行：lyric-1、lyric-2、lyric-3… 按出现顺序 */
.line .lyric-1 { color: var(--color-base-content) / 0.85; }
.line .lyric-2 { color: var(--color-base-content) / 0.6; }

.playing p {
  font-size: 16px;
  color: var(--color-base-content);
  font-weight: 600;
}
"#;
fn default_seek_step_secs() -> f64 {
    5.0
}

fn default_volume_step() -> u32 {
    5
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            close_to_tray: false,
            auto_play_on_start: false,
            pause_on_device_change: false,
            window_scale: 100,
            font_family: "Segoe UI, 'SF Pro Display', -apple-system, sans-serif".to_string(),
            desktop_lyrics_enabled: false,
            desktop_lyrics_locked: false,
            desktop_lyrics_always_on_top: true,
            desktop_lyrics_position: default_lyrics_position(),
            theme_mode: default_theme_mode(),
            theme_primary: default_theme_primary(),
            desktop_lyrics_css: default_desktop_lyrics_css(),
            detail_lyrics_css: default_detail_lyrics_css(),
            shortcuts: ShortcutSettings::default(),
            seek_step_secs: default_seek_step_secs(),
            volume_step: default_volume_step(),
            scan_extensions: default_scan_extensions(),
            skip_silence: false,
            normalize_loudness: false,
            loudness_target_lufs: default_loudness_target(),
        }
    }
}
