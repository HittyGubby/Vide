import { isTauri } from "$lib/isTauri";
import { remoteSession } from "$lib/transport";

/**
 * Public capabilities of the server the browser is talking to.
 *
 * The web client is always backed by a Vide server, but it had no way to know
 * whether that server exposes media to guests — `/api/v1/status` reports it
 * and only the Tauri remote flow used to read it. Download permissions and the
 * "play as guest" affordance depend on it.
 */
class ServerStatusStore {
  loaded = $state(false);
  /** Guests may stream/download media without a session. */
  publicMedia = $state(false);

  async load(): Promise<void> {
    const session = remoteSession();
    // Local Tauri has no HTTP server and therefore no public-media policy.
    if (isTauri && !session) {
      this.loaded = true;
      return;
    }
    try {
      const response = await fetch((session?.baseUrl ?? "") + "/api/v1/status");
      if (!response.ok) return;
      const data = (await response.json()) as { public_media?: boolean };
      this.publicMedia = Boolean(data.public_media);
      this.loaded = true;
    } catch {
      /* offline: keep the previous value */
    }
  }
}

export const serverStatus = new ServerStatusStore();
