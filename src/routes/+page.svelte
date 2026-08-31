<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount, tick } from "svelte";
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
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { Search, Play, Download, ListPlus, ExternalLink, Trash2, Tag as TagIcon } from "lucide-svelte";
  import { player } from "$lib/stores/player.svelte";
  import { searchPresets } from "$lib/stores/searchPresets.svelte";
  import PresetPanel from "$lib/components/library/PresetPanel.svelte";
  import TagEditor from "$lib/components/tag/TagEditor.svelte";
  import ContextMenu from "$lib/components/song/ContextMenu.svelte";
  import type { Song } from "$lib/types";
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

  function loadPrefs(): ColumnPrefs {
    const dflt = defaultPrefs();
    try {
      const saved = localStorage.getItem(COL_KEY);
      if (!saved) return dflt;
      const data: unknown = JSON.parse(saved);
      // v1 stored an array of {id, shown, width}; drop it and start fresh.
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
    } catch {
      return dflt;
    }
  }

  let prefs = $state<ColumnPrefs>(loadPrefs());

  function savePrefs() {
    try {
      localStorage.setItem(COL_KEY, JSON.stringify(prefs));
    } catch {
      /* ignore */
    }
  }

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
  let activePresetId = $state<string | null>(null);
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

  // Remember the scroll position between visits (localStorage, UI-only).
  const SCROLL_KEY = "vide_list_scroll";
  function saveScroll() {
    if (!listEl) return;
    try {
      localStorage.setItem(SCROLL_KEY, String(listEl.scrollTop));
    } catch { /* ignore */ }
  }
  function restoreScroll() {
    if (!listEl) return;
    try {
      const s = localStorage.getItem(SCROLL_KEY);
      if (s != null) listEl.scrollTop = parseInt(s, 10) || 0;
    } catch { /* ignore */ }
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

  onMount(async () => {
    await searchPresets.load();
    // Handle an initial locate request (navigating here with ?locate=).
    const locateId = $page.url.searchParams.get("locate");
    const presetParam = $page.url.searchParams.get("preset");
    if (presetParam === "all") {
      activePresetId = null;
      await searchPresets.select(null);
    } else {
      activePresetId = searchPresets.activeId;
    }
    await loadSongs();
    if (locateId) {
      await scrollToSong(locateId);
    } else {
      await tick();
      restoreScroll();
    }
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
        // Force "全部歌曲" so the song is present in this view.
        activePresetId = null;
        void searchPresets.select(null);
        void loadSongs().then(() => scrollToSong(locateId));
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
      const preset = searchPresets.active;
      if (preset) {
        songs = await invoke<Song[]>("search_songs", { query: preset.query });
      } else {
        songs = await invoke<Song[]>("list_library_songs");
      }
    } catch (e) {
      error = String(e);
      songs = [];
    } finally {
      loading = false;
    }
  }

  async function selectPreset(id: string | null) {
    activePresetId = id;
    await searchPresets.select(id);
    await loadSongs();
  }

  function toggleSort(field: ColumnId) {
    if (sortField === field) sortAsc = !sortAsc;
    else {
      sortField = field;
      sortAsc = true;
    }
  }

  function sortIndicator(field: ColumnId): string {
    if (sortField !== field) return "";
    return sortAsc ? " ▲" : " ▼";
  }

  function formatDuration(seconds?: number): string {
    if (!seconds || seconds <= 0) return "";
    const m = Math.floor(seconds / 60);
    const s = Math.floor(seconds % 60);
    return m + ":" + s.toString().padStart(2, "0");
  }

  // Unknown artist / album display as empty (per requirement).
  function displayText(song: Song, field: ColumnId): string {
    const v = field === "artist" ? song.artist : song.album;
    return v && v !== "Unknown" ? v : "";
  }

  function platformTag(song: Song): { label: string; kind: "local" | "online" } | null {
    if (song.paths.length > 0) return { label: "local", kind: "local" };
    const src = song.sources[0];
    if (src) return { label: src.platform, kind: "online" };
    return null;
  }

  function tagText(song: Song): string {
    return song.tags.map((t) => t.namespace + ":" + t.value).join(", ");
  }

  let sortedSongs = $derived.by(() => {
    const list = [...songs];
    list.sort((a, b) => {
      let cmp = 0;
      switch (sortField) {
        case "name":
          cmp = a.title.localeCompare(b.title);
          break;
        case "artist":
          cmp = a.artist.localeCompare(b.artist);
          break;
        case "album":
          cmp = a.album.localeCompare(b.album);
          break;
        case "tag":
          cmp = tagText(a).localeCompare(tagText(b));
          break;
        case "time":
          cmp = (a.metadata.duration ?? 0) - (b.metadata.duration ?? 0);
          break;
      }
      return sortAsc ? cmp : -cmp;
    });
    return list;
  });

  // Virtual slice of the list to render (+ overscan) so long lists stay smooth.
  const OVERSCAN = 8;
  let virtualSlice = $derived.by(() => {
    const n = sortedSongs.length;
    if (n === 0) return { start: 0, end: 0, padTop: 0, padBottom: 0 };
    const h = viewHeight || 600;
    const first = Math.max(0, Math.floor(viewTop / ROW_H) - OVERSCAN);
    const last = Math.min(n, Math.ceil((viewTop + h) / ROW_H) + OVERSCAN);
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
          title: s.metadata?.display_title ?? s.title,
          artist: s.metadata?.artist_override ?? s.artist,
          album: s.metadata?.album ?? s.album ?? "",
        })),
        song.id,
      );
      player.title = song.metadata?.display_title ?? song.title;
      player.artist = song.metadata?.artist_override ?? song.artist;
      player.album = song.metadata?.album ?? song.album ?? "";
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
  }

  // ── "Jump to detail" external URL ─────────────────
  function buildDetailUrl(song: Song): string | null {
    const src = song.sources[0];
    if (!src) return null;
    const id = src.id;
    switch (src.platform) {
      case "youtube":
        return "https://www.youtube.com/watch?v=" + id;
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
        return "https://y.qq.com/n/yqq/song/" + id + ".html";
      case "migu":
      case "mg":
        return "http://music.migu.cn/v3/music/song/" + id;
      default:
        return null;
    }
  }

  async function jumpToDetail(song: Song) {
    const url = buildDetailUrl(song);
    if (url) {
      try {
        await openUrl(url);
      } catch (e) {
        console.error("open url error:", e);
      }
    }
  }

  // Clear (delete) the cached media for a song, forcing a fresh fetch.
  let clearCacheNote = $state<string | null>(null);
  async function clearCache(song: Song) {
    try {
      const n = await invoke<number>("clear_song_cache", { id: song.id });
      clearCacheNote = "已清空缓存" + (n > 0 ? "（" + n + " 条）" : "（无缓存）");
      setTimeout(() => (clearCacheNote = null), 3000);
    } catch (e) {
      console.error("clear cache error:", e);
    }
  }

  // Download an online song into the local cache/library.
  let downloadNote = $state<string | null>(null);
  async function downloadSong(song: Song) {
    try {
      const n = await invoke<number>("download_song", { id: song.id });
      if (n > 0) {
        downloadNote = "已下载（" + n + " 个路径）";
        await loadSongs();
      } else {
        downloadNote = "已是本地文件";
      }
      setTimeout(() => (downloadNote = null), 3000);
    } catch (e) {
      console.error("download error:", e);
      downloadNote = "下载失败: " + String(e);
      setTimeout(() => (downloadNote = null), 4000);
    }
  }

  // ── Context menu ──────────────────────────────────
  let ctxSong = $state<Song | null>(null);
  let ctxPos = $state<{ x: number; y: number } | null>(null);

  function openContextMenu(song: Song, e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    ctxSong = song;
    ctxPos = { x: e.clientX, y: e.clientY };
  }

  function closeContextMenu() {
    ctxSong = null;
    ctxPos = null;
  }

  let contextMenuItems = $derived.by(() => {
    if (!ctxSong) return [];
    const detailUrl = buildDetailUrl(ctxSong);
    return [
      { label: "播放", icon: Play, action: () => playSong(ctxSong!) },
      { label: "编辑标签", icon: ListPlus, action: () => (editingSong = ctxSong) },
      { label: "", action: () => {}, icon: undefined, divider: true },
      ...(detailUrl
        ? [{ label: "跳转详情", icon: ExternalLink, action: () => jumpToDetail(ctxSong!) } as MenuItem]
        : []),
      { label: "清空该曲缓存", icon: Trash2, action: () => clearCache(ctxSong!) },
      { label: "下载", icon: Download, action: () => downloadSong(ctxSong!) },
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
  <PresetPanel
    activeId={activePresetId}
    onSelect={selectPreset}
    onPlay={playSong}
    songs={songs}
  />

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
            onclick={() => playSong(song)}
            oncontextmenu={(e) => openContextMenu(song, e)}
          >
            {#each headers as header (header.id)}
              <div class="td" style={colStyle(header)}>
                {#if header.column.id === "num"}
                  {#if isPlaying(song)}
                    <span class="playing-indicator">&#9654;</span>
                  {:else}
                    <span class="cell-num">{i + 1}</span>
                  {/if}
                {:else if header.column.id === "name"}
                  <div class="name-cell">
                    <span class="name-text">{song.title}</span>
                    {#if platformTag(song)}
                      <span class="tag-badge" class:local={platformTag(song)!.kind == "local"}>
                        {platformTag(song)!.label}
                      </span>
                    {/if}
                  </div>
                {:else if header.column.id === "tag"}
                  <div class="tag-cell">
                    {#if song.tags.length > 0}
                      <span class="tag-cell-text">{tagText(song)}</span>
                    {/if}
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

{#if clearCacheNote}
  <div class="toast-note">{clearCacheNote}</div>
{/if}

{#if editingSong}
  <TagEditor song={editingSong} onclose={closeTagEditor} />
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
  .song-list { flex: 1; overflow-y: auto; overflow-x: hidden; min-width: 0; }
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

  .name-cell { display: flex; align-items: center; gap: 6px; min-width: 0; width: 100%; }
  .name-text { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tag-badge {
    flex: none; font-size: 10px; padding: 1px 5px; border-radius: 4px;
    background: var(--color-primary) / 0.15; color: var(--color-primary);
  }
  .tag-badge.local { background: var(--color-success) / 0.15; color: var(--color-success); }

  .tag-cell { width: 100%; min-width: 0; }
  .tag-cell-text { font-size: 11px; color: var(--color-base-content) / 0.65; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .cell-text { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cell-time { color: var(--color-base-content) / 0.6; font-size: 12px; }

  .empty-state {
    flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center;
    gap: 8px; padding: 40px; color: var(--color-base-content) / 0.5;
  }
  .empty-icon { opacity: 0.4; }
  .empty-title { font-size: 15px; font-weight: 500; color: var(--color-base-content) / 0.7; }
  .empty-desc { font-size: 12px; text-align: center; max-width: 240px; line-height: 1.5; }

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
