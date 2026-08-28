<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount, tick } from "svelte";
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
    /** Flex rank (relative grow weight). Kept for reference only. */
    flex: number;
    /** Fixed pixel width for truly fixed columns (# and time) — not resizable. */
    fixed?: number;
    /** Default pixel width for resizable columns. */
    defWidth?: number;
    /** Minimum pixel width when resizing. */
    min?: number;
  }

  // Name column flex:1 → grows to fill remaining space; others use flex weights.
  const COLUMN_DEFS: ColumnDef[] = [
    { id: "num", label: "#", flex: 0, fixed: 40 },
    { id: "name", label: "名称", flex: 1, min: 120, defWidth: 340 },
    { id: "tag", label: "标签", flex: 0.55, min: 80, defWidth: 180 },
    { id: "artist", label: "歌手", flex: 0.45, min: 80, defWidth: 140 },
    { id: "album", label: "专辑", flex: 0.5, min: 80, defWidth: 150 },
    { id: "time", label: "时长", flex: 0, fixed: 52 },
  ];

  interface ColumnState {
    id: ColumnId;
    shown: boolean;
    /**
     * Width. For resizable columns this is a flex-weight (a share of the
     * available width), e.g. name=1, tag=0.55. For fixed columns (# / time) it
     * is a pixel width. Column widths are derived from these weights, so the
     * row always fills the container and the rightmost column stays pinned to
     * the right edge.
     */
    width: number;
  }

  // Resizable columns use flex weights (share of available width).
  const DEFAULT_COLUMNS: ColumnState[] = [
    { id: "num", shown: true, width: 40 },
    { id: "name", shown: true, width: 1 },
    { id: "tag", shown: true, width: 0.55 },
    { id: "artist", shown: true, width: 0.45 },
    { id: "album", shown: true, width: 0.5 },
    { id: "time", shown: true, width: 52 },
  ];

  const COL_KEY = "vide_list_columns";

  function loadColumns(): ColumnState[] {
    try {
      const saved = localStorage.getItem(COL_KEY);
      if (saved) {
        const data = JSON.parse(saved) as ColumnState[];
        return COLUMN_DEFS.map((def) => {
          const dflt = DEFAULT_COLUMNS.find((d) => d.id === def.id)!;
          const found = data.find((c) => c.id === def.id);
          if (!found) {
            return { id: def.id, shown: true, width: dflt.width };
          }
          let w = found.width;
          // Fixed columns (# / time) keep their px width.
          if (def.fixed != null) {
            return { id: def.id, shown: !!found.shown, width: def.fixed };
          }
          // Resizable columns store a flex weight. Migrate any legacy px value
          // (> 2, from the earlier px-based scheme) back to the default weight.
          if (w > 2) w = dflt.width;
          return { id: def.id, shown: !!found.shown, width: w };
        });
      }
    } catch {
      /* ignore */
    }
    return DEFAULT_COLUMNS.map((c) => ({ ...c }));
  }

  let columns = $state<ColumnState[]>(loadColumns());
  let visibleColumns = $derived(columns.filter((c) => c.shown));
  let dragColId = $state<ColumnId | null>(null);
  let resizeColId = $state<ColumnId | null>(null);
  let resizeStartX = $state(0);
  let resizeStartWidth = $state(0);

  function saveColumns() {
    try {
      localStorage.setItem(COL_KEY, JSON.stringify(columns));
    } catch {
      /* ignore */
    }
  }

  function updateWidth(id: ColumnId, width: number) {
    const def = COLUMN_DEFS.find((d) => d.id === id)!;
    // Fixed columns (num/time) are not resizable.
    if (def.fixed != null) return;
    const col = columns.find((c) => c.id === id)!;
    // Width is a flex weight; clamp to a sensible minimum so columns never vanish.
    col.width = Math.max(width, def.min ?? 0.1);
    columns = columns.map((c) => (c.id === id ? { ...c } : c));
  }

  // Resize map: the column being dragged plus the visible column to its right
  // (if any). Dragging the boundary shifts weight from one to the other so the
  // total stays constant and the other columns do not move.
  let resizePair = $state<{ left: ColumnId; right: ColumnId | null; startW: number } | null>(null);

  function startPairResize(e: PointerEvent, id: ColumnId) {
    e.preventDefault();
    e.stopPropagation();
    const th = (e.currentTarget as HTMLElement).closest(".th") as HTMLElement | null;
    if (th) th.draggable = false;
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    // The draggable column is `id`; the next visible column is its right neighbour.
    const visible = visibleColumns.map((c) => c.id);
    const idx = visible.indexOf(id);
    const right = idx >= 0 && idx + 1 < visible.length ? visible[idx + 1] : null;
    const rightDef = right !== null ? COLUMN_DEFS.find((d) => d.id === right)! : null;
    resizePair = {
      left: id,
      right: rightDef && rightDef.fixed == null ? right : null,
      startW: columns.find((c) => c.id === id)!.width,
    };
    resizeColId = id;
    resizeStartX = e.clientX;
    resizeStartWidth = columns.find((c) => c.id === id)!.width;
  }

  // ── Reorder (drag header) — horizontal-only drag ghost ──
  function handleHeaderDragStart(e: DragEvent, id: ColumnId) {
    // If a resize is in progress, suppress reorder drag (HTML5 drag would
    // otherwise swallow pointer events and break the resize gesture).
    if (resizeColId) {
      e.preventDefault();
      e.stopPropagation();
      return;
    }
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

  function handleHeaderDragOver(e: DragEvent, id: ColumnId) {
    e.preventDefault();
    if (dragColId && dragColId !== id) {
      (e.currentTarget as HTMLElement).classList.add("drop-target");
    }
  }

  function handleHeaderDragLeave(e: DragEvent) {
    (e.currentTarget as HTMLElement).classList.remove("drop-target");
  }

  function handleHeaderDrop(e: DragEvent, targetId: ColumnId) {
    e.preventDefault();
    (e.currentTarget as HTMLElement).classList.remove("drop-target");
    if (!dragColId || dragColId === targetId) return;
    const from = columns.findIndex((c) => c.id === dragColId);
    const to = columns.findIndex((c) => c.id === targetId);
    if (from === -1 || to === -1) return;
    const next = [...columns];
    const [moved] = next.splice(from, 1);
    next.splice(to, 0, moved);
    columns = next;
    dragColId = null;
    saveColumns();
  }

  function handleHeaderDragEnd() {
    dragColId = null;
  }

  // ── Resize (drag handle) — pointer capture for reliable up/down ──
  function startResize(e: PointerEvent, id: ColumnId) {
    startPairResize(e, id);
  }

  function onResizeMove(e: PointerEvent) {
    if (!resizeColId || !resizePair) return;
    const delta = e.clientX - resizeStartX;
    // Convert the pixel delta to a weight delta relative to the container width.
    const containerW = listEl ? listEl.clientWidth : 600;
    const wDelta = delta / Math.max(containerW, 1);
    const leftId = resizePair.left;
    const rightId = resizePair.right;
    if (rightId) {
      // Shift weight from left column to right column and back, keeping the
      // total constant so the other columns do not move.
      const lc = columns.find((c) => c.id === leftId)!;
      const rc = columns.find((c) => c.id === rightId)!;
      const lw = Math.max(resizePair.startW + wDelta, COLUMN_DEFS.find((d) => d.id === leftId)!.min ?? 0.1);
      const rw = Math.max(rc.width - (lw - resizePair.startW), COLUMN_DEFS.find((d) => d.id === rightId)!.min ?? 0.1);
      columns = columns.map((c) =>
        c.id === leftId ? { ...c, width: lw } : c.id === rightId ? { ...c, width: rw } : c,
      );
    } else {
      updateWidth(leftId, resizePair.startW + wDelta);
    }
  }

  function onResizeEnd(e: PointerEvent) {
    const el = e.currentTarget as HTMLElement;
    if (el?.hasPointerCapture?.(e.pointerId)) el.releasePointerCapture(e.pointerId);
    // Re-enable reorder drag on the column header now that resizing is done.
    const th = el?.closest?.(".th") as HTMLElement | null;
    if (th) th.draggable = true;
    resizeColId = null;
    resizePair = null;
    saveColumns();
  }

  // Double-click resets to the default (flex) weight.
  function resetWidth(e: MouseEvent, id: ColumnId) {
    e.preventDefault();
    e.stopPropagation();
    const def = COLUMN_DEFS.find((d) => d.id === id)!;
    const col = columns.find((c) => c.id === id)!;
    // Reset to the default weight (fixed columns keep their px).
    col.width = def.fixed ?? DEFAULT_COLUMNS.find((d) => d.id === id)!.width;
    columns = columns.map((c) => (c.id === id ? { ...c } : c));
    saveColumns();
  }

  // ── Header context menu (show/hide columns) ───────
  let colMenu = $state<{ x: number; y: number } | null>(null);
  let colMenuItems = $state<MenuItem[]>([]);

  function openColMenu(e: MouseEvent) {
    e.preventDefault();
    const items: MenuItem[] = COLUMN_DEFS.map((def) => {
      const shown = columns.find((c) => c.id === def.id)!.shown;
      return {
        label: (shown ? "✓ " : "   ") + def.label,
        action: () => {
          const col = columns.find((c) => c.id === def.id)!;
          col.shown = !col.shown;
          // Ensure at least one column stays visible.
          if (!columns.some((c) => c.shown)) col.shown = true;
          columns = columns.map((c) => (c.id === def.id ? { ...c } : c));
          saveColumns();
        },
      };
    });
    colMenuItems = items;
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

  function isFixed(id: ColumnId): boolean {
    return COLUMN_DEFS.find((d) => d.id === id)!.fixed != null;
  }

  function colStyle(id: ColumnId): string {
    const def = COLUMN_DEFS.find((d) => d.id === id)!;
    const w = columns.find((c) => c.id === id)!.width;
    // Fixed columns (# / time) use px. Resizable columns use a flex weight, so
    // the row always fills the container and the rightmost column stays pinned
    // to the right edge.
    if (def.fixed != null) {
      return "flex: 0 0 " + w + "px; width: " + w + "px; min-width: 0;";
    }
    return "flex: " + w + " 1 0; min-width: 0;";
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
    <div class="table-header" style="padding-right: {scrollbarWidth}px">
      {#each visibleColumns as col}
        <div
          class="th"
          class:dragging={dragColId === col.id}
          style={colStyle(col.id)}
          draggable="true"
          ondragstart={(e) => handleHeaderDragStart(e, col.id)}
          ondragover={(e) => handleHeaderDragOver(e, col.id)}
          ondragleave={handleHeaderDragLeave}
          ondrop={(e) => handleHeaderDrop(e, col.id)}
          ondragend={handleHeaderDragEnd}
          oncontextmenu={openColMenu}
        >
          <button class="th-label" onclick={() => toggleSort(col.id)} title="点击排序，拖拽调序">
            {labelOf(col.id)}{sortIndicator(col.id)}
          </button>
          {#if !isFixed(col.id)}
            <div
              class="resize-handle"
              title="拖动调宽，双击复位"
              onpointerdown={(e) => startResize(e, col.id)}
              onpointermove={onResizeMove}
              onpointerup={onResizeEnd}
              onpointercancel={onResizeEnd}
              ondblclick={(e) => resetWidth(e, col.id)}
            ></div>
          {/if}
        </div>
      {/each}
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
            class:playing={isPlaying(song)}
            class:located={locatorTarget === song.id}
            data-song-id={song.id}
            onclick={() => playSong(song)}
            oncontextmenu={(e) => openContextMenu(song, e)}
          >
            {#each visibleColumns as col}
              <div class="td" style={colStyle(col.id)}>
                {#if col.id === "num"}
                  {#if isPlaying(song)}
                    <span class="playing-indicator">&#9654;</span>
                  {:else}
                    <span class="cell-num">{i + 1}</span>
                  {/if}
                {:else if col.id === "name"}
                  <div class="name-cell">
                    <span class="name-text">{song.title}</span>
                    {#if platformTag(song)}
                      <span class="tag-badge" class:local={platformTag(song)!.kind == "local"}>
                        {platformTag(song)!.label}
                      </span>
                    {/if}
                  </div>
                {:else if col.id === "tag"}
                  <div class="tag-cell">
                    {#if song.tags.length > 0}
                      <span class="tag-cell-text">{tagText(song)}</span>
                    {/if}
                  </div>
                {:else if col.id === "artist"}
                  <span class="cell-text">{displayText(song, "artist")}</span>
                {:else if col.id === "album"}
                  <span class="cell-text">{displayText(song, "album")}</span>
                {:else if col.id === "time"}
                  <span class="cell-time">{formatDuration(song.metadata.duration)}</span>
                {/if}
              </div>
            {/each}
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
  }
  .resize-handle::after {
    content: ""; position: absolute; left: 50%; top: 6px; bottom: 6px;
    width: 1px; background: transparent; transition: background 0.12s;
  }
  .resize-handle:hover::after { background: var(--color-primary) / 0.6; }

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
  .song-row.playing { background: var(--color-primary) / 0.12; }
  .song-row.playing .name-text { color: var(--color-primary); }
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
