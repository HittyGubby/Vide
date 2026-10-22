import { invoke } from "$lib/transport";

/**
 * Online search history (the /search page), separate from the library query
 * history. Persisted in SQLite; no localStorage.
 */
class SearchStore {
  history = $state<string[]>([]);
  private ready: Promise<void>;

  constructor() {
    this.ready = this.load();
  }

  async load() {
    const legacyKey = "vide_search_history";
    try {
      let history = await invoke<string[]>("get_online_search_history");
      if (history.length === 0) {
        // One-time migration from the old localStorage history.
        const legacy = localStorage.getItem(legacyKey);
        if (legacy) {
          const parsed: unknown = JSON.parse(legacy);
          if (Array.isArray(parsed)) {
            const cleaned = parsed
              .filter((entry): entry is string => typeof entry === "string")
              .slice(0, 20);
            if (cleaned.length) {
              await invoke("set_online_search_history", { history: cleaned });
              history = cleaned;
            }
          }
          localStorage.removeItem(legacyKey);
        }
      } else {
        localStorage.removeItem(legacyKey);
      }
      this.history = history;
    } catch {
      this.history = [];
    }
  }

  async add(query: string) {
    const trimmed = query.trim();
    if (!trimmed) return;
    await this.ready;
    try {
      this.history = await invoke<string[]>("add_online_search_history", {
        query: trimmed,
      });
    } catch {
      /* keep the current list */
    }
  }

  async remove(query: string) {
    await this.ready;
    this.history = this.history.filter((item) => item !== query);
    await this.persist();
  }

  async clear() {
    await this.ready;
    this.history = [];
    await this.persist();
  }

  private async persist() {
    try {
      await invoke("set_online_search_history", { history: this.history });
    } catch {
      /* non-fatal */
    }
  }
}

export const searchStore = new SearchStore();
