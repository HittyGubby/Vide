import type { AppSettings, ThemeMode } from "$lib/types";
import { applyThemeMode, DEFAULT_PRIMARY, primaryContentFor } from "$lib/theme";

/**
 * Guests (web, not logged in) cannot write the server's settings table, so
 * their dark/light choice and accent colour live in localStorage. Signed-in
 * users and the local desktop app keep using SQLite.
 */
const KEY = "vide.guest-theme";

export interface GuestTheme {
  mode: ThemeMode;
  primary: string;
}

function isHex(value: unknown): value is string {
  return typeof value === "string" && /^#?[0-9a-f]{6}$/i.test(value);
}

export function readGuestTheme(): GuestTheme | null {
  if (typeof localStorage === "undefined") return null;
  try {
    const parsed = JSON.parse(localStorage.getItem(KEY) ?? "null");
    if (!parsed || typeof parsed !== "object") return null;
    const mode = parsed.mode === "light" ? "light" : parsed.mode === "dark" ? "dark" : null;
    if (!mode) return null;
    return { mode, primary: isHex(parsed.primary) ? parsed.primary : DEFAULT_PRIMARY };
  } catch {
    return null;
  }
}

export function writeGuestTheme(theme: GuestTheme): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(theme));
  } catch {
    /* private mode / storage full: the theme just won't survive a reload */
  }
}

export function clearGuestTheme(): void {
  try {
    localStorage.removeItem(KEY);
  } catch {
    /* ignore */
  }
}

export function applyGuestTheme(theme: GuestTheme): void {
  if (typeof document === "undefined") return;
  applyThemeMode(theme.mode);
  const root = document.documentElement;
  root.style.setProperty("--color-primary", theme.primary);
  root.style.setProperty("--color-primary-content", primaryContentFor(theme.primary));
}

/** Overlay the locally stored guest theme onto the server's settings. */
export function withGuestTheme(settings: AppSettings): AppSettings {
  const theme = readGuestTheme();
  return theme ? { ...settings, theme_mode: theme.mode, theme_primary: theme.primary } : settings;
}
