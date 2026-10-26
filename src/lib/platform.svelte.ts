import { isTauri } from "$lib/isTauri";
import { isRemoteMode } from "$lib/transport";

/**
 * Phones use the mobile UI; tablets keep the desktop UI (decided).
 *
 * The tier is decided by the SHORT side, not the width, so a phone held in
 * landscape (852x393) stays on the mobile UI while an iPad (768x1024) keeps
 * the desktop one. User-agent sniffing is deliberately not used: iPadOS
 * reports a desktop Safari UA, and "request desktop site" would fight the
 * layout instead of helping it.
 */
const MOBILE_SHORT_SIDE = 600;

export type UITier = "mobile" | "desktop";

class PlatformStore {
  /** CSS pixels; kept in sync with resize/orientation changes. */
  width = $state(typeof window === "undefined" ? 1280 : window.innerWidth);
  height = $state(typeof window === "undefined" ? 800 : window.innerHeight);
  /** True for touch-first devices; only used for interaction tweaks. */
  coarse = $state(false);
  ready = $state(false);

  get tier(): UITier {
    const short = Math.min(this.width, this.height);
    return short > 0 && short < MOBILE_SHORT_SIDE ? "mobile" : "desktop";
  }

  /** Whether the phone layout (drawer + mini player) is in use. */
  get mobileUI(): boolean {
    return this.tier === "mobile";
  }

  get orientation(): "portrait" | "landscape" {
    return this.height >= this.width ? "portrait" : "landscape";
  }

  /** Start tracking the viewport. Call once from the root layout. */
  init(): () => void {
    if (typeof window === "undefined") return () => {};
    const pointer = window.matchMedia("(pointer: coarse)");
    const update = () => {
      this.width = window.innerWidth;
      this.height = window.innerHeight;
      this.coarse = pointer.matches;
      this.ready = true;
    };
    update();
    window.addEventListener("resize", update);
    window.addEventListener("orientationchange", update);
    pointer.addEventListener("change", update);
    return () => {
      window.removeEventListener("resize", update);
      window.removeEventListener("orientationchange", update);
      pointer.removeEventListener("change", update);
    };
  }
}

export const platform = new PlatformStore();

/* ── Capabilities ──────────────────────────────────────
 * Features are gated by what the current runtime can actually do, instead of
 * scattering isTauri / isRemoteMode checks through the components. */

/** Local SQLite library, local scan, local file paths. */
export function hasLocalLibrary(): boolean {
  return isTauri && !isRemoteMode();
}

/** Native window chrome (minimize / maximize / close, window geometry). */
export function hasWindowControls(): boolean {
  return isTauri;
}

export function hasGlobalShortcuts(): boolean {
  return isTauri;
}

/** The separate always-on-top lyrics window. */
export function hasDesktopLyrics(): boolean {
  return isTauri;
}

/** Native file pickers (Tauri dialog plugin). */
export function hasFileDialogs(): boolean {
  return isTauri;
}

/** Window zoom / font size: desktop-only layout knobs. */
export function hasWindowScale(): boolean {
  return isTauri;
}
