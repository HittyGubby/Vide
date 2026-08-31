import { getAllWindows, type Window } from "@tauri-apps/api/window";
import type { AppSettings } from "$lib/types";

/** Label of the desktop lyrics window declared in tauri.conf.json. */
export const LYRICS_WINDOW_LABEL = "lyrics";

let cached: Window | null = null;

/** The lyrics window, or null when the backend has not created it yet. */
export async function lyricsWindow(): Promise<Window | null> {
  if (cached) return cached;
  try {
    const found = (await getAllWindows()).find((w) => w.label === LYRICS_WINDOW_LABEL);
    if (found) cached = found;
    return found ?? null;
  } catch (e) {
    console.error("get lyrics window error:", e);
    return null;
  }
}

function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Mirror the desktop-lyrics settings onto the window itself.
 *
 * Locked mode ignores every input event, which both stops the user from
 * dragging/resizing the window and lets clicks fall through to whatever is
 * underneath it.
 */
export async function applyLyricsWindowSettings(settings: AppSettings) {
  const win = await lyricsWindow();
  if (!win) return;
  try {
    await win.setResizable(!settings.desktop_lyrics_locked);

    if (!settings.desktop_lyrics_enabled) {
      await win.hide();
      return;
    }

    // Window hints are only honoured once GTK has realized the window, and
    // that never happens while it stays hidden — so show first and apply the
    // rest afterwards.
    await win.show();

    // Click-through is the dangerous one: tao's X11 backend unwraps the
    // underlying GDK window, so calling this before the window is mapped
    // panics inside the event loop and aborts the whole app.
    if (settings.desktop_lyrics_locked) await delay(300);
    await win.setIgnoreCursorEvents(settings.desktop_lyrics_locked);
    await win.setAlwaysOnTop(settings.desktop_lyrics_always_on_top);
  } catch (e) {
    console.error("apply lyrics window settings error:", e);
  }
}

