import { invoke } from "$lib/transport";

/**
 * UI state that is not a user-facing setting (column layout, window geometry,
 * scroll position, ...). Stored in SQLite via the settings table, behind a
 * `client:` namespace. localStorage is only used to migrate older data.
 */
export async function loadClientState<T>(key: string): Promise<T | null> {
  try {
    const value = await invoke<T | null>("get_client_state", { key });
    return value ?? null;
  } catch (e) {
    console.error("load client state error:", key, e);
    return null;
  }
}

export async function saveClientState(key: string, value: unknown): Promise<void> {
  try {
    await invoke("set_client_state", { key, value });
  } catch (e) {
    console.error("save client state error:", key, e);
  }
}
