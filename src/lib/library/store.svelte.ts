import { invoke } from "$lib/transport";
import { loadClientState, saveClientState } from "$lib/clientState";
import { libraryQuery } from "$lib/stores/libraryQuery.svelte";
import type { Song } from "$lib/types";
import {
  COLUMN_BY_SORT_FIELD,
  SORT_FIELD_BY_COLUMN,
  sortSongs,
  type LibraryColumn,
} from "$lib/library/sorting";

/**
 * The library view's data and sort state.
 *
 * Living in a module singleton (instead of the page component) means switching
 * to settings or the player and coming back reuses the already-loaded list
 * instead of fetching the whole library again — which matters a lot on the
 * phone, where the payload crosses the network.
 */
class LibraryStore {
  songs = $state<Song[]>([]);
  loading = $state(true);
  error = $state<string | null>(null);
  sortField = $state<LibraryColumn>("name");
  sortAsc = $state(true);

  private loadedQuery: string | null = null;
  private hasLoaded = false;
  private sortPrefsLoaded = false;

  /** The active sort: a query SORT BY wins over the column-header fallback. */
  headerSort = $derived.by((): { field: LibraryColumn; asc: boolean } | null => {
    const directive = libraryQuery.sort;
    if (directive) {
      const column = COLUMN_BY_SORT_FIELD[directive.field];
      return column ? { field: column, asc: directive.asc } : null;
    }
    return { field: this.sortField, asc: this.sortAsc };
  });

  sorted = $derived.by(() =>
    sortSongs(this.songs, libraryQuery.sort, this.sortField, this.sortAsc),
  );

  /** Load once for this session; later mounts reuse the data. */
  async init(): Promise<void> {
    if (!this.sortPrefsLoaded) {
      this.sortPrefsLoaded = true;
      await this.loadSortPrefs();
    }
    await libraryQuery.load();
    if (!this.hasLoaded || this.loadedQuery !== libraryQuery.query) {
      await this.reload();
    }
  }

  async reload(): Promise<void> {
    this.loadedQuery = libraryQuery.query;
    await this.load();
    this.hasLoaded = true;
  }

  async load(): Promise<void> {
    this.loading = true;
    this.error = null;
    try {
      const query = libraryQuery.filter.trim();
      this.songs = query
        ? await invoke<Song[]>("search_songs", { query })
        : await invoke<Song[]>("list_library_songs");
    } catch (e) {
      this.error = String(e);
      this.songs = [];
    } finally {
      this.loading = false;
    }
  }

  /** React to the query bar applying a new query. */
  async syncQuery(query: string): Promise<void> {
    if (!this.hasLoaded || this.loadedQuery === query) return;
    await this.reload();
  }

  /** Toggle the column-header sort and mirror it into the query string. */
  async toggleSort(field: LibraryColumn): Promise<void> {
    if (field === "num") return;
    const current = this.headerSort;
    const asc = current && current.field === field ? !current.asc : true;
    this.sortField = field;
    this.sortAsc = asc;
    await saveClientState("list.sort", { field, asc });
    const mapped = SORT_FIELD_BY_COLUMN[field];
    if (mapped) await libraryQuery.setSort(mapped, asc);
  }

  async loadSortPrefs(): Promise<void> {
    const saved = await loadClientState<{ field?: string; asc?: boolean }>("list.sort");
    if (!saved) return;
    const fields: LibraryColumn[] = ["num", "name", "tag", "artist", "album", "time"];
    if (fields.includes(saved.field as LibraryColumn)) {
      this.sortField = saved.field as LibraryColumn;
    }
    if (typeof saved.asc === "boolean") this.sortAsc = saved.asc;
  }

  applyMetadata(updated: Song): void {
    this.songs = this.songs.map((song) => (song.id === updated.id ? updated : song));
  }

  applyMany(updated: Song[]): void {
    const byId = new Map(updated.map((song) => [song.id, song]));
    this.songs = this.songs.map((song) => byId.get(song.id) ?? song);
  }
}

export const libraryStore = new LibraryStore();
