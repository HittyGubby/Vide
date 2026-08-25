import { invoke } from "@tauri-apps/api/core";
import type { SearchPreset } from "$lib/types";

class SearchPresetStore {
  presets = $state<SearchPreset[]>([]);
  /** ID of the last-selected preset, or null for "all songs". */
  activeId = $state<string | null>(null);
  loaded = $state(false);

  /** Load presets + the last active one from the backend. */
  async load() {
    try {
      this.presets = await invoke<SearchPreset[]>("list_search_presets");
      const last = await invoke<string | null>("get_last_search_preset");
      // Only restore if the preset still exists.
      this.activeId =
        last && this.presets.some((p) => p.id === last) ? last : null;
    } catch {
      this.presets = [];
      this.activeId = null;
    } finally {
      this.loaded = true;
    }
  }

  /** The currently active preset object, if any. */
  get active(): SearchPreset | null {
    return this.presets.find((p) => p.id === this.activeId) ?? null;
  }

  /** Create or update a preset by name. Returns the saved preset. */
  async save(name: string, query: string): Promise<SearchPreset> {
    const saved = await invoke<SearchPreset>("save_search_preset", {
      name,
      query,
    });
    await this.load();
    return saved;
  }

  /** Delete a preset; if it was active, fall back to "all songs". */
  async remove(id: string) {
    await invoke("delete_search_preset", { id });
    if (this.activeId === id) this.activeId = null;
    await this.load();
  }

  /** Select a preset and persist it as the last-used one. */
  async select(id: string | null) {
    this.activeId = id;
    try {
      await invoke("set_last_search_preset", { id: id ?? "" });
    } catch {
      /* non-fatal */
    }
  }
}

export const searchPresets = new SearchPresetStore();
