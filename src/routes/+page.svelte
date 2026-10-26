<script lang="ts">
  import { invoke } from "$lib/transport";
  import { onDestroy, onMount, tick } from "svelte";
  import { page } from "$app/stores";
  import { listen } from "@tauri-apps/api/event";
  import { Search, Play, Download, ListPlus, Info, Pencil, Trash2 } from "lucide-svelte";
  import { player } from "$lib/stores/player.svelte";
  import { isRemoteMode } from "$lib/transport";
  import { isTauri } from "$lib/isTauri";
  import {
    clearSelectedCache,
    clearSongCache,
    downloadSelected,
    downloadSong as downloadSongAction,
    revealOrOpenDetails,
  } from "$lib/library/actions";
  import { canDownload, canManage } from "$lib/stores/auth.svelte";
  import { libraryQuery } from "$lib/stores/libraryQuery.svelte";
  import { loadClientState, saveClientState } from "$lib/clientState";
  import { ColumnLayout } from "$lib/table/columnLayout.svelte";
  import type { TableColumnDef } from "$lib/table/columns";
  import { platform } from "$lib/platform.svelte";
  import MobileSongList from "$lib/components/library/MobileSongList.svelte";
  import MenuSheet from "$lib/ui/MenuSheet.svelte";
  import { libraryStore } from "$lib/library/store.svelte";
  import { SelectionStore } from "$lib/library/selection.svelte";
  import { virtualSlice as computeVirtualSlice } from "$lib/virtual";
  import type { LibraryColumn } from "$lib/library/sorting";
  import TagEditor from "$lib/components/tag/TagEditor.svelte";
  import ContextMenu from "$lib/components/song/ContextMenu.svelte";
  import SongHoverCard from "$lib/components/song/SongHoverCard.svelte";
  import MetadataEditor from "$lib/components/song/MetadataEditor.svelte";
  import MultiMetadataEditor from "$lib/components/song/MultiMetadataEditor.svelte";
  import MultiTagEditor from "$lib/components/song/MultiTagEditor.svelte";
  import type { Song, SongDetails } from "$lib/types";
  import type { MenuItem } from "$lib/ui/menu";
  import {
    displayAlbum,
    displayArtist,
    displayText,
    displayTitle,
    formatDuration,
    platformTag,
    tagHue,
    tagText,
  } from "$lib/songDisplay";

  type ColumnId = LibraryColumn;

  // Fixed columns (# / duration) keep an exact width and cannot be resized.
  const COLUMN_DEFS: (TableColumnDef & { id: ColumnId })[] = [
    { id: "num", label: "#", fixed: true, defWidth: 40, min: 40 },
    { id: "name", label: "名称", defWidth: 300, min: 120 },
    { id: "tag", label: "标签", defWidth: 160, min: 80 },
    { id: "artist", label: "歌手", defWidth: 130, min: 80 },
    { id: "album", label: "专辑", defWidth: 140, min: 80 },
    { id: "time", label: "时长", fixed: true, defWidth: 52, min: 52 },
  ];

  // Column layout (width / order / visibility / fit) is shared with the search
  // table through ColumnLayout. The localStorage key is only read once, to
  // migrate layouts saved by older builds.
  const COL_KEY = "vide_list_columns";
  const layout = new ColumnLayout(COLUMN_DEFS, "list.columns");

  let scrollSaveTimer: ReturnType<typeof setTimeout> | undefined;

  async function loadColumns() {
    await layout.load(COL_KEY);
    // The table is created before the async load; fit the restored widths to
    // the current container once the header has rendered.
    await tick();
    if (headerEl) layout.fitToWidth(headerEl.clientWidth - scrollbarWidth);
  }

  onDestroy(() => {
    layout.flush();
    if (scrollSaveTimer) {
      clearTimeout(scrollSaveTimer);
      scrollSaveTimer = undefined;
    }
    if (listEl) void saveClientState("list.scroll", { top: listEl.scrollTop });
  });

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
    layout.move(sourceId, targetId);
  }

  function handleHeaderDragEnd() {
    dragColId = null;
  }

  // Double-click the resize handle to restore the column's default width.
  function resetWidth(e: MouseEvent, id: string) {
    e.preventDefault();
    e.stopPropagation();
    layout.resetWidth(id);
  }

  // ── Header context menu (show/hide columns) ───────
  let colMenu = $state<{ x: number; y: number } | null>(null);
  let colMenuItems = $state<MenuItem[]>([]);

  function openColMenu(e: MouseEvent) {
    e.preventDefault();
    const canHide = layout.visibleIds().length > 1;
    colMenuItems = COLUMN_DEFS.map((def) => {
      const shown = layout.prefs.visibility[def.id] !== false;
      return {
        label: (shown ? "✓ " : "   ") + def.label,
        disabled: shown && !canHide,
        action: () => layout.toggleVisibility(def.id),
      };
    });
    colMenu = { x: e.clientX, y: e.clientY };
  }
  // ── Song data (owned by the store so route switches keep the list) ──
  let songs = $derived(libraryStore.songs);
  let loading = $derived(libraryStore.loading);
  let error = $derived(libraryStore.error);
  let sortedSongs = $derived(libraryStore.sorted);
  let headerSort = $derived(libraryStore.headerSort);
  let editingSong = $state<Song | null>(null);
  let editingMetaSong = $state<Song | null>(null);
  let listEl = $state<HTMLDivElement | null>(null);
  let mobileList = $state<{
    scrollToOffset: (top: number) => void;
    scrollToSong: (id: string) => void;
  } | null>(null);
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
  function saveScrollTop(top: number) {
    if (scrollSaveTimer) clearTimeout(scrollSaveTimer);
    scrollSaveTimer = setTimeout(() => {
      scrollSaveTimer = undefined;
      void saveClientState("list.scroll", { top });
    }, 300);
  }

  function saveScroll() {
    if (listEl) saveScrollTop(listEl.scrollTop);
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
    if (typeof top !== "number") return;
    if (platform.mobileUI) {
      mobileList?.scrollToOffset(top);
      return;
    }
    if (listEl) {
      const max = Math.max(0, listEl.scrollHeight - listEl.clientHeight);
      listEl.scrollTop = Math.min(Math.max(0, top), max);
      // Use the browser-clamped value: setting a stale saved offset can make
      // the virtual window render rows at the wrong place.
      viewTop = listEl.scrollTop;
      viewHeight = listEl.clientHeight || 0;
    }
  }

  // Locate the given song id in the list: highlight + scroll it into view.
  const ROW_H = 34; // song-row height in px
  let locatorTarget = $state<string | null>(null);
  async function scrollToSong(id: string) {
    locatorTarget = id;
    // Find the index in the current sorted list and center the row.
    for (let attempt = 0; attempt < 40; attempt++) {
      await tick();
      const idx = sortedSongs.findIndex((s) => s.id === id);
      if (idx >= 0) {
        if (platform.mobileUI) {
          mobileList?.scrollToSong(id);
        } else if (listEl) {
          const target = idx * ROW_H - (listEl.clientHeight / 2) + ROW_H / 2;
          listEl.scrollTop = Math.max(0, target);
          // The virtual list relies on viewTop; update it explicitly because a
          // programmatic scrollTop assignment does not always fire onscroll.
          viewTop = listEl.scrollTop;
          viewHeight = listEl.clientHeight || 0;
          saveScroll();
        }
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
      void libraryStore.reload();
    }).then((fn) => (unlisten = fn));
    return () => unlisten?.();
  });

  onMount(async () => {
    await loadColumns();
    await libraryStore.init();

    // A locate request must be able to see the whole library: clear the
    // restored query first, otherwise the song may be filtered out.
    const locateId = $page.url.searchParams.get("locate");
    if (locateId) {
      await libraryQuery.execute("");
      await libraryStore.reload();
      await scrollToSong(locateId);
    } else {
      await tick();
      await restoreScroll();
    }
  });

  // The query bar writes libraryQuery.query; the store only reloads when the
  // applied query actually differs from the loaded one.
  $effect(() => {
    void libraryStore.syncQuery(libraryQuery.query);
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
        void libraryQuery.execute("").then(async () => {
          await libraryStore.reload();
          await scrollToSong(locateId);
        });
      }
    } else if (req === 0) {
      _lastLocateReq = -1;
    }
  });

  async function reloadSongs(): Promise<void> {
    await libraryStore.reload();
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
    void libraryStore.toggleSort(field);
  }

  function sortIndicator(field: ColumnId): string {
    const current = headerSort;
    if (!current || current.field !== field) return "";
    return current.asc ? " ▲" : " ▼";
  }

  // Touch sort picker: the sheet lists every sortable column and marks the
  // active one with its direction.
  const SORT_FIELDS: { id: ColumnId; label: string }[] = [
    { id: "name", label: "标题" },
    { id: "tag", label: "标签" },
    { id: "artist", label: "歌手" },
    { id: "album", label: "专辑" },
    { id: "time", label: "时长" },
  ];

  let sortMenuItems = $derived(
    SORT_FIELDS.map((field) => ({
      label: field.label + sortIndicator(field.id),
      action: () => {
        sortSheetOpen = false;
        toggleSort(field.id);
      },
    })),
  );

  // Virtual slice of the list to render (+ overscan) so long lists stay smooth.
  const OVERSCAN = 8;
  let virtualSlice = $derived(
    computeVirtualSlice(sortedSongs.length, ROW_H, viewTop, viewHeight, OVERSCAN),
  );

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
    return player.mediaId === song.id;
  }

  function openTagEditor(song: Song, e: MouseEvent) {
    e.stopPropagation();
    editingSong = song;
  }

  function closeTagEditor() {
    editingSong = null;
    // Tags are written immediately; refresh the visible rows once the editor
    // loses focus/closes so tag columns and filters reflect the change.
    void libraryStore.reload();
  }

  // ── Details (local file / online source) ──────────
  /** Transient toast shared by the row actions (cache / details). */
  let songNote = $state<string | null>(null);
  function showSongNote(message: string) {
    songNote = message;
    setTimeout(() => (songNote = null), 3000);
  }

  // ── Row actions (implemented in $lib/library/actions.ts) ──
  async function showSongDetails(song: Song) {
    try {
      const note = await revealOrOpenDetails(song);
      if (note) showSongNote(note);
    } catch (e) {
      console.error("details error:", e);
      showSongNote("打开详情失败: " + String(e));
    }
  }

  async function clearCache(song: Song) {
    try {
      showSongNote(await clearSongCache(song));
    } catch (e) {
      console.error("clear cache error:", e);
      showSongNote("清空缓存失败: " + String(e));
    }
  }

  async function downloadSong(song: Song) {
    try {
      const note = await downloadSongAction(song);
      if (note) showSongNote(note);
    } catch (e) {
      console.error("download error:", e);
      showSongNote("下载失败: " + String(e));
    }
  }

  // ── Context menu ──────────────────────────────────
  let ctxSong = $state<Song | null>(null);
  let ctxPos = $state<{ x: number; y: number } | null>(null);
  let sheetOpen = $state(false);
  let sortSheetOpen = $state(false);
  let bulkMetaSongs = $state<Song[] | null>(null);
  let bulkTagSongs = $state<Song[] | null>(null);
  const selection = new SelectionStore<Song>((song) => song.id);

  function selectedSongs(): Song[] {
    return selection.items(sortedSongs);
  }

  function clearSelection(): void {
    selection.clear();
  }

  function onRowClick(song: Song, index: number, e: MouseEvent) {
    if (e.ctrlKey || e.metaKey) {
      selection.toggle(song, index);
      return;
    }
    if (e.shiftKey) {
      selection.selectRange(sortedSongs, index);
      return;
    }
    if (selection.size > 0) selection.clear();
    clearHover();
    void playSong(song);
  }

  /** Touch: long-press enters selection mode and selects the row. */
  function onRowLongPress(song: Song, index: number) {
    if (!selection.has(song)) selection.setChecked(song, true, index);
    selection.anchor = index;
    clearHover();
  }

  function onRowContext(song: Song, index: number, e: MouseEvent) {
    if (!selection.has(song)) {
      selection.setKeys([song.id]);
      selection.anchor = index;
    }
    openContextMenu(song, e);
  }

  /** Open the bottom action sheet for one row (touch). */
  function openSongSheet(song: Song) {
    clearHover();
    ctxSong = song;
    ctxPos = null;
    sheetOpen = true;
  }

  /** "操作" in the mobile selection bar: reuse the multi-select menu. */
  function openBulkSheet() {
    const first = selectedSongs()[0];
    if (!first) return;
    ctxSong = first;
    ctxPos = null;
    sheetOpen = true;
  }

  async function bulkDownload() {
    try {
      const note = await downloadSelected(selectedSongs());
      if (note) showSongNote(note);
    } catch (e) {
      console.error("bulk download error:", e);
      showSongNote("下载失败: " + String(e));
    }
    clearSelection();
  }

  async function bulkClearCache() {
    try {
      showSongNote(await clearSelectedCache(selectedSongs()));
    } catch (e) {
      console.error("bulk clear cache error:", e);
      showSongNote("清空缓存失败: " + String(e));
    }
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
    libraryStore.applyMetadata(updated);
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
    if (platform.mobileUI) {
      // Touch: the action sheet renders the same MenuItem[] as the desktop
      // context menu, so permissions and labels cannot drift.
      ctxPos = null;
      sheetOpen = true;
    } else {
      ctxPos = { x: e.clientX, y: e.clientY };
    }
  }

  function closeContextMenu() {
    ctxSong = null;
    ctxPos = null;
    sheetOpen = false;
  }

  let contextMenuItems = $derived.by(() => {
    if (!ctxSong) return [];
    if (selection.size > 1 && selection.has(ctxSong)) {
      return [
        { label: "下载 " + selection.size + " 首", icon: Download, action: () => void bulkDownload() },
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
      // Reading, not editing: guests may download when the server exposes
      // media publicly. Cache cleanup stays login-only.
      ...(canDownload()
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

  // ── Columns: the shared ColumnLayout owns width / order / visibility ──
  let columns = $derived(layout.visible());
  let resizingCol = $state<string | null>(null);

  // Keep the columns filling the list whenever its width changes (window
  // resize, sidebar toggles, ...).
  $effect(() => {
    if (!headerEl) return;
    const ro = new ResizeObserver(() => {
      if (headerEl) layout.fitToWidth(headerEl.clientWidth - scrollbarWidth);
    });
    ro.observe(headerEl);
    return () => ro.disconnect();
  });

  function colStyle(column: TableColumnDef): string {
    const size = layout.widthOf(column.id);
    return "flex: 0 0 " + size + "px; width: " + size + "px; min-width: 0;";
  }

  /** Pointer-based resize (works with touch); the layout keeps the total. */
  function startColumnResize(e: PointerEvent, id: string) {
    if (layout.isFixed(id)) return;
    e.preventDefault();
    e.stopPropagation();
    const startX = e.clientX;
    const startWidth = layout.widthOf(id);
    resizingCol = id;
    function move(event: PointerEvent) {
      layout.setWidth(id, startWidth + event.clientX - startX);
    }
    function up() {
      resizingCol = null;
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
    }
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", up);
  }
</script>

<div class="library">
  {#if platform.mobileUI}
    <div class="mobile-library">
      {#if selection.size > 0}
        <div class="list-bar">
          <button class="bar-btn" onclick={() => selection.clear()}>取消</button>
          <span class="bar-title">已选 {selection.size} 首</span>
          <button class="bar-btn" onclick={() => selection.selectAll(sortedSongs)}>全选</button>
          <button class="bar-btn" onclick={openBulkSheet}>操作</button>
        </div>
      {:else}
        <div class="list-bar">
          <span class="bar-title">{sortedSongs.length} 首</span>
          <button class="bar-btn" onclick={() => (sortSheetOpen = true)}>
            排序{headerSort ? sortIndicator(headerSort.field) : ""}
          </button>
        </div>
      {/if}

      {#if loading}
        <div class="empty-state"><p>加载中...</p></div>
      {:else if error}
        <div class="empty-state">
          <p class="empty-title">加载失败</p>
          <p class="empty-desc">{error}</p>
        </div>
      {:else if sortedSongs.length === 0}
        <div class="empty-state">
          <Search size={28} class="empty-icon" />
          <p class="empty-title">没有歌曲</p>
          <p class="empty-desc">添加库目录或导入播放列表开始使用</p>
        </div>
      {:else}
        <MobileSongList
          bind:this={mobileList}
          songs={sortedSongs}
          playingId={player.mediaId}
          locateId={locatorTarget}
          {selection}
          onplay={(song) => void playSong(song)}
          onmenu={openSongSheet}
          onlongpress={onRowLongPress}
          onscroll={saveScrollTop}
        />
      {/if}
    </div>
  {:else}
    <div class="library-main">
    <!-- Table header -->
    <div class="table-header" bind:this={headerEl} style="padding-right: {scrollbarWidth}px">
      {#each columns as column (column.id)}
        <!-- Only the label starts a reorder drag, so the resize handle never
             competes with the native drag gesture. -->
        <div
          class="th"
          class:dragging={dragColId === column.id}
          style={colStyle(column)}
          ondragover={(e) => handleHeaderDragOver(e, column.id)}
          ondragleave={handleHeaderDragLeave}
          ondrop={(e) => handleHeaderDrop(e, column.id)}
          oncontextmenu={openColMenu}
        >
          <button
            class="th-label"
            draggable="true"
            onclick={() => toggleSort(column.id as ColumnId)}
            ondragstart={(e) => handleHeaderDragStart(e, column.id)}
            ondragend={handleHeaderDragEnd}
            title="点击排序，拖拽调序"
          >
            {column.label}{sortIndicator(column.id as ColumnId)}
          </button>
          {#if !column.fixed}
            <div
              class="resize-handle"
              class:resizing={resizingCol === column.id}
              title="拖动调宽，双击复位"
              onpointerdown={(e) => startColumnResize(e, column.id)}
              ondblclick={(e) => resetWidth(e, column.id)}
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
            {#each columns as column (column.id)}
              <div class="td" style={colStyle(column)}>
                {#if column.id === "num"}
                  {#if selection.size > 0}
                    <input
                      type="checkbox"
                      class="row-check"
                      checked={selection.has(song)}
                      onclick={(e) => e.stopPropagation()}
                      onchange={(e) => selection.setChecked(song, e.currentTarget.checked, i)}
                    />
                  {:else if isPlaying(song)}
                    <span class="playing-indicator">&#9654;</span>
                  {:else}
                    <span class="cell-num">{i + 1}</span>
                  {/if}
                {:else if column.id === "name"}
                  <div class="name-cell">
                    <span class="name-text">{displayTitle(song)}</span>
                    {#if platformTag(song)}
                      <span class="tag-badge" class:local={platformTag(song)!.kind == "local"}>
                        {platformTag(song)!.label}
                      </span>
                    {/if}
                  </div>
                {:else if column.id === "tag"}
                  <div class="tag-cell">
                    {#each song.tags as tag}
                      <span
                        class="tag-pill"
                        style="--tag-hue: {tagHue(tag)}"
                        title={tag.namespace + ":" + tag.value}
                      >{tag.namespace}:{tag.value}</span>
                    {/each}
                  </div>
                {:else if column.id === "artist"}
                  <span class="cell-text">{displayText(song, "artist")}</span>
                {:else if column.id === "album"}
                  <span class="cell-text">{displayText(song, "album")}</span>
                {:else if column.id === "time"}
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
  {/if}
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
      libraryStore.applyMany(updated);
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
      libraryStore.applyMany(updated);
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

{#if sheetOpen && contextMenuItems.length > 0}
  <MenuSheet
    bind:open={sheetOpen}
    title={ctxSong ? displayTitle(ctxSong) : ""}
    items={contextMenuItems}
  />
{/if}

{#if sortSheetOpen}
  <MenuSheet bind:open={sortSheetOpen} title="排序" items={sortMenuItems} />
{/if}

{#if colMenu}
  <ContextMenu items={colMenuItems} x={colMenu.x} y={colMenu.y} onclose={() => (colMenu = null)} />
{/if}

<style>
  .library {
    display: flex;
    height: 100%;
  }

  /* ── Mobile list ─────────────────────────────────── */
  .mobile-library {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .list-bar {
    flex: none;
    display: flex;
    align-items: center;
    gap: 6px;
    height: 40px;
    padding: 0 6px 0 12px;
    border-bottom: 1px solid var(--panel-border);
    background: var(--color-base-100);
  }

  .bar-title {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    color: color-mix(in oklch, var(--color-base-content) 60%, transparent);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .bar-btn {
    flex: none;
    height: 30px;
    padding: 0 10px;
    border: 1px solid var(--panel-border);
    border-radius: 7px;
    background: var(--color-base-200);
    color: color-mix(in oklch, var(--color-base-content) 85%, transparent);
    font-size: 12px;
    cursor: pointer;
  }

  .bar-btn:active {
    border-color: var(--color-primary);
    color: var(--color-primary);
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
    height: 30px;
    min-width: 0;
    overflow: hidden;
    background: var(--color-base-200);
    border-bottom: 1px solid var(--color-base-300);
    font-size: 11px;
    font-weight: 600;
    color: color-mix(in oklch, var(--color-base-content) 70%, transparent);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    user-select: none;
  }

  .th {
    position: relative;
    display: flex;
    align-items: center;
    padding: 0 6px;
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
  .resize-handle.resizing::after { background: color-mix(in oklch, var(--color-primary) 60%, transparent); }

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
    height: 34px;
    min-width: 0;
    color: var(--color-base-content);
    font-size: 13px;
    cursor: pointer;
    transition: background 0.1s;
  }
  .song-row:hover { background: color-mix(in oklch, var(--color-base-300) 70%, transparent); }
  .song-row.is-playing { background: color-mix(in oklch, var(--color-primary) 12%, transparent); }
  .song-row.is-playing .name-text { color: var(--color-primary); }
  .song-row.located { background: color-mix(in oklch, var(--color-primary) 20%, transparent); }
  .song-row.located .name-text { color: var(--color-primary); font-weight: 600; }

  .td {
    display: flex;
    align-items: center;
    padding: 0 6px;
    min-width: 0;
    user-select: text; /* allow copying */
  }

  .cell-num { color: color-mix(in oklch, var(--color-base-content) 50%, transparent); font-size: 12px; }
  .playing-indicator { color: var(--color-primary); font-size: 10px; }
  .row-check { width: 13px; height: 13px; accent-color: var(--color-primary); cursor: pointer; }

  .name-cell { display: flex; align-items: center; gap: 6px; min-width: 0; width: 100%; }
  .name-text { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tag-badge {
    flex: none; font-size: 10px; padding: 1px 5px; border-radius: 4px;
    background: color-mix(in oklch, var(--color-primary) 15%, transparent); color: var(--color-primary);
  }
  .tag-badge.local { background: color-mix(in oklch, var(--color-success) 15%, transparent); color: var(--color-success); }

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
  .cell-time { color: color-mix(in oklch, var(--color-base-content) 60%, transparent); font-size: 12px; }

  .empty-state {
    flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center;
    gap: 8px; padding: 40px; color: color-mix(in oklch, var(--color-base-content) 50%, transparent);
  }
  .empty-icon { opacity: 0.4; }
  .empty-title { font-size: 15px; font-weight: 500; color: color-mix(in oklch, var(--color-base-content) 70%, transparent); }
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
  .download-title { margin: 0; font-size: 12px; color: color-mix(in oklch, var(--color-base-content) 70%, transparent); }
  .download-label { font-size: 11px; color: color-mix(in oklch, var(--color-base-content) 60%, transparent); }
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
    background: var(--color-base-100); color: color-mix(in oklch, var(--color-base-content) 80%, transparent);
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
