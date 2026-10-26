import { invoke } from "$lib/transport";
import { parseLyrics } from "$lib/lyrics";

/**
 * Lyrics for the currently playing song, cached per song id.
 *
 * Shared by the desktop detail view and the mobile player so the two cannot
 * disagree about what is loaded (and so switching between them is instant).
 */
class PlayerLyricsStore {
  id: string | null = null;
  text = $state<string | null>(null);
  loading = $state(false);

  private cache = new Map<string, string | null>();

  lines = $derived(this.text ? parseLyrics(this.text) : []);

  async load(id: string | null): Promise<void> {
    if (this.id === id) return;
    this.id = id;
    if (!id) {
      this.text = null;
      this.loading = false;
      return;
    }
    if (this.cache.has(id)) {
      this.text = this.cache.get(id) ?? null;
      this.loading = false;
      return;
    }
    this.loading = true;
    this.text = null;
    try {
      const raw = await invoke<string | null>("get_lyrics", { songId: id });
      this.cache.set(id, raw ?? null);
      if (this.id === id) this.text = raw ?? null;
    } catch {
      this.cache.set(id, null);
      if (this.id === id) this.text = null;
    } finally {
      if (this.id === id) this.loading = false;
    }
  }
}

export const playerLyrics = new PlayerLyricsStore();
