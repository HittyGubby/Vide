import type { AppSettings } from "$lib/types";

/** Fallback matching the bundled daisyUI dark theme. */
export const DEFAULT_PRIMARY = "#5f5dfd";

/** Pick a readable foreground for a solid background colour. */
function readableOn(hex: string): string {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return "#ffffff";
  const n = parseInt(m[1], 16);
  const r = (n >> 16) & 255;
  const g = (n >> 8) & 255;
  const b = n & 255;
  // Relative luminance (sRGB, simplified).
  const luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
  return luminance > 0.6 ? "#101014" : "#ffffff";
}

/**
 * Apply the user's accent colour. daisyUI exposes the theme as CSS custom
 * properties, so overriding `--color-primary` recolours every component that
 * uses the primary semantic colour.
 */
export function applyTheme(settings: AppSettings) {
  if (typeof document === "undefined") return;
  const primary = settings.theme_primary?.trim() || DEFAULT_PRIMARY;
  const root = document.documentElement;
  root.style.setProperty("--color-primary", primary);
  root.style.setProperty("--color-primary-content", readableOn(primary));
}

/**
 * Apply every appearance setting that is not layout-specific.
 *
 * Window scale and zoom stay in the desktop shell; the theme colour and the
 * preferred font apply to the web client as well (previously the web build
 * never loaded settings at startup, so it silently used the defaults).
 */
export function applyAppearance(settings: AppSettings) {
  if (typeof document === "undefined") return;
  applyTheme(settings);
  if (settings.font_family) {
    document.body.style.fontFamily = settings.font_family;
  }
}