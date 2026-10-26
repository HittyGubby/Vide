import { invoke } from "$lib/transport";
import { authReady } from "$lib/stores/auth.svelte";
import type { SearchPreset } from "$lib/types";

interface LibraryQueryState {
  history: string[];
  last_query: string;
}

export interface QuerySort {
  field: string;
  asc: boolean;
}

const SORT_CLAUSE =
  /(?:^|\s)SORT\s+BY\s+([A-Za-z_][A-Za-z0-9_]*)(?:\s+(ASC|DESC))?\s*$/i;

/** Rebuild a query with a fragment inserted before any SORT BY clause. */
export function insertQueryFragment(
  query: string,
  fragment: string,
  separator = " ",
): string {
  const { filter, sort } = splitSortClause(query);
  const next = filter ? filter + separator + fragment : fragment;
  return sort
    ? `${next} SORT BY ${sort.field} ${sort.asc ? "ASC" : "DESC"}`
    : next;
}

/**
 * Split a trailing SORT BY clause from the filter query. The directive is kept
 * in the user-visible query string so saved queries/history include the sort,
 * but the database search itself only receives the filter.
 */
export function splitSortClause(query: string): {
  filter: string;
  sort: QuerySort | null;
} {
  const match = SORT_CLAUSE.exec(query);
  if (!match) return { filter: query.trim(), sort: null };

  // Do not treat "SORT BY" inside a quoted CONTAINS value as a directive.
  let inQuote = false;
  let escaped = false;
  for (let i = 0; i < match.index; i++) {
    const ch = query[i];
    if (escaped) {
      escaped = false;
      continue;
    }
    if (ch === "\\") {
      escaped = true;
      continue;
    }
    if (ch === '"') inQuote = !inQuote;
  }
  if (inQuote) return { filter: query.trim(), sort: null };

  const field = match[1].toLowerCase();
  const asc = (match[2] ?? "ASC").toUpperCase() === "ASC";
  return {
    filter: query.slice(0, match.index).trim(),
    sort: { field, asc },
  };
}

/**
 * The single source of truth for the library view:
 *
 *   query     - the query currently applied to the list
 *   history   - recently executed queries (SQLite, separate from online search)
 *   saved     - named queries (the old "search presets")
 *
 * The list page reacts to query; the titlebar query bar edits it. Saved
 * queries and history are persisted in SQLite, not localStorage.
 */
class LibraryQueryStore {
  /** The query currently driving the list. Empty means "all library songs". */
  query = $state("");
  /** Query sent to the database (the SORT BY tail removed). */
  filter = $state("");
  /** Optional sort directive carried in the query string. */
  sort = $state<QuerySort | null>(null);
  history = $state<string[]>([]);
  saved = $state<SearchPreset[]>([]);
  loaded = $state(false);

  private loadPromise: Promise<void> | null = null;

  /** Load once; concurrent callers share the same promise. */
  load(): Promise<void> {
    if (this.loadPromise) return this.loadPromise;
    this.loadPromise = this.loadInternal();
    return this.loadPromise;
  }

  private async loadInternal(): Promise<void> {
    await authReady;
    try {
      const [state, saved] = await Promise.all([
        invoke<LibraryQueryState>("get_library_query_state"),
        invoke<SearchPreset[]>("list_search_presets"),
      ]);
      this.history = state.history ?? [];
      this.saved = saved ?? [];
      this.applyQuery(state.last_query ?? "");
    } catch (e) {
      console.error("load library query state error:", e);
    } finally {
      this.loaded = true;
    }
  }

  /** Apply a query and record it (last query + history) in SQLite. */
  async execute(query: string): Promise<void> {
    // A very fast user can submit before the initial restore finishes; never
    // let that late load overwrite the query they just entered.
    await this.load();
    const next = query.trim();
    this.applyQuery(next);
    try {
      const state = await invoke<LibraryQueryState>("record_library_query", {
        query: next,
      });
      this.history = state.history ?? [];
    } catch (e) {
      console.error("record library query error:", e);
    }
  }

  private applyQuery(raw: string) {
    const query = raw.trim();
    this.query = query;
    const { filter, sort } = splitSortClause(query);
    this.filter = filter;
    this.sort = sort;
  }

  /** Replace/append the trailing sort directive; does not add history noise. */
  async setSort(field: string, asc: boolean): Promise<void> {
    const base = this.filter;
    const clause = "SORT BY " + field + " " + (asc ? "ASC" : "DESC");
    const next = base ? base + " " + clause : clause;
    this.applyQuery(next);
    try {
      await invoke("set_last_library_query", { query: next });
    } catch (e) {
      console.error("save library sort error:", e);
    }
  }

  async removeHistory(query: string): Promise<void> {
    await this.load();
    try {
      const state = await invoke<LibraryQueryState>("remove_query_history", {
        query,
      });
      this.history = state.history ?? [];
    } catch (e) {
      console.error("remove query history error:", e);
    }
  }

  async clearHistory(): Promise<void> {
    await this.load();
    try {
      const state = await invoke<LibraryQueryState>("clear_query_history");
      this.history = state.history ?? [];
    } catch (e) {
      console.error("clear query history error:", e);
    }
  }

  /** The saved query with exactly this query string, if any. */
  savedFor(query: string): SearchPreset | undefined {
    const trimmed = query.trim();
    return this.saved.find((preset) => preset.query.trim() === trimmed);
  }

  /** Save the current query under a name, replacing a same-named entry. */
  async save(name: string, query: string): Promise<SearchPreset> {
    await this.load();
    const saved = await invoke<SearchPreset>("save_search_preset", {
      name,
      query,
    });
    this.saved = await invoke<SearchPreset[]>("list_search_presets");
    return saved;
  }

  async remove(id: string): Promise<void> {
    await this.load();
    await invoke("delete_search_preset", { id });
    this.saved = await invoke<SearchPreset[]>("list_search_presets");
  }
}

export const libraryQuery = new LibraryQueryStore();
