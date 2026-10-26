import { isTauri } from "$lib/isTauri";
import { browserAuthStatus, isRemoteGuest, isRemoteMode } from "$lib/transport";
import { serverStatus } from "$lib/stores/serverStatus.svelte";

export const auth = $state({ authenticated: false });

/** Refresh the browser cookie session state. No-op on Tauri. */
export async function refreshAuth(): Promise<void> {
  if (isTauri) return;
  try {
    auth.authenticated = await browserAuthStatus();
  } catch {
    auth.authenticated = false;
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
