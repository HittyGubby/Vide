import type { AppSettings, ThemeMode } from "$lib/types";
import { applyThemeMode, DEFAULT_PRIMARY, primaryContentFor } from "$lib/theme";

/**
 * Guests (web, not logged in) cannot write the server's settings table, so
 * their appearance and playback preferences live in localStorage. Signed-in
 * users and the local desktop app keep using SQLite.
 */
const KEY = "vide.guest-theme";

export interface GuestSettings {
  mode: ThemeMode;
  primary: string;
  skip_silence: boolean;
  normalize_loudness: boolean;
  loudness_target_lufs: number;
}

function isHex(value: unknown): value is string {
  return typeof value === "string" && /^#?[0-9a-f]{6}$/i.test(value);
}

export function readGuestSettings(): GuestSettings | null {
  if (typeof localStorage === "undefined") return null;
  try {
    const parsed = JSON.parse(localStorage.getItem(KEY) ?? "null");
    if (!parsed || typeof parsed !== "object") return null;
    const mode = parsed.mode === "light" ? "light" : parsed.mode === "dark" ? "dark" : null;
    if (!mode) return null;
    const target = Number(parsed.loudness_target_lufs);
    return {
      mode,
      primary: isHex(parsed.primary) ? parsed.primary : DEFAULT_PRIMARY,
      skip_silence: parsed.skip_silence === true,
      normalize_loudness: parsed.normalize_loudness === true,
      loudness_target_lufs:
        Number.isFinite(target) && target > -40 && target < 0 ? target : -16,
    };
  } catch {
    return null;
  }
}

export function writeGuestSettings(settings: GuestSettings): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(settings));
  } catch {
    /* private mode / storage full: the settings just won't survive a reload */
  }
}

export function clearGuestSettings(): void {
  try {
    localStorage.removeItem(KEY);
  } catch {
    /* ignore */
  }
}

export function applyGuestSettings(theme: GuestSettings): void {
  if (typeof document === "undefined") return;
  applyThemeMode(theme.mode);
  const root = document.documentElement;
  root.style.setProperty("--color-primary", theme.primary);
  root.style.setProperty("--color-primary-content", primaryContentFor(theme.primary));
}

/** Overlay the locally stored guest settings onto the server's settings. */
export function withGuestSettings(settings: AppSettings): AppSettings {
  const guest = readGuestSettings();
  if (!guest) return settings;
  return {
    ...settings,
    theme_mode: guest.mode,
    theme_primary: guest.primary,
    skip_silence: guest.skip_silence,
    normalize_loudness: guest.normalize_loudness,
    loudness_target_lufs: guest.loudness_target_lufs,
  };
}
