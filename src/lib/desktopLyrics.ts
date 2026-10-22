import {
  availableMonitors,
  getAllWindows,
  PhysicalPosition,
  PhysicalSize,
  type Monitor,
  type Window,
} from "@tauri-apps/api/window";
import { emit } from "@tauri-apps/api/event";
import { loadClientState } from "$lib/clientState";
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

interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Monitor work area in physical pixels (excludes taskbars/docks). */
function monitorRect(monitor: Monitor): Rect {
  return {
    x: monitor.workArea.position.x,
    y: monitor.workArea.position.y,
    w: monitor.workArea.size.width,
    h: monitor.workArea.size.height,
  };
}

/** Squared distance from a point to the nearest edge of a rect. */
function distanceToRect(rect: Rect, x: number, y: number): number {
  const dx = Math.max(rect.x - x, 0, x - (rect.x + rect.w));
  const dy = Math.max(rect.y - y, 0, y - (rect.y + rect.h));
  return dx * dx + dy * dy;
}

/**
 * Keep the lyrics window inside a monitor's work area.
 *
 * An undecorated always-on-top window that was dragged (or restored) fully
 * off-screen is effectively invisible: there is no taskbar entry to bring it
 * back, and the settings toggle only calls show(), which happily shows it at
 * the same off-screen position. Before showing it, shrink it to the monitor if
 * needed and clamp its top-left corner into the nearest work area.
 */
export async function ensureLyricsWindowOnScreen(win: Window): Promise<void> {
  try {
    // Work in physical pixels end to end: monitor positions and window
    // positions use the same space, so mixed-DPI setups need no conversion.
    const [pos, size, monitors] = await Promise.all([
      win.outerPosition(),
      win.outerSize(),
      availableMonitors(),
    ]);
    if (monitors.length === 0) return;

    let x = pos.x;
    let y = pos.y;
    let w = size.width;
    let h = size.height;

    const rects = monitors.map(monitorRect);

    // "Visible" means a meaningful part of the window intersects a work area;
    // a two-pixel sliver is not enough to grab it.
    const visible = rects.some(
      (r) =>
        x + w > r.x + 24 &&
        x < r.x + r.w - 24 &&
        y + h > r.y + 24 &&
        y < r.y + r.h - 24,
    );
    if (visible) return;

    // Pick the monitor nearest to the window centre (usually the one it was
    // last used on, or the primary monitor when it was never placed).
    const cx = x + w / 2;
    const cy = y + h / 2;
    let target = rects[0];
    let best = Number.POSITIVE_INFINITY;
    for (const r of rects) {
      const d = distanceToRect(r, cx, cy);
      if (d < best) {
        best = d;
        target = r;
      }
    }

    // A window wider than the monitor can never fit; shrink it first.
    const newW = Math.min(w, target.w);
    const newH = Math.min(h, target.h);
    if (newW !== w || newH !== h) {
      await win.setSize(new PhysicalSize(Math.round(newW), Math.round(newH)));
      w = newW;
      h = newH;
    }

    x = Math.min(Math.max(x, target.x), Math.max(target.x, target.x + target.w - w));
    y = Math.min(Math.max(y, target.y), Math.max(target.y, target.y + target.h - h));
    await win.setPosition(new PhysicalPosition(Math.round(x), Math.round(y)));
  } catch (e) {
    // Monitor APIs can fail on some window systems; showing the window in its
    // current place is still better than not showing it at all.
    console.error("ensure lyrics window on screen error:", e);
  }
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
    // rest afterwards. Remember whether it was already visible so a settings
    // save does not flash the locator frame on every change.
    const wasVisible = await win.isVisible().catch(() => false);
    await win.show();

    // Restore the last geometry when re-opening the window; do it after the
    // window is mapped because GTK may ignore geometry set while hidden. The
    // clamp below still rescues it if the monitor layout has changed since.
    if (!wasVisible) {
      const geometry = await loadClientState<{
        x: number;
        y: number;
        w: number;
        h: number;
      }>("window.lyrics");
      if (geometry && geometry.w > 0 && geometry.h > 0) {
        try {
          await win.setSize(new PhysicalSize(geometry.w, geometry.h));
          await win.setPosition(new PhysicalPosition(geometry.x, geometry.y));
        } catch (e) {
          console.error("restore lyrics window geometry error:", e);
        }
      }
    }

    // Bring an off-screen window back into a monitor's work area every time
    // it is shown, so the "show desktop lyrics" toggle can always recover it.
    await ensureLyricsWindowOnScreen(win);
    // The window may have no lyrics and be fully transparent (or locked,
    // which makes it click-through). Flash its frame so the user can see
    // where it appeared.
    if (!wasVisible) {
      // Fire-and-forget: a missed pulse must never prevent click-through or
      // always-on-top from being applied below.
      void emit("vide:lyrics-shown", null).catch((e) =>
        console.error("emit lyrics shown error:", e),
      );
    }

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
