<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { Search, Play, Download, ListPlus, ExternalLink, Tag as TagIcon } from "lucide-svelte";
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
    /** Flex rank (relative grow weight). name is 1 (fills remaining). */
    flex: number;
    /** Fixed pixel width for # and time columns (resize disabled). */
    fixed?: number;
    /** Minimum flex weight when resizing. */
    min?: number;
  }

  // Name column flex:1 → grows to fill remaining space; others use flex weights.
  const COLUMN_DEFS: ColumnDef[] = [
    { id: "num", label: "#", flex: 0, fixed: 40 },
    { id: "name", label: "名称", flex: 1, min: 0.5 },
    { id: "tag", label: "标签", flex: 0.55, min: 0.2 },
    { id: "artist", label: "歌手", flex: 0.45, min: 0.15 },
    { id: "album", label: "专辑", flex: 0.5, min: 0.2 },
    { id: "time", label: "时长", flex: 0, fixed: 52 },
  ];

  interface ColumnState {
    id: ColumnId;
    shown: boolean;
    /** px for fixed columns; flex weight for flexible columns. */
    width: number;
  }

  // Default order: # / name / tag / artist / album / time (name flex:1 fills rest).
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
          const found = data.find((c) => c.id === def.id);
          return found
            ? {
                id: def.id,
                shown: !!found.shown,
                width: def.fixed != null ? def.fixed : Math.max(found.width, def.min ?? 0.1),
              }
            : { id: def.id, shown: true, width: def.fixed ?? def.flex };
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
    col.width = Math.max(width, def.min ?? 0.1);
    columns = columns.map((c) => (c.id === id ? { ...c } : c));
  }

  // ── Reorder (drag header) — horizontal-only drag ghost ──
  function handleHeaderDragStart(e: DragEvent, id: ColumnId) {
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
    e.preventDefault();
    e.stopPropagation();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    resizeColId = id;
    resizeStartX = e.clientX;
    resizeStartWidth = columns.find((c) => c.id === id)!.width;
  }

  function onResizeMove(e: PointerEvent) {
    if (!resizeColId) return;
    const delta = e.clientX - resizeStartX;
    // Convert pixel delta to a flex-weight delta (name col ≈ 500px ⇔ weight 1).
    updateWidth(resizeColId, resizeStartWidth + delta / 500);
  }

  function onResizeEnd(e: PointerEvent) {
    const el = e.currentTarget as HTMLElement;
    if (el?.hasPointerCapture?.(e.pointerId)) el.releasePointerCapture(e.pointerId);
    resizeColId = null;
    saveColumns();
  }

  // Double-click resets to default (flex) weight.
  function resetWidth(e: MouseEvent, id: ColumnId) {
    e.preventDefault();
    e.stopPropagation();
    const def = COLUMN_DEFS.find((d) => d.id === id)!;
    const col = columns.find((c) => c.id === id)!;
    col.width = def.fixed ?? def.flex;
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

  onMount(async () => {
    await searchPresets.load();
    activePresetId = searchPresets.activeId;
    await loadSongs();
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

  async function playSong(song: Song) {
    try {
      await invoke("play_song", { id: song.id });
      player.mediaId = song.id;
      player.title = song.title;
      player.artist = song.artist;
      player.album = song.album;
      player.isPlaying = true;
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
      { label: "下载", icon: Download, action: () => {}, disabled: true },
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
    if (def.fixed != null) return "width: " + def.fixed + "px; flex: 0 0 " + def.fixed + "px;";
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
    <div class="table-header">
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
      <div class="song-list">
        {#each sortedSongs as song, i}
          <div
            class="song-row"
            class:playing={isPlaying(song)}
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
      </div>
    {/if}
  </div>
</div>

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
    overflow: hidden;
    border-right: 1px solid transparent;
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
    right: -3px;
    top: 0;
    bottom: 0;
    width: 7px;
    cursor: col-resize;
    z-index: 2;
  }
  .resize-handle::after {
    content: ""; position: absolute; left: 50%; top: 6px; bottom: 6px;
    width: 1px; background: transparent; transition: background 0.12s;
  }
  .resize-handle:hover::after { background: var(--color-primary) / 0.6; }

  /* Body */
  .song-list { flex: 1; overflow-y: auto; overflow-x: hidden; min-width: 0; }
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
</style>
