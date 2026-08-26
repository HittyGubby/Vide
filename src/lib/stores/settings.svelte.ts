import { invoke } from "@tauri-apps/api/core";
import type { AppSettings } from "$lib/types";

class SettingsStore {
  settings = $state<AppSettings>({
    close_to_tray: false,
    auto_play_on_start: false,
    pause_on_device_change: false,
    window_scale: 100,
    font_family: "Segoe UI, 'SF Pro Display', -apple-system, sans-serif",
    lyrics_align: "center",
    lyrics_font_size: 14,
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
    }
  }

  /** Save the current settings snapshot. Returns a promise. */
  async save() {
    try {
      await invoke("set_app_settings", { settings: { ...this.settings } });
    } catch (e) {
      console.error("save settings error:", e);
    }
  }

  /** Update a field and persist. */
  async update<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    this.settings = { ...this.settings, [key]: value };
    await this.save();
  }
}

export const settingsStore = new SettingsStore();
