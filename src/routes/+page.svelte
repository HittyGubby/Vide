<script lang="ts">
  import { invoke } from "$lib/transport";
  import { onDestroy, onMount, tick } from "svelte";
  import {
    createTable,
    tableFeatures,
    columnSizingFeature,
    columnResizingFeature,
    columnOrderingFeature,
    columnVisibilityFeature,
    type ColumnSizingState,
    type ColumnOrderState,
    type ColumnVisibilityState,
    type Updater,
  } from "@tanstack/svelte-table";
  import { page } from "$app/stores";
  import { listen } from "@tauri-apps/api/event";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { Search, Play, Download, ListPlus, Info, Pencil, Trash2 } from "lucide-svelte";
  import { player } from "$lib/stores/player.svelte";
  import { downloadTarget as remoteDownloadTarget, isRemoteMode } from "$lib/transport";
  import { isTauri } from "$lib/isTauri";
  import { save, open } from "@tauri-apps/plugin-dialog";
  import { downloadManyWeb } from "$lib/download";
  import { canManage } from "$lib/stores/auth.svelte";
  import { libraryQuery } from "$lib/stores/libraryQuery.svelte";
  import { loadClientState, saveClientState } from "$lib/clientState";
  import TagEditor from "$lib/components/tag/TagEditor.svelte";
  import ContextMenu from "$lib/components/song/ContextMenu.svelte";
  import SongHoverCard from "$lib/components/song/SongHoverCard.svelte";
  import MetadataEditor from "$lib/components/song/MetadataEditor.svelte";
  import MultiMetadataEditor from "$lib/components/song/MultiMetadataEditor.svelte";
  import MultiTagEditor from "$lib/components/song/MultiTagEditor.svelte";
  import type { Song, SongDetails, Tag } from "$lib/types";
  import type { MenuItem } from "$lib/components/song/ContextMenu.svelte";

  type ColumnId = "num" | "name" | "tag" | "artist" | "album" | "time";

  interface ColumnDef {
    id: ColumnId;
    label: string;
    /** Fixed columns (# / 时长) keep an exact width and cannot be resized. */
    fixed?: number;
    /** Default pixel width. */
    defWidth: number;
    /** Minimum pixel width when resizing. */
    min: number;
  }

  const COLUMN_DEFS: ColumnDef[] = [
    { id: "num", label: "#", fixed: 40, defWidth: 40, min: 40 },
    { id: "name", label: "名称", defWidth: 300, min: 120 },
    { id: "tag", label: "标签", defWidth: 160, min: 80 },
    { id: "artist", label: "歌手", defWidth: 130, min: 80 },
    { id: "album", label: "专辑", defWidth: 140, min: 80 },
    { id: "time", label: "时长", fixed: 52, defWidth: 52, min: 52 },
  ];

  // Column layout (width / order / visibility) is owned by TanStack Table,
  // which tracks the resize gesture, min/max clamping and touch input. The
  // three state slices are persisted here so they survive a restart.
  const COL_KEY = "vide_list_columns";

  interface ColumnPrefs {
    sizing: ColumnSizingState;
    order: ColumnOrderState;
    visibility: ColumnVisibilityState;
  }

  function defOf(id: string): ColumnDef {
    return COLUMN_DEFS.find((d) => d.id === id)!;
  }

  function defaultPrefs(): ColumnPrefs {
    return {
      sizing: Object.fromEntries(COLUMN_DEFS.map((d) => [d.id, d.defWidth])),
      order: COLUMN_DEFS.map((d) => d.id),
      visibility: Object.fromEntries(COLUMN_DEFS.map((d) => [d.id, true])),
    };
  }

  /** Validate stored column prefs and fill in any missing columns. */
  function normalizePrefs(data: unknown): ColumnPrefs {
    const dflt = defaultPrefs();
    if (!data || typeof data !== "object" || Array.isArray(data)) return dflt;
    const raw = data as Record<string, unknown>;

    const sizing: ColumnSizingState = { ...dflt.sizing };
    const rawSizing = (raw.sizing ?? {}) as Record<string, unknown>;
    for (const d of COLUMN_DEFS) {
      const w = Number(rawSizing[d.id]);
      if (Number.isFinite(w) && w > 0) sizing[d.id] = Math.max(w, d.min);
    }

    const order: ColumnOrderState = Array.isArray(raw.order)
      ? raw.order.filter((id): id is string => COLUMN_DEFS.some((d) => d.id === id))
      : [];
    for (const d of COLUMN_DEFS) if (!order.includes(d.id)) order.push(d.id);

    const visibility: ColumnVisibilityState = { ...dflt.visibility };
    const rawVis = (raw.visibility ?? {}) as Record<string, unknown>;
    for (const d of COLUMN_DEFS) {
      if (typeof rawVis[d.id] === "boolean") visibility[d.id] = rawVis[d.id] as boolean;
    }
    // Keep at least one column on screen.
    if (!COLUMN_DEFS.some((d) => visibility[d.id] !== false)) visibility.name = true;

    return { sizing, order, visibility };
  }

  let prefs = $state<ColumnPrefs>(defaultPrefs());
  let prefsSaveTimer: ReturnType<typeof setTimeout> | undefined;
  let scrollSaveTimer: ReturnType<typeof setTimeout> | undefined;

  // Column layout is UI state, stored in SQLite (client:list.columns).
  function savePrefs() {
    if (prefsSaveTimer) clearTimeout(prefsSaveTimer);
    prefsSaveTimer = setTimeout(() => {
      prefsSaveTimer = undefined;
      void saveClientState("list.columns", $state.snapshot(prefs));
    }, 250);
  }

  async function loadColumnPrefs() {
    const stored = await loadClientState<unknown>("list.columns");
    if (stored) {
      prefs = normalizePrefs(stored);
      localStorage.removeItem(COL_KEY);
    } else {
      // One-time migration from the old localStorage layout.
      try {
        const legacy = localStorage.getItem(COL_KEY);
        if (legacy) {
          prefs = normalizePrefs(JSON.parse(legacy));
          await saveClientState("list.columns", $state.snapshot(prefs));
          localStorage.removeItem(COL_KEY);
        }
      } catch {
        /* ignore corrupt legacy data */
      }
    }
    // The table is created before the async load; fit the restored widths to
    // the current container once the header has rendered.
    await tick();
    if (headerEl) fitToWidth(headerEl.clientWidth - scrollbarWidth);
  }

  onDestroy(() => {
    if (prefsSaveTimer) {
      clearTimeout(prefsSaveTimer);
      prefsSaveTimer = undefined;
      void saveClientState("list.columns", $state.snapshot(prefs));
    }
    if (scrollSaveTimer) {
      clearTimeout(scrollSaveTimer);
      scrollSaveTimer = undefined;
    }
    if (listEl) void saveClientState("list.scroll", { top: listEl.scrollTop });
  });

  /** Resolve a TanStack updater (a value or an updater function). */
  function resolveUpdater<T>(updater: Updater<T>, prev: T): T {
    return typeof updater === "function" ? (updater as (old: T) => T)(prev) : updater;
  }

  /** Ids of the currently visible columns, in display order. */
  function visibleIds(): string[] {
    return prefs.order.filter((id) => prefs.visibility[id] !== false);
  }

  /** The visible, resizable column after `id` — the one that gives up width. */
  function nextResizable(id: string): string | null {
    const ids = visibleIds();
    for (let i = ids.indexOf(id) + 1; i < ids.length; i++) {
      if (defOf(ids[i]).fixed == null) return ids[i];
    }
    return null;
  }

  /**
   * TanStack resizes a single column, which would change the table's total
   * width and break the "row fills the container" layout. Take the delta from
   * the next resizable column instead, so the total stays constant and only
   * the dragged boundary moves.
   */
  function applySizing(updater: Updater<ColumnSizingState>): ColumnSizingState {
    const next = resolveUpdater(updater, prefs.sizing);
    let changed: string | null = null;
    let delta = 0;
    for (const id of Object.keys(next)) {
      const prev = prefs.sizing[id];
      if (prev != null && prev !== next[id]) {
        changed = id;
        delta = next[id] - prev;
      }
    }
    if (changed == null || delta === 0) return next;
    const neighbour = nextResizable(changed);
    if (neighbour == null) return next;
    const neighbourSize = next[neighbour] ?? defOf(neighbour).defWidth;
    // Growing: the neighbour gives up space, never dropping below its minimum.
    const applied = delta > 0 ? Math.min(delta, neighbourSize - defOf(neighbour).min) : delta;
    return {
      ...next,
      [changed]: prefs.sizing[changed] + applied,
      [neighbour]: neighbourSize - applied,
    };
  }

  /** Set one column's width, keeping the table total constant. */
  function setWidth(id: string, width: number) {
    const def = defOf(id);
    if (def.fixed != null) return;
    prefs = { ...prefs, sizing: applySizing({ ...prefs.sizing, [id]: Math.max(width, def.min) }) };
    savePrefs();
  }

  /** Scale the resizable columns so the table keeps filling `width`. */
  function fitToWidth(width: number) {
    const visible = visibleIds();
    const resizable = visible.filter((id) => defOf(id).fixed == null);
    const fixedTotal = visible.reduce((s, id) => s + (defOf(id).fixed ?? 0), 0);
    const target = width - fixedTotal;
    const total = resizable.reduce((s, id) => s + (prefs.sizing[id] ?? defOf(id).defWidth), 0);
    if (resizable.length === 0 || total <= 0 || target <= 0) return;
    const k = target / total;
    if (Math.abs(k - 1) < 0.01) return;
    const sizing: ColumnSizingState = { ...prefs.sizing };
    for (const id of resizable) {
      const cur = sizing[id] ?? defOf(id).defWidth;
      sizing[id] = Math.max(defOf(id).min, Math.round(cur * k));
    }
    prefs = { ...prefs, sizing };
    savePrefs();
  }

  let dragColId = $state<string | null>(null);

  // ── Reorder (drag the header label) — horizontal-only drag ghost ──
  function handleHeaderDragStart(e: DragEvent, id: string) {
    dragColId = id;
    const dt = e.dataTransfer;
    if (!dt) return;
    dt.setData("text/plain", id);
    dt.effectAllowed = "move";
    // Use a tiny horizontal bar as the drag image so the column text
    // does not follow the cursor (only a horizontal ghost is shown).
    const ghost = document.createElement("div");
    ghost.style.width = "40px";
    ghost.style.height = "4px";
    ghost.style.background = "var(--color-primary)";
    ghost.style.borderRadius = "2px";
    ghost.style.opacity = "0.8";
    document.body.appendChild(ghost);
    dt.setDragImage(ghost, 20, 2);
    setTimeout(() => ghost.remove(), 0);
  }

  function handleHeaderDragOver(e: DragEvent, id: string) {
    e.preventDefault();
    if (dragColId && dragColId !== id) {
      (e.currentTarget as HTMLElement).classList.add("drop-target");
    }
  }

  function handleHeaderDragLeave(e: DragEvent) {
    (e.currentTarget as HTMLElement).classList.remove("drop-target");
  }

  function handleHeaderDrop(e: DragEvent, targetId: string) {
    e.preventDefault();
    (e.currentTarget as HTMLElement).classList.remove("drop-target");
    const sourceId = dragColId;
    dragColId = null;
    if (!sourceId || sourceId === targetId) return;
    const from = prefs.order.indexOf(sourceId);
    const to = prefs.order.indexOf(targetId);
    if (from < 0 || to < 0) return;
    const order = [...prefs.order];
    order.splice(to, 0, ...order.splice(from, 1));
    prefs = { ...prefs, order };
    savePrefs();
  }

  function handleHeaderDragEnd() {
    dragColId = null;
  }

  // Double-click the resize handle to restore the column's default width.
  function resetWidth(e: MouseEvent, id: string) {
    e.preventDefault();
    e.stopPropagation();
    setWidth(id, defOf(id).defWidth);
  }

  // ── Header context menu (show/hide columns) ───────
  let colMenu = $state<{ x: number; y: number } | null>(null);
  let colMenuItems = $state<MenuItem[]>([]);

  function openColMenu(e: MouseEvent) {
    e.preventDefault();
    const canHide = visibleIds().length > 1;
    colMenuItems = COLUMN_DEFS.map((def) => {
      const shown = prefs.visibility[def.id] !== false;
      return {
        label: (shown ? "✓ " : "   ") + def.label,
        disabled: shown && !canHide,
        action: () => {
          if (shown && !canHide) return;
          prefs = {
            ...prefs,
            visibility: { ...prefs.visibility, [def.id]: !shown },
          };
          savePrefs();
        },
      };
    });
    colMenu = { x: e.clientX, y: e.clientY };
  }
  // ── Song data ─────────────────────────────────────
  let songs = $state<Song[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let currentMediaId = $state<string | null>(null);
  let sortField = $state<ColumnId>("name");
  let sortAsc = $state(true);
  let editingSong = $state<Song | null>(null);
  let editingMetaSong = $state<Song | null>(null);
  // The last query actually reflected in `songs`; the effect below only
  // reloads when the store's applied query differs.
  let lastLoadedQuery = $state<string | null>(null);
  let initialLoadDone = $state(false);
  let listEl = $state<HTMLDivElement | null>(null);
  // Header element: always rendered, so it can measure the available width
  // even while the list is empty or still loading.
  let headerEl = $state<HTMLDivElement | null>(null);
  // Virtual window state (so long lists stay smooth).
  let viewTop = $state(0);        // scrollTop (px)
  let viewHeight = $state(0);     // viewport height (px)
  // Width of the list's vertical scrollbar, so the header can pad its right
  // edge to align with the rows (0 when there is no scrollbar).
  let scrollbarWidth = $state(0);

  function measureScrollbar() {
    if (listEl) {
      scrollbarWidth = Math.max(0, listEl.offsetWidth - listEl.clientWidth);
    }
  }

  function onListScroll() {
    if (listEl) {
      viewTop = listEl.scrollTop;
      viewHeight = listEl.clientHeight || 0;
      measureScrollbar();
    }
    clearHover();
    saveScroll();
  }

  // Re-measure the scrollbar width whenever the list element attaches or the
  // rendered rows change, so the header's right padding stays aligned.
  $effect(() => {
    const _tick = viewHeight;
    const _el = listEl;
    const _n = sortedSongs.length;
    if (_el) {
      // Runs after DOM update.
      queueMicrotask(() => measureScrollbar());
    }
  });

  // Scroll position is UI state, stored in SQLite (client:list.scroll).
  // The localStorage key is only read once to migrate old data.
  const SCROLL_KEY = "vide_list_scroll";
  function saveScroll() {
    if (!listEl) return;
    const top = listEl.scrollTop;
    if (scrollSaveTimer) clearTimeout(scrollSaveTimer);
    scrollSaveTimer = setTimeout(() => {
      scrollSaveTimer = undefined;
      void saveClientState("list.scroll", { top });
    }, 300);
  }

  async function restoreScroll() {
    let saved = await loadClientState<{ top?: number } | number>("list.scroll");
    if (saved == null) {
      try {
        const legacy = localStorage.getItem(SCROLL_KEY);
        if (legacy != null) {
          const top = parseInt(legacy, 10) || 0;
          saved = { top };
          await saveClientState("list.scroll", saved);
          localStorage.removeItem(SCROLL_KEY);
        }
      } catch {
        /* ignore corrupt legacy data */
      }
    }
    const top = typeof saved === "number" ? saved : saved?.top;
    if (saved != null) localStorage.removeItem(SCROLL_KEY);
    if (typeof top === "number" && listEl) {
      const max = Math.max(0, listEl.scrollHeight - listEl.clientHeight);
      listEl.scrollTop = Math.min(Math.max(0, top), max);
      // Use the browser-clamped value: setting a stale saved offset can make
      // the virtual window render rows at the wrong place.
      viewTop = listEl.scrollTop;
      viewHeight = listEl.clientHeight || 0;
    }
  }

  // Locate the given song id in the list: highlight + scroll it into view.
  const ROW_H = 36; // song-row height in px
  let locatorTarget = $state<string | null>(null);
  async function scrollToSong(id: string) {
    locatorTarget = id;
    // Find the index in the current sorted list and center the row.
    for (let attempt = 0; attempt < 40; attempt++) {
      await tick();
      const idx = sortedSongs.findIndex((s) => s.id === id);
      if (idx >= 0 && listEl) {
        const target = idx * ROW_H - (listEl.clientHeight / 2) + ROW_H / 2;
        listEl.scrollTop = Math.max(0, target);
        // The virtual list relies on viewTop; update it explicitly because a
        // programmatic scrollTop assignment does not always fire onscroll.
        viewTop = listEl.scrollTop;
        viewHeight = listEl.clientHeight || 0;
        saveScroll();
        break;
      }
      await new Promise((r) => setTimeout(r, 50));
    }
    setTimeout(() => (locatorTarget = null), 2500);
  }

  // The background startup scan (and the settings-page "scan now" action)
  // announce changes; refresh the current view without disturbing the query.
  onMount(() => {
    if (!isTauri) return;
    let unlisten: (() => void) | undefined;
    void listen("vide:library-changed", () => {
      void loadSongs();
    }).then((fn) => (unlisten = fn));
    return () => unlisten?.();
  });

  onMount(async () => {
    await loadColumnPrefs();
    await loadSortPrefs();
    await libraryQuery.load();
    lastLoadedQuery = libraryQuery.query;
    initialLoadDone = true;

    // A locate request must be able to see the whole library: clear the
    // restored query first, otherwise the song may be filtered out.
    const locateId = $page.url.searchParams.get("locate");
    if (locateId) {
      lastLoadedQuery = "";
      await libraryQuery.execute("");
      await loadSongs();
      await scrollToSong(locateId);
    } else {
      await loadSongs();
      await tick();
      await restoreScroll();
    }
  });

  // The titlebar query bar writes libraryQuery.query; reload the list whenever
  // the applied query changes (Enter, a saved query, a history item, clear).
  $effect(() => {
    if (!initialLoadDone || !libraryQuery.loaded) return;
    const query = libraryQuery.query;
    if (lastLoadedQuery === query) return;
    lastLoadedQuery = query;
    void loadSongs();
  });

  // Reactive locate: the player bar signals a monotonic request id so
  // repeated right-clicks on the SAME song always fire (a URL query does not
  // change when the id is identical, so SvelteKit would skip navigation).
  let _lastLocateReq = $state(-1);
  $effect(() => {
    const req = player.locateRequestId;
    if (req > 0 && req !== _lastLocateReq) {
      _lastLocateReq = req;
      const locateId = player.locateSongId;
      if (locateId) {
        // Force "全部歌曲" so the song is present in this view. Set the
        // bookkeeping first so the query effect does not also reload.
        lastLoadedQuery = "";
        void libraryQuery.execute("").then(async () => {
          await loadSongs();
          await scrollToSong(locateId);
        });
      }
    } else if (req === 0) {
      _lastLocateReq = -1;
    }
  });

  $effect(() => {
    currentMediaId = player.mediaId;
  });

  async function loadSongs() {
    loading = true;
    error = null;
    try {
      const query = libraryQuery.filter.trim();
      songs = query
        ? await invoke<Song[]>("search_songs", { query })
        : await invoke<Song[]>("list_library_songs");
    } catch (e) {
      error = String(e);
      songs = [];
    } finally {
      loading = false;
    }
    // The new result set may be shorter than the old scroll offset. Clamp it
    // before the virtual window computes its padding, otherwise the list can
    // stay scrolled into empty space (and fight scroll anchoring).
    await tick();
    if (listEl) {
      const max = Math.max(0, listEl.scrollHeight - listEl.clientHeight);
      if (listEl.scrollTop > max) listEl.scrollTop = max;
      viewTop = listEl.scrollTop;
      viewHeight = listEl.clientHeight || 0;
    }
  }

  function toggleSort(field: ColumnId) {
    if (field === "num") return;
    const current = headerSort;
    const asc = current && current.field === field ? !current.asc : true;
    sortField = field;
    sortAsc = asc;
    void saveClientState("list.sort", { field: sortField, asc: sortAsc });

    // Make the query string the source of truth: append or replace its
    // trailing SORT BY clause and execute it.
    const mapped = SORT_FIELD_BY_COLUMN[field];
    if (mapped) void libraryQuery.setSort(mapped, asc);
  }

  async function loadSortPrefs() {
    const saved = await loadClientState<{ field?: string; asc?: boolean }>("list.sort");
    if (!saved) return;
    const fields: ColumnId[] = ["num", "name", "tag", "artist", "album", "time"];
    if (fields.includes(saved.field as ColumnId)) {
      sortField = saved.field as ColumnId;
    }
    if (typeof saved.asc === "boolean") sortAsc = saved.asc;
  }

  function sortIndicator(field: ColumnId): string {
    const current = headerSort;
    if (!current || current.field !== field) return "";
    return current.asc ? " ▲" : " ▼";
  }

  function formatDuration(seconds?: number): string {
    if (!seconds || seconds <= 0) return "";
    const m = Math.floor(seconds / 60);
    const s = Math.floor(seconds % 60);
    return m + ":" + s.toString().padStart(2, "0");
  }

  // Display overrides win over the raw file tags; Unknown displays as empty.
  function displayTitle(song: Song): string {
    return song.metadata.display_title ?? song.title;
  }
  function displayArtist(song: Song): string {
    return song.metadata.artist_override ?? song.artist;
  }
  function displayAlbum(song: Song): string {
    return song.metadata.album_override ?? song.album;
  }

  function displayText(song: Song, field: ColumnId): string {
    const v = field === "artist" ? displayArtist(song) : displayAlbum(song);
    return v && v !== "Unknown" ? v : "";
  }

  function platformTag(song: Song): { label: string; kind: "local" | "online" } | null {
    if (song.paths.length > 0 || song.local) return { label: "local", kind: "local" };
    const src = song.sources[0];
    if (src) return { label: src.platform, kind: "online" };
    return null;
  }

  function tagText(song: Song): string {
    return song.tags.map((t) => t.namespace + ":" + t.value).join(", ");
  }

  function tagHue(tag: Tag): number {
    let hash = 0;
    const text = tag.namespace + ":" + tag.value;
    for (let i = 0; i < text.length; i++) {
      hash = (hash * 31 + text.charCodeAt(i)) >>> 0;
    }
    return hash % 360;
  }

  // Column headers can express any sort field in the query language, not just
  // the five visible columns.
  const SORT_FIELD_BY_COLUMN: Record<ColumnId, string | null> = {
    num: null,
    name: "title",
    tag: "tags",
    artist: "artist",
    album: "album",
    time: "duration",
  };
  const COLUMN_BY_SORT_FIELD: Record<string, ColumnId> = {
    title: "name",
    song_metadata: "name",
    tags: "tag",
    artist: "artist",
    album: "album",
    duration: "time",
  };

  /** The active sort: a query SORT BY wins over the column-header fallback. */
  let headerSort = $derived.by((): { field: ColumnId; asc: boolean } | null => {
    const directive = libraryQuery.sort;
    if (directive) {
      const column = COLUMN_BY_SORT_FIELD[directive.field];
      return column ? { field: column, asc: directive.asc } : null;
    }
    return { field: sortField, asc: sortAsc };
  });

  function sortValue(song: Song, field: string): string | number {
    switch (field) {
      case "title":
      case "song_metadata":
        return displayTitle(song);
      case "artist":
        return displayArtist(song);
      case "album":
        return displayAlbum(song);
      case "tags":
        return tagText(song);
      case "duration":
        return song.metadata.duration ?? 0;
      case "added_time":
        return song.metadata.added_time ?? "";
      case "file_name":
        return song.metadata.file_name ?? "";
      case "notes":
        return song.metadata.notes ?? "";
      case "lyrics":
        return song.metadata.lyrics ?? "";
      case "source":
        return song.sources.map((source) => source.platform).join(" ");
      case "path":
        return song.paths.map((path) => path.toString()).join(" ");
      default:
        return "";
    }
  }

  function compareByField(a: Song, b: Song, field: string): number {
    const left = sortValue(a, field);
    const right = sortValue(b, field);
    if (typeof left === "number" && typeof right === "number") {
      return left - right;
    }
    return String(left).localeCompare(String(right));
  }

  function compareByColumn(a: Song, b: Song, field: ColumnId): number {
    switch (field) {
      case "name":
        return displayTitle(a).localeCompare(displayTitle(b));
      case "artist":
        return displayArtist(a).localeCompare(displayArtist(b));
      case "album":
        return displayAlbum(a).localeCompare(displayAlbum(b));
      case "tag":
        return tagText(a).localeCompare(tagText(b));
      case "time":
        return (a.metadata.duration ?? 0) - (b.metadata.duration ?? 0);
      default:
        return 0;
    }
  }

  let sortedSongs = $derived.by(() => {
    const list = [...songs];
    const directive = libraryQuery.sort;
    list.sort((a, b) => {
      const cmp = directive
        ? compareByField(a, b, directive.field)
        : compareByColumn(a, b, sortField);
      const asc = directive ? directive.asc : sortAsc;
      return asc ? cmp : -cmp;
    });
    return list;
  });

  // Virtual slice of the list to render (+ overscan) so long lists stay smooth.
  const OVERSCAN = 8;
  let virtualSlice = $derived.by(() => {
    const n = sortedSongs.length;
    if (n === 0) return { start: 0, end: 0, padTop: 0, padBottom: 0 };
    const h = viewHeight || 600;
    // Clamp stale scroll offsets to the current content: a shorter filtered
    // result must not keep rendering pads based on the old list height.
    const maxTop = Math.max(0, n * ROW_H - h);
    const top = Math.min(Math.max(0, viewTop), maxTop);
    const first = Math.max(0, Math.floor(top / ROW_H) - OVERSCAN);
    const last = Math.min(n, Math.ceil((top + h) / ROW_H) + OVERSCAN);
    return {
      start: first,
      end: last,
      padTop: first * ROW_H,
      padBottom: Math.max(0, (n - last) * ROW_H),
    };
  });

  async function playSong(song: Song) {
    try {
      // Register the current visible list as the queue and start playing the
      // chosen song — the Rust backend owns sequencing & auto-advance.
      player.mediaId = song.id;
      await player.playFromList(
        sortedSongs.map((s) => ({
          id: s.id,
          title: displayTitle(s),
          artist: displayArtist(s),
          album: displayAlbum(s),
        })),
        song.id,
      );
      player.title = displayTitle(song);
      player.artist = displayArtist(song);
      player.album = displayAlbum(song);
      player.cover = song.metadata?.cover_url ?? null;
    } catch (e) {
      console.error("play error:", e);
    }
  }

  function isPlaying(song: Song): boolean {
    return currentMediaId === song.id;
  }

  function openTagEditor(song: Song, e: MouseEvent) {
    e.stopPropagation();
    editingSong = song;
  }

  function closeTagEditor() {
    editingSong = null;
    // Tags are written immediately; refresh the visible rows once the editor
    // loses focus/closes so tag columns and filters reflect the change.
    void loadSongs();
  }

  // ── Details (local file / online source) ──────────
  /** Transient toast shared by the row actions (cache / details). */
  let songNote = $state<string | null>(null);
  function showSongNote(message: string) {
    songNote = message;
    setTimeout(() => (songNote = null), 3000);
  }

  function buildDetailUrl(song: Song): string | null {
    const src = song.sources[0];
    if (!src) return null;
    const id = src.id;
    switch (src.platform) {
      case "youtube":
        return "https://www.youtube.com/watch?v=" + id;
      case "soundcloud":
        return id;
      case "bilibili":
      case "bili":
        return "https://www.bilibili.com/video/" + id;
      case "netease":
      case "wy":
        return "https://music.163.com/#/song?id=" + id;
      case "kugou":
      case "kg":
        return "https://www.kugou.com/song/#hash=" + id;
      case "kuwo":
      case "kw":
        return "http://www.kuwo.cn/play_detail/" + id;
      case "tencent":
      case "qq":
      case "tx":
        return "https://y.qq.com/n/ryqq/songDetail/" + id;
      default:
        return null;
    }
  }

  /**
   * "Details" for a list row:
   * - local file  → open its folder in the file manager and select the file;
   * - online song → open its page in the default browser.
   */
  async function showSongDetails(song: Song) {
    if (song.paths.length > 0) {
      let lastError = "";
      for (const path of song.paths) {
        try {
          // The backend opens the file manager and selects the file, falling
          // back to the containing folder on desktops without FileManager1.
          await invoke("reveal_local_file", { path });
          return;
        } catch (e) {
          // Try the next duplicate location; report only if none worked.
          lastError = String(e);
          console.error("reveal item error:", e);
        }
      }
      showSongNote("无法在文件管理器中定位该文件" + (lastError ? "：" + lastError : ""));
      return;
    }

    const url = buildDetailUrl(song);
    if (!url) {
      showSongNote("该歌曲没有可打开的来源链接");
      return;
    }
    try {
      if (isTauri) await openUrl(url);
      else window.open(url, "_blank", "noopener,noreferrer");
    } catch (e) {
      console.error("open url error:", e);
      showSongNote("无法打开浏览器");
    }
  }

  // Clear (delete) the cached media for a song, forcing a fresh fetch.
  async function clearCache(song: Song) {
    try {
      const n = await invoke<number>("clear_song_cache", { id: song.id });
      showSongNote("已清空缓存" + (n > 0 ? "（" + n + " 条）" : "（无缓存）"));
    } catch (e) {
      console.error("clear cache error:", e);
    }
  }

  function suggestDownloadName(song: Song): string {
    const base = [displayArtist(song), displayTitle(song)].filter(Boolean).join(" - ") || "audio";
    const existing = song.paths[0] ?? song.metadata.file_name ?? "";
    const ext = existing.includes(".") ? existing.split(".").pop()! : "mp3";
    return base.replace(/[\\/:*?"<>|]/g, "_") + "." + (ext === "media" ? "mp3" : ext);
  }

  async function downloadSong(song: Song) {
    try {
      if (!isTauri) {
        const target = remoteDownloadTarget(song.id);
        if (!target) throw new Error("download endpoint unavailable");
        const link = document.createElement("a");
        link.href = target.url;
        link.download = "";
        link.click();
        showSongNote("已开始下载");
        return;
      }
      const suggested = suggestDownloadName(song);
      const destination = await save({ defaultPath: suggested });
      if (!destination) return;
      if (isRemoteMode()) {
        const target = remoteDownloadTarget(song.id);
        if (!target) throw new Error("download endpoint unavailable");
        const path = await invoke<string>("download_remote_to_path", {
          url: target.url,
          headers: target.headers,
          path: destination,
        });
        showSongNote("已保存到 " + path);
      } else {
        const path = await invoke<string>("download_song_to_path", {
          id: song.id,
          path: destination,
        });
        showSongNote("已保存到 " + path);
      }
    } catch (e) {
      console.error("download error:", e);
      showSongNote("下载失败: " + String(e));
    }
  }

  // ── Context menu ──────────────────────────────────
  let ctxSong = $state<Song | null>(null);
  let ctxPos = $state<{ x: number; y: number } | null>(null);
  let selectedIds = $state<Set<string>>(new Set());
  let selectionAnchor = $state<number | null>(null);
  let bulkMetaSongs = $state<Song[] | null>(null);
  let bulkTagSongs = $state<Song[] | null>(null);

  function selectedSongs(): Song[] {
    return sortedSongs.filter((song) => selectedIds.has(song.id));
  }

  function clearSelection() {
    selectedIds = new Set();
    selectionAnchor = null;
  }

  function selectRange(index: number) {
    const anchor = selectionAnchor ?? index;
    const start = Math.min(anchor, index);
    const end = Math.max(anchor, index);
    const next = new Set(selectedIds);
    for (let i = start; i <= end; i += 1) next.add(sortedSongs[i].id);
    selectedIds = next;
    selectionAnchor = index;
  }

  function onRowClick(song: Song, index: number, e: MouseEvent) {
    if (e.ctrlKey || e.metaKey) {
      const next = new Set(selectedIds);
      if (next.has(song.id)) next.delete(song.id);
      else next.add(song.id);
      selectedIds = next;
      selectionAnchor = index;
      return;
    }
    if (e.shiftKey) {
      selectRange(index);
      return;
    }
    if (selectedIds.size > 0) clearSelection();
    clearHover();
    void playSong(song);
  }

  function onRowContext(song: Song, index: number, e: MouseEvent) {
    if (!selectedIds.has(song.id)) {
      selectedIds = new Set([song.id]);
      selectionAnchor = index;
    }
    openContextMenu(song, e);
  }

  async function bulkDownload() {
    const songs = selectedSongs();
    if (songs.length === 0) return;
    if (isTauri) {
      const directory = await open({ directory: true, multiple: false });
      if (!directory || Array.isArray(directory)) return;
      for (const song of songs) {
        const destination = directory + "/" + suggestDownloadName(song);
        if (isRemoteMode()) {
          const target = remoteDownloadTarget(song.id);
          if (!target) continue;
          await invoke("download_remote_to_path", {
            url: target.url,
            headers: target.headers,
            path: destination,
          });
        } else {
          await invoke("download_song_to_path", { id: song.id, path: destination });
        }
      }
      showSongNote("已下载 " + songs.length + " 首");
    } else {
      await downloadManyWeb(songs.map((song) => ({ id: song.id, title: displayTitle(song) })));
    }
    clearSelection();
  }

  async function bulkClearCache() {
    const songs = selectedSongs();
    for (const song of songs) {
      try {
        await invoke("clear_song_cache", { id: song.id });
      } catch {
        /* remote per-song cache clearing is not available */
      }
    }
    showSongNote("已清空所选缓存");
    clearSelection();
  }

  function openBulkMetadata() {
    bulkMetaSongs = selectedSongs();
    closeContextMenu();
  }

  function openBulkTags() {
    bulkTagSongs = selectedSongs();
    closeContextMenu();
  }

  // ── Hover details ─────────────────────────────────
  let hoverDetails = $state<SongDetails | null>(null);
  let hoverPos = $state<{ x: number; y: number } | null>(null);
  let hoverTimer: ReturnType<typeof setTimeout> | undefined;
  let hoverHideTimer: ReturnType<typeof setTimeout> | undefined;
  const detailsCache = new Map<string, SongDetails>();

  function clearHover() {
    if (hoverTimer) {
      clearTimeout(hoverTimer);
      hoverTimer = undefined;
    }
    if (hoverHideTimer) {
      clearTimeout(hoverHideTimer);
      hoverHideTimer = undefined;
    }
    hoverDetails = null;
    hoverPos = null;
  }

  /** Give the pointer time to move from the row into the card itself. */
  function scheduleHideHover() {
    if (hoverHideTimer) clearTimeout(hoverHideTimer);
    hoverHideTimer = setTimeout(() => {
      hoverHideTimer = undefined;
      clearHover();
    }, 500);
  }

  function keepHover() {
    if (hoverHideTimer) {
      clearTimeout(hoverHideTimer);
      hoverHideTimer = undefined;
    }
  }

  function scheduleHover(song: Song, e: MouseEvent) {
    clearHover();
    const x = e.clientX;
    const y = e.clientY;
    hoverTimer = setTimeout(async () => {
      hoverTimer = undefined;
      const cached = detailsCache.get(song.id);
      if (cached) {
        hoverDetails = cached;
        hoverPos = { x, y };
        return;
      }
      try {
        const details = await invoke<SongDetails>("get_song_details", { id: song.id });
        detailsCache.set(song.id, details);
        hoverDetails = details;
        hoverPos = { x, y };
      } catch (error) {
        console.error("song details error:", error);
      }
    }, 600);
  }

  /** Apply a saved metadata override to the list, player and hover cache. */
  function applySavedMetadata(updated: Song) {
    songs = songs.map((song) => (song.id === updated.id ? updated : song));
    detailsCache.delete(updated.id);
    clearHover();
    if (player.mediaId === updated.id) {
      player.title = displayTitle(updated);
      player.artist = displayArtist(updated);
      player.album = displayAlbum(updated);
    }
  }

  function openContextMenu(song: Song, e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    clearHover();
    ctxSong = song;
    ctxPos = { x: e.clientX, y: e.clientY };
  }

  function closeContextMenu() {
    ctxSong = null;
    ctxPos = null;
  }

  let contextMenuItems = $derived.by(() => {
    if (!ctxSong) return [];
    if (selectedIds.size > 1 && selectedIds.has(ctxSong.id)) {
      return [
        { label: "下载 " + selectedIds.size + " 首", icon: Download, action: () => void bulkDownload() },
        { label: "编辑标签", icon: ListPlus, action: openBulkTags },
        { label: "编辑信息", icon: Pencil, action: openBulkMetadata },
        { label: "清空缓存", icon: Trash2, action: () => void bulkClearCache() },
        { label: "", action: () => {}, icon: undefined, divider: true },
        { label: "取消多选", icon: Info, action: clearSelection },
      ] as MenuItem[];
    }
    const song = ctxSong;
    const isLocal = song.paths.length > 0;
    const remote = isRemoteMode() || !isTauri;
    const canEdit = canManage();
    return [
      { label: "播放", icon: Play, action: () => playSong(song) },
      ...(canEdit
        ? [
            { label: "编辑标签", icon: ListPlus, action: () => (editingSong = song) } as MenuItem,
            { label: "编辑信息", icon: Pencil, action: () => (editingMetaSong = song) } as MenuItem,
            { label: "", action: () => {}, icon: undefined, divider: true } as MenuItem,
          ]
        : []),
      // One "details" entry for both kinds of song: reveal the file for a
      // local track, open the source page for an online one.
      { label: "详情", icon: Info, action: () => showSongDetails(song) },
      // Download works for local and online songs; cache cleanup only for online.
      ...(canManage()
        ? [{ label: "下载", icon: Download, action: () => void downloadSong(song) } as MenuItem]
        : []),
      ...(remote || isLocal
        ? []
        : [
            {
              label: "清空该曲缓存",
              icon: Trash2,
              action: () => clearCache(song),
            } as MenuItem,
          ]),
    ] as MenuItem[];
  });

  function labelOf(id: ColumnId): string {
    return COLUMN_DEFS.find((d) => d.id === id)!.label;
  }

  // ── TanStack table: owns column sizing / order / visibility ──
  const LIST_FEATURES = tableFeatures({
    columnSizingFeature,
    columnResizingFeature,
    columnOrderingFeature,
    columnVisibilityFeature,
  });

  const table = createTable({
    features: LIST_FEATURES,
    columns: COLUMN_DEFS.map((d) => ({
      id: d.id,
      header: d.label,
      size: d.defWidth,
      minSize: d.min,
      maxSize: d.fixed ?? 2000,
      enableResizing: d.fixed == null,
    })),
    get data() {
      return sortedSongs;
    },
    // Commit while dragging so the row layout follows the cursor.
    columnResizeMode: "onChange",
    state: {
      get columnSizing() {
        return prefs.sizing;
      },
      get columnOrder() {
        return prefs.order;
      },
      get columnVisibility() {
        return prefs.visibility;
      },
    },
    onColumnSizingChange: (updater) => {
      prefs = { ...prefs, sizing: applySizing(updater) };
      savePrefs();
    },
    onColumnOrderChange: (updater) => {
      prefs = { ...prefs, order: resolveUpdater(updater, prefs.order) };
      savePrefs();
    },
    onColumnVisibilityChange: (updater) => {
      prefs = { ...prefs, visibility: resolveUpdater(updater, prefs.visibility) };
      savePrefs();
    },
  });

  /** Flat header list in display order (this table has no column groups). */
  let headers = $derived(table.getHeaderGroups()[0]?.headers ?? []);

  // Keep the columns filling the list whenever its width changes (window
  // resize, sidebar toggles, ...).
  $effect(() => {
    if (!headerEl) return;
    const ro = new ResizeObserver(() => {
      if (headerEl) fitToWidth(headerEl.clientWidth - scrollbarWidth);
    });
    ro.observe(headerEl);
    return () => ro.disconnect();
  });

  function colStyle(header: { getSize(): number }): string {
    const size = header.getSize();
    return "flex: 0 0 " + size + "px; width: " + size + "px; min-width: 0;";
  }
</script>

<div class="library">
  <div class="library-main">
    <!-- Table header -->
    <div class="table-header" bind:this={headerEl} style="padding-right: {scrollbarWidth}px">
      {#each headers as header (header.id)}
        <!-- Only the label starts a reorder drag, so the resize handle never
             competes with the native drag gesture. -->
        <div
          class="th"
          class:dragging={dragColId === header.column.id}
          style={colStyle(header)}
          ondragover={(e) => handleHeaderDragOver(e, header.column.id)}
          ondragleave={handleHeaderDragLeave}
          ondrop={(e) => handleHeaderDrop(e, header.column.id)}
          oncontextmenu={openColMenu}
        >
          <button
            class="th-label"
            draggable="true"
            onclick={() => toggleSort(header.column.id as ColumnId)}
            ondragstart={(e) => handleHeaderDragStart(e, header.column.id)}
            ondragend={handleHeaderDragEnd}
            title="点击排序，拖拽调序"
          >
            {labelOf(header.column.id as ColumnId)}{sortIndicator(header.column.id as ColumnId)}
          </button>
          {#if header.column.getCanResize()}
            <div
              class="resize-handle"
              class:resizing={header.column.getIsResizing()}
              title="拖动调宽，双击复位"
              onmousedown={(e) => {
                e.stopPropagation();
                e.preventDefault();
                header.getResizeHandler()(e);
              }}
              ontouchstart={(e) => {
                e.stopPropagation();
                header.getResizeHandler()(e);
              }}
              ondblclick={(e) => resetWidth(e, header.column.id)}
            ></div>
          {/if}
        </div>
      {/each}
      <div class="col-filler"></div>
    </div>

    <!-- List / states -->
    {#if loading}
      <div class="empty-state"><p>加载中...</p></div>
    {:else if error}
      <div class="empty-state">
        <p class="empty-title">加载失败</p>
        <p class="empty-desc">{error}</p>
      </div>
    {:else if sortedSongs.length === 0}
      <div class="empty-state">
        <Search size={32} class="empty-icon" />
        <p class="empty-title">没有歌曲</p>
        <p class="empty-desc">添加库目录或导入播放列表开始使用</p>
      </div>
    {:else}
      <div class="song-list" bind:this={listEl} onscroll={onListScroll}>
        <div class="virtual-pad" style="height: {virtualSlice.padTop}px"></div>
        {#each sortedSongs.slice(virtualSlice.start, virtualSlice.end) as song, k}
          {@const i = virtualSlice.start + k}
          <div
            class="song-row"
            class:is-playing={isPlaying(song)}
            class:located={locatorTarget === song.id}
            data-song-id={song.id}
            onclick={(e) => onRowClick(song, i, e)}
            onmouseenter={(e) => scheduleHover(song, e)}
            onmouseleave={scheduleHideHover}
            oncontextmenu={(e) => onRowContext(song, i, e)}
          >
            {#each headers as header (header.id)}
              <div class="td" style={colStyle(header)}>
                {#if header.column.id === "num"}
                  {#if selectedIds.size > 0}
                    <input
                      type="checkbox"
                      class="row-check"
                      checked={selectedIds.has(song.id)}
                      onclick={(e) => e.stopPropagation()}
                      onchange={(e) => {
                        const next = new Set(selectedIds);
                        if (e.currentTarget.checked) next.add(song.id);
                        else next.delete(song.id);
                        selectedIds = next;
                        selectionAnchor = i;
                      }}
                    />
                  {:else if isPlaying(song)}
                    <span class="playing-indicator">&#9654;</span>
                  {:else}
                    <span class="cell-num">{i + 1}</span>
                  {/if}
                {:else if header.column.id === "name"}
                  <div class="name-cell">
                    <span class="name-text">{displayTitle(song)}</span>
                    {#if platformTag(song)}
                      <span class="tag-badge" class:local={platformTag(song)!.kind == "local"}>
                        {platformTag(song)!.label}
                      </span>
                    {/if}
                  </div>
                {:else if header.column.id === "tag"}
                  <div class="tag-cell">
                    {#each song.tags as tag}
                      <span
                        class="tag-pill"
                        style="--tag-hue: {tagHue(tag)}"
                        title={tag.namespace + ":" + tag.value}
                      >{tag.namespace}:{tag.value}</span>
                    {/each}
                  </div>
                {:else if header.column.id === "artist"}
                  <span class="cell-text">{displayText(song, "artist")}</span>
                {:else if header.column.id === "album"}
                  <span class="cell-text">{displayText(song, "album")}</span>
                {:else if header.column.id === "time"}
                  <span class="cell-time">{formatDuration(song.metadata.duration)}</span>
                {/if}
              </div>
            {/each}
            <div class="col-filler"></div>
          </div>
        {/each}
        <div class="virtual-pad" style="height: {virtualSlice.padBottom}px"></div>
      </div>
    {/if}
  </div>
</div>

{#if songNote}
  <div class="toast-note">{songNote}</div>
{/if}


{#if editingSong}
  <TagEditor song={editingSong} onclose={closeTagEditor} />
{/if}

{#if editingMetaSong}
  <MetadataEditor
    song={editingMetaSong}
    onclose={() => (editingMetaSong = null)}
    onsaved={applySavedMetadata}
  />
{/if}

{#if bulkMetaSongs}
  <MultiMetadataEditor
    songs={bulkMetaSongs}
    onclose={() => {
      bulkMetaSongs = null;
      clearSelection();
    }}
    onsaved={(updated) => {
      const byId = new Map(updated.map((song) => [song.id, song]));
      songs = songs.map((song) => byId.get(song.id) ?? song);
      bulkMetaSongs = null;
      clearSelection();
    }}
  />
{/if}

{#if bulkTagSongs}
  <MultiTagEditor
    songs={bulkTagSongs}
    onclose={() => {
      bulkTagSongs = null;
      clearSelection();
    }}
    onsaved={(updated) => {
      const byId = new Map(updated.map((song) => [song.id, song]));
      songs = songs.map((song) => byId.get(song.id) ?? song);
      bulkTagSongs = null;
      clearSelection();
    }}
  />
{/if}

{#if hoverDetails && hoverPos}
  <SongHoverCard
    details={hoverDetails}
    x={hoverPos.x}
    y={hoverPos.y}
    onenter={keepHover}
    onleave={scheduleHideHover}
  />
{/if}

{#if ctxPos && contextMenuItems.length > 0}
  <ContextMenu items={contextMenuItems} x={ctxPos.x} y={ctxPos.y} onclose={closeContextMenu} />
{/if}

{#if colMenu}
  <ContextMenu items={colMenuItems} x={colMenu.x} y={colMenu.y} onclose={() => (colMenu = null)} />
{/if}

<style>
  .library {
    display: flex;
    height: 100%;
  }

  .library-main {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  /* Header */
  .table-header {
    display: flex;
    align-items: stretch;
    height: 32px;
    min-width: 0;
    overflow: hidden;
    background: var(--color-base-200);
    border-bottom: 1px solid var(--color-base-300);
    font-size: 11px;
    font-weight: 600;
    color: var(--color-base-content) / 0.7;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    user-select: none;
  }

  .th {
    position: relative;
    display: flex;
    align-items: center;
    padding: 0 8px;
    min-width: 0;
    border-right: 1px solid var(--panel-border);
  }

  .th.dragging { opacity: 0.5; }

  .th.drop-target::after {
    content: ""; position: absolute; right: -1px; top: 4px; bottom: 4px;
    width: 2px; background: var(--color-primary); border-radius: 1px;
  }

  .th-label {
    flex: 1;
    text-align: left;
    background: none;
    border: none;
    color: inherit;
    font: inherit;
    font-weight: inherit;
    letter-spacing: inherit;
    text-transform: inherit;
    cursor: pointer;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    padding: 0;
  }

  .th-label:hover { color: var(--color-base-content); }

  .resize-handle {
    position: absolute;
    right: 0;
    top: 0;
    bottom: 0;
    width: 7px;
    cursor: col-resize;
    z-index: 3;
    touch-action: none;
  }
  .resize-handle::after {
    content: ""; position: absolute; left: 50%; top: 6px; bottom: 6px;
    width: 1px; background: transparent; transition: background 0.12s;
  }
  .resize-handle:hover::after,
  .resize-handle.resizing::after { background: var(--color-primary) / 0.6; }

  /* Absorbs the slack between the last column and the right edge so rows and
     the header always span the full width. */
  .col-filler { flex: 1 1 0; min-width: 0; }

  /* Body */
  .song-list {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    min-width: 0;
    /* Scroll anchoring fights a virtual list when rows/padding change above
       the viewport; it can cause a self-sustaining scroll loop. */
    overflow-anchor: none;
    overscroll-behavior: contain;
  }
  .virtual-pad { flex-shrink: 0; }
  .song-row {
    display: flex;
    align-items: center;
    height: 36px;
    min-width: 0;
    color: var(--color-base-content);
    font-size: 13px;
    cursor: pointer;
    transition: background 0.1s;
  }
  .song-row:hover { background: var(--color-base-300) / 0.7; }
  .song-row.is-playing { background: var(--color-primary) / 0.12; }
  .song-row.is-playing .name-text { color: var(--color-primary); }
  .song-row.located { background: var(--color-primary) / 0.2; }
  .song-row.located .name-text { color: var(--color-primary); font-weight: 600; }

  .td {
    display: flex;
    align-items: center;
    padding: 0 8px;
    min-width: 0;
    user-select: text; /* allow copying */
  }

  .cell-num { color: var(--color-base-content) / 0.5; font-size: 12px; }
  .playing-indicator { color: var(--color-primary); font-size: 10px; }
  .row-check { width: 13px; height: 13px; accent-color: var(--color-primary); cursor: pointer; }

  .name-cell { display: flex; align-items: center; gap: 6px; min-width: 0; width: 100%; }
  .name-text { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tag-badge {
    flex: none; font-size: 10px; padding: 1px 5px; border-radius: 4px;
    background: var(--color-primary) / 0.15; color: var(--color-primary);
  }
  .tag-badge.local { background: var(--color-success) / 0.15; color: var(--color-success); }

  .tag-cell {
    width: 100%;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 3px;
    overflow: hidden;
    white-space: nowrap;
  }
  .tag-pill {
    flex: none;
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 10px;
    line-height: 1.5;
    padding: 1px 5px;
    border-radius: 999px;
    color: hsl(var(--tag-hue) 70% 78%);
    background: hsl(var(--tag-hue) 55% 45% / 0.16);
    border: 1px solid hsl(var(--tag-hue) 60% 55% / 0.28);
  }

  .cell-text { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cell-time { color: var(--color-base-content) / 0.6; font-size: 12px; }

  .empty-state {
    flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center;
    gap: 8px; padding: 40px; color: var(--color-base-content) / 0.5;
  }
  .empty-icon { opacity: 0.4; }
  .empty-title { font-size: 15px; font-weight: 500; color: var(--color-base-content) / 0.7; }
  .empty-desc { font-size: 12px; text-align: center; max-width: 240px; line-height: 1.5; }

  .download-backdrop {
    position: fixed; inset: 0; z-index: 700;
    background: rgba(0, 0, 0, 0.45);
    display: flex; align-items: center; justify-content: center;
  }
  .download-dialog {
    width: min(520px, calc(100vw - 40px));
    background: var(--color-base-200);
    border: 1px solid var(--panel-border-strong);
    border-radius: 10px;
    padding: 16px;
    display: flex; flex-direction: column; gap: 8px;
    box-shadow: var(--shadow-lg);
  }
  .download-dialog h3 { margin: 0; font-size: 14px; }
  .download-title { margin: 0; font-size: 12px; color: var(--color-base-content) / 0.7; }
  .download-label { font-size: 11px; color: var(--color-base-content) / 0.6; }
  .download-input {
    width: 100%; box-sizing: border-box;
    padding: 6px 9px;
    border: 1px solid var(--color-base-300);
    border-radius: 5px;
    background: var(--color-base-100);
    color: var(--color-base-content);
    font: 11px var(--font-mono, monospace);
  }
  .download-input:focus { outline: none; border-color: var(--color-primary); }
  .download-success {
    margin: 2px 0; font-size: 11px; color: var(--color-success);
    overflow-wrap: anywhere; user-select: text;
  }
  .download-actions { display: flex; justify-content: flex-end; gap: 6px; margin-top: 4px; }
  .download-actions button {
    padding: 5px 10px; border-radius: 5px; border: 1px solid var(--panel-border);
    background: var(--color-base-100); color: var(--color-base-content) / 0.8;
    font-size: 12px;
  }
  .download-actions button:hover:not(:disabled) { border-color: var(--color-primary); color: var(--color-primary); }
  .download-actions button:disabled { opacity: 0.5; }

  .toast-note {
    position: fixed;
    bottom: 72px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 400;
    background: var(--color-base-300);
    color: var(--color-primary);
    border: 1px solid var(--panel-border-strong);
    border-radius: 8px; padding: 8px 16px;
    font-size: 12px;
    box-shadow: var(--shadow-md);
  }
</style>
