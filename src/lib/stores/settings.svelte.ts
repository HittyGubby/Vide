import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import type { AppSettings, LyricsPosition } from "$lib/types";
import { defaultShortcuts } from "$lib/shortcuts";
import { DEFAULT_DESKTOP_LYRICS_CSS, DEFAULT_DETAIL_LYRICS_CSS } from "$lib/lyrics";
import { applyTheme, DEFAULT_PRIMARY } from "$lib/theme";

class SettingsStore {
  settings = $state<AppSettings>({
    close_to_tray: false,
    auto_play_on_start: false,
    pause_on_device_change: false,
    window_scale: 100,
    font_family: "Segoe UI, 'SF Pro Display', -apple-system, sans-serif",
    desktop_lyrics_enabled: false,
    desktop_lyrics_locked: false,
    desktop_lyrics_always_on_top: true,
    desktop_lyrics_position: "center" as LyricsPosition,
    theme_primary: DEFAULT_PRIMARY,
    desktop_lyrics_css: DEFAULT_DESKTOP_LYRICS_CSS,
    detail_lyrics_css: DEFAULT_DETAIL_LYRICS_CSS,
    shortcuts: defaultShortcuts(),
    seek_step_secs: 5,
    volume_step: 5,
  });
  loaded = $state(false);

  /** Load settings from the backend. */
  async load() {
    try {
      this.settings = await invoke<AppSettings>("get_app_settings");
    } catch {
      /* keep defaults */
    } finally {
      this.loaded = true;
      applyTheme(this.settings);
    }
  }

  /** Save the current settings snapshot. Returns a promise. */
  async save() {
    try {
      await invoke("set_app_settings", { settings: { ...this.settings } });
      // Let the other windows (desktop lyrics) pick the change up right away.
      try {
        await emit("vide:settings-changed", null);
      } catch {
        /* not fatal */
      }
    } catch (e) {
      console.error("save settings error:", e);
    }
  }

  /** Update a field and persist. */
  async update<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    this.settings = { ...this.settings, [key]: value };
    applyTheme(this.settings);
    await this.save();
  }
}

export const settingsStore = new SettingsStore();
