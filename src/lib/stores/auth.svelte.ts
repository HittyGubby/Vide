import { isTauri } from "$lib/isTauri";
import { browserAuthStatus, isRemoteGuest, isRemoteMode } from "$lib/transport";
import { serverStatus } from "$lib/stores/serverStatus.svelte";

export const auth = $state({ authenticated: false });

/**
 * Resolves once the initial session probe has finished, so stores that read
 * server-backed data know whether to use the server or localStorage.
 */
let markAuthReady: () => void = () => {};
export const authReady = new Promise<void>((resolve) => {
  markAuthReady = resolve;
});
// The Tauri app never calls refreshAuth (it has no cookie session); without
// this the history/query stores would await authReady forever.
if (isTauri) markAuthReady();

/** Refresh the browser cookie session state. No-op on Tauri. */
export async function refreshAuth(): Promise<void> {
  if (isTauri) {
    markAuthReady();
    return;
  }
  try {
    auth.authenticated = await browserAuthStatus();
  } catch {
    auth.authenticated = false;
  } finally {
    markAuthReady();
  }
}

/** Editing/writing: requires a session (local Tauri always has one). */
export function canManage(): boolean {
  if (isRemoteMode()) return !isRemoteGuest() || auth.authenticated;
  return isTauri || auth.authenticated;
}

/**
 * Downloading is reading, not editing: guests may download when the server
 * exposes media publicly (the same rule that gates streaming).
 */
export function canDownload(): boolean {
  if (canManage()) return true;
  return serverStatus.publicMedia;
}
