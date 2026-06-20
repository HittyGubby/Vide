<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { Search, Tag, Play, Download, ListPlus } from "lucide-svelte";
  import { player } from "$lib/stores/player.svelte";
  import { searchStore } from "$lib/stores/search.svelte";
  import SearchHistory from "$lib/components/search/SearchHistory.svelte";
  import TagEditor from "$lib/components/tag/TagEditor.svelte";
  import ContextMenu from "$lib/components/song/ContextMenu.svelte";
  import type { Song } from "$lib/types";
  import type { MenuItem } from "$lib/components/song/ContextMenu.svelte";

  interface ColumnDef {
    id: string;
    label: string;
    width: string;
  }

  const ALL_COLUMNS: ColumnDef[] = [
    { id: "title", label: "标题", width: "2fr" },
    { id: "artist", label: "歌手", width: "1.5fr" },
    { id: "album", label: "专辑", width: "1.5fr" },
    { id: "tags", label: "标签", width: "1.5fr" },
    { id: "duration", label: "时长", width: "70px" },
  ];

  let songs = $state<Song[]>([]);
  let query = $state("");
  let sortField = $state("title");
  let sortAsc = $state(true);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let currentMediaId = $state<string | null>(null);
  let showHistory = $state(false);
  let historyTimer: ReturnType<typeof setTimeout> | undefined;

  onMount(() => {
    loadSongs();
  });

  async function loadSongs() {
    loading = true;
    error = null;
    try {
      songs = await invoke("list_songs");
    } catch (e) {
      error = String(e);
      songs = [];
    } finally {
      loading = false;
    }
  }

  // Keep currentMediaId in sync with the shared store
  $effect(() => {
    currentMediaId = player.mediaId;
  });

  // Derive filtered + sorted results
  let filteredSongs = $derived.by(() => {
    let result = songs;
    const q = query.trim().toLowerCase();
    if (q) {
      result = result.filter((s) => {
        if (s.title.toLowerCase().includes(q)) return true;
        if (s.artist.toLowerCase().includes(q)) return true;
        if (s.album.toLowerCase().includes(q)) return true;
        // Also search file paths
        if (s.paths.some((p) => p.toLowerCase().includes(q))) return true;
        return false;
      });
    }

    return [...result].sort((a, b) => {
      let cmp = 0;
      switch (sortField) {
        case "title":
          cmp = a.title.localeCompare(b.title);
          break;
        case "artist":
          cmp = a.artist.localeCompare(b.artist);
          break;
        case "album":
          cmp = a.album.localeCompare(b.album);
          break;
        case "duration":
          cmp = (a.metadata.duration ?? 0) - (b.metadata.duration ?? 0);
          break;
        case "tags":
          cmp = a.tags.length - b.tags.length;
          break;
        default:
          cmp = 0;
      }
      return sortAsc ? cmp : -cmp;
    });
  });

  function toggleSort(field: string) {
    if (sortField === field) {
      sortAsc = !sortAsc;
    } else {
      sortField = field;
      sortAsc = true;
    }
  }

  function sortIndicator(field: string): string {
    if (sortField !== field) return "";
    return sortAsc ? " ▲" : " ▼";
  }

  function formatDuration(seconds?: number): string {
    if (!seconds || seconds <= 0) return "--:--";
    const m = Math.floor(seconds / 60);
    const s = Math.floor(seconds % 60);
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  function displayTags(song: Song): string {
    // Show up to 2 tags as "ns:val" + count remaining
    const shown = song.tags.slice(0, 2);
    const rest = song.tags.length - shown.length;
    const parts = shown.map((t) => `${t.namespace}:${t.value}`);
    if (rest > 0) parts.push(`+${rest}`);
    return parts.join(", ");
  }

  async function playSong(song: Song) {
    try {
      await invoke("play_song", { id: song.id });
      player.mediaId = song.id;
      player.title = song.title;
      player.artist = song.artist;
      player.isPlaying = true;
    } catch (e) {
      console.error("play error:", e);
    }
  }

  function isPlaying(song: Song): boolean {
    return currentMediaId === song.id;
  }

  // ── Tag Editing ────────────────────────────────────

  let editingSong = $state<Song | null>(null);

  function openTagEditor(song: Song, e: MouseEvent) {
    e.stopPropagation();
    editingSong = song;
  }

  function closeTagEditor() {
    songs = songs;
    editingSong = null;
  }

  // ── Column Order & Visibility ───────────────────────

  const COL_KEY = "vide_columns";

  function loadColumnState() {
    try {
      const saved = localStorage.getItem(COL_KEY);
      if (saved) {
        const data = JSON.parse(saved);
        return {
          order: data.order as string[],
          visible: data.visible as Record<string, boolean>,
        };
      }
    } catch {
      /* ignore */
    }
    return {
      order: ALL_COLUMNS.map((c) => c.id),
      visible: Object.fromEntries(ALL_COLUMNS.map((c) => [c.id, true])),
    };
  }

  const initialState = loadColumnState();
  let columnOrder = $state<string[]>(initialState.order);
  let columnVisible = $state<Record<string, boolean>>(initialState.visible);

  let visibleColumns = $derived(
    columnOrder
      .filter((id) => columnVisible[id])
      .map((id) => ALL_COLUMNS.find((c) => c.id === id)!),
  );

  function saveColumnState() {
    try {
      localStorage.setItem(
        COL_KEY,
        JSON.stringify({ order: columnOrder, visible: columnVisible }),
      );
    } catch {
      /* ignore */
    }
  }

  // ── Column Drag & Drop ────────────────────────────────

  let dragColId = $state<string | null>(null);

  function handleDragStart(id: string) {
    dragColId = id;
  }

  function handleDragOver(e: DragEvent, id: string) {
    e.preventDefault();
    if (dragColId && dragColId !== id) {
      (e.currentTarget as HTMLElement).classList.add("drop-target");
    }
  }

  function handleDragLeave(e: DragEvent) {
    (e.currentTarget as HTMLElement).classList.remove("drop-target");
  }

  function handleDrop(e: DragEvent, targetId: string) {
    e.preventDefault();
    (e.currentTarget as HTMLElement).classList.remove("drop-target");
    if (!dragColId || dragColId === targetId) return;
    const fromIdx = columnOrder.indexOf(dragColId);
    const toIdx = columnOrder.indexOf(targetId);
    if (fromIdx === -1 || toIdx === -1) return;
    const newOrder = [...columnOrder];
    newOrder.splice(fromIdx, 1);
    newOrder.splice(toIdx, 0, dragColId);
    columnOrder = newOrder;
    dragColId = null;
    saveColumnState();
  }

  function handleDragEnd() {
    dragColId = null;
  }

  // ── Column Header Context Menu ────────────────────────

  let colMenu = $state<{ x: number; y: number } | null>(null);
  let colMenuItems = $state<MenuItem[]>([]);

  function openColMenu(e: MouseEvent, col: ColumnDef) {
    e.preventDefault();
    const items: MenuItem[] = ALL_COLUMNS.map((c) => ({
      label: c.label,
      action: () => {
        columnVisible[c.id] = !columnVisible[c.id];
        // Ensure at least one column remains visible
        const remaining = Object.values(columnVisible).filter(Boolean).length;
        if (remaining === 0) columnVisible[c.id] = true;
        columnVisible = { ...columnVisible };
        saveColumnState();
      },
    }));
    items.push({ label: "", action: () => {}, divider: true });
    items.push({
      label: "重置列布局",
      action: () => {
        columnOrder = ALL_COLUMNS.map((c) => c.id);
        columnVisible = Object.fromEntries(
          ALL_COLUMNS.map((c) => [c.id, true]),
        );
        saveColumnState();
      },
    });

    colMenuItems = items.map((it) => ({
      ...it,
      label: columnVisible[it.label] ? `✓ ${it.label}` : `  ${it.label}`,
    }));
    // Rebuild with checked indicators
    colMenuItems = ALL_COLUMNS.map((c) => ({
      label: columnVisible[c.id] ? `✓ ${c.label}` : `  ${c.label}`,
      action: () => {
        columnVisible[c.id] = !columnVisible[c.id];
        const remaining = Object.values(columnVisible).filter(Boolean).length;
        if (remaining === 0) columnVisible[c.id] = true;
        columnVisible = { ...columnVisible };
        saveColumnState();
      },
    }));
    colMenuItems.push({ label: "", action: () => {}, divider: true });
    colMenuItems.push({
      label: "重置列布局",
      action: () => {
        columnOrder = ALL_COLUMNS.map((c) => c.id);
        columnVisible = Object.fromEntries(
          ALL_COLUMNS.map((c) => [c.id, true]),
        );
        saveColumnState();
      },
    });

    colMenu = { x: e.clientX, y: e.clientY };
  }

  // ── Song Right-Click Context Menu ─────────────────────

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
    return [
      {
        label: "播放",
        icon: Play,
        action: () => playSong(ctxSong!),
      },
      {
        label: "编辑标签",
        icon: Tag,
        action: () => {
          editingSong = ctxSong;
        },
      },
      { label: "", action: () => {}, icon: undefined, divider: true },
      {
        label: "下载",
        icon: Download,
        action: () => {},
        disabled: true,
      },
      {
        label: "添加到播放列表",
        icon: ListPlus,
        action: () => {},
        disabled: true,
      },
    ] as MenuItem[];
  });

  // ── Search History ──────────────────────────────────

  function handleSearchInput(e: Event) {
    const target = e.target as HTMLInputElement;
    query = target.value;
    if (target.value) {
      showHistory = false;
    }
  }

  function handleSearchFocus() {
    clearTimeout(historyTimer);
    if (!query.trim()) {
      showHistory = searchStore.history.length > 0;
    }
  }

  function handleSearchBlur() {
    historyTimer = setTimeout(() => {
      showHistory = false;
    }, 200);
  }

  function handleSearchKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      if (query.trim()) {
        searchStore.add(query);
        showHistory = false;
      }
    } else if (e.key === "Escape") {
      showHistory = false;
    }
  }

  function selectHistory(q: string) {
    query = q;
    showHistory = false;
  }
</script>

<div class="library-view">
  <!-- Local search bar -->
  <div class="search-bar">
    <div class="search-input-wrapper">
      <span class="search-icon">
        <Search size={14} />
      </span>
      <div class="search-field">
        <input
          type="text"
          class="local-search"
          placeholder="搜索我的音乐... (留空 = 我的库)"
          value={query}
          oninput={handleSearchInput}
          onfocus={handleSearchFocus}
          onblur={handleSearchBlur}
          onkeydown={handleSearchKeydown}
        />
        {#if showHistory}
          <SearchHistory
            onselect={selectHistory}
            onclose={() => (showHistory = false)}
          />
        {/if}
      </div>
    </div>
    {#if filteredSongs.length > 0}
      <span class="song-count">{filteredSongs.length}</span>
    {/if}
  </div>

  <!-- Song table header -->
  <div class="table-header" role="row">
    <div class="col col-index">#</div>
    {#each visibleColumns as col}
      <button
        class="col"
        class:dragging={dragColId === col.id}
        style="flex: {col.width}"
        draggable="true"
        onclick={() => toggleSort(col.id)}
        oncontextmenu={(e) => openColMenu(e, col)}
        ondragstart={() => handleDragStart(col.id)}
        ondragover={(e) => handleDragOver(e, col.id)}
        ondragleave={handleDragLeave}
        ondrop={(e) => handleDrop(e, col.id)}
        ondragend={handleDragEnd}
        class:col-active={sortField === col.id}
      >
        {col.label}{sortIndicator(col.id)}
      </button>
    {/each}
  </div>

  <!-- Song rows or states -->
  {#if loading}
    <div class="empty-state">
      <p class="empty-title">加载中...</p>
    </div>
  {:else if error}
    <div class="empty-state">
      <p class="empty-title">加载失败</p>
      <p class="empty-desc">{error}</p>
      <button class="retry-btn" onclick={loadSongs}>重试</button>
    </div>
  {:else if filteredSongs.length === 0}
    <div class="empty-state">
      <div class="empty-icon">
        {#if query.trim()}
          <Search size={32} />
        {:else}
          <Tag size={32} />
        {/if}
      </div>
      <p class="empty-title">
        {query.trim() ? "没有匹配的歌曲" : "还没有歌曲"}
      </p>
      <p class="empty-desc">
        {query.trim() ? "尝试其他关键词" : "添加库目录或导入播放列表开始使用"}
      </p>
    </div>
  {:else}
    <div class="song-list">
      {#each filteredSongs as song, i}
        <button
          class="song-row"
          class:playing={isPlaying(song)}
          onclick={() => playSong(song)}
          ondblclick={() => playSong(song)}
          oncontextmenu={(e) => openContextMenu(song, e)}
        >
          <div class="col col-index">
            {#if isPlaying(song)}
              <span class="playing-indicator">▶</span>
            {:else}
              {i + 1}
            {/if}
          </div>
          {#each visibleColumns as col}
            <div class="col" style="flex: {col.width}">
              {#if col.id === "title"}
                <span class="truncate song-title-cell">{song.title}</span>
              {:else if col.id === "artist"}
                <span class="truncate">{song.artist}</span>
              {:else if col.id === "album"}
                <span class="truncate">{song.album}</span>
              {:else if col.id === "tags"}
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div
                  class="tags-btn"
                  role="button"
                  tabindex="0"
                  onclick={(e) => openTagEditor(song, e)}
                  onkeydown={(e) => {
                    if (e.key === "Enter" || e.key === " ")
                      openTagEditor(song, e as any);
                  }}
                >
                  <span class="truncate tags-cell">{displayTags(song)}</span>
                </div>
              {:else if col.id === "duration"}
                <span>{formatDuration(song.metadata.duration)}</span>
              {/if}
            </div>
          {/each}
        </button>
      {/each}
    </div>
  {/if}
</div>

{#if editingSong}
  <TagEditor song={editingSong} onclose={closeTagEditor} />
{/if}

{#if ctxPos && contextMenuItems.length > 0}
  <ContextMenu
    items={contextMenuItems}
    x={ctxPos.x}
    y={ctxPos.y}
    onclose={closeContextMenu}
  />
{/if}

{#if colMenu}
  <ContextMenu
    items={colMenuItems}
    x={colMenu.x}
    y={colMenu.y}
    onclose={() => (colMenu = null)}
  />
{/if}

<style>
  .library-view {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  /* ── Search Bar ──────────────────────────────────── */
  .search-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 16px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .search-input-wrapper {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1;
    height: 30px;
    padding: 0 10px;
    background: var(--bg-input);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    transition: border-color 0.15s;
  }

  .search-input-wrapper:focus-within {
    border-color: var(--accent-primary);
  }

  .search-field {
    position: relative;
    flex: 1;
    height: 100%;
  }

  .search-icon {
    color: var(--text-tertiary);
    flex-shrink: 0;
    display: flex;
  }

  .local-search {
    flex: 1;
    height: 100%;
    background: none;
    border: none;
    color: var(--text-primary);
    font-size: 13px;
    outline: none;
    user-select: text;
  }

  .local-search::placeholder {
    color: var(--text-tertiary);
  }

  .song-count {
    font-size: 11px;
    color: var(--text-tertiary);
    background: var(--bg-elevated);
    padding: 2px 8px;
    border-radius: 10px;
    white-space: nowrap;
  }

  /* ── Table Header ────────────────────────────────── */
  .table-header {
    display: flex;
    align-items: center;
    height: 32px;
    padding: 0 12px 0 8px;
    background: var(--bg-surface);
    border-bottom: 1px solid var(--border-subtle);
    font-size: 11px;
    font-weight: 500;
    color: var(--text-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .col {
    display: flex;
    align-items: center;
    gap: 4px;
    cursor: pointer;
    user-select: none;
    min-width: 0;
    background: none;
    border: none;
    color: inherit;
    font: inherit;
    text-transform: inherit;
    letter-spacing: inherit;
    text-align: left;
    padding: 0;
  }

  .col:hover {
    color: var(--text-secondary);
  }

  .col-active {
    color: var(--accent-primary) !important;
  }

  .col.dragging {
    opacity: 0.5;
  }

  .drop-target {
    position: relative;
  }

  .drop-target::after {
    content: "";
    position: absolute;
    right: -2px;
    top: 4px;
    bottom: 4px;
    width: 2px;
    background: var(--accent-primary);
    border-radius: 1px;
  }

  .col-index {
    width: 36px;
    flex: none;
    text-align: center;
  }

  .col-duration {
    width: 70px;
    flex: none;
    text-align: right;
    padding-right: 4px;
  }

  /* ── Song List ───────────────────────────────────── */
  .song-list {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
  }

  .song-row {
    display: flex;
    align-items: center;
    width: 100%;
    height: 36px;
    padding: 0 12px 0 8px;
    background: none;
    border: none;
    color: var(--text-primary);
    font-size: 13px;
    cursor: pointer;
    transition: background 0.1s;
  }

  .song-row:hover {
    background: var(--bg-hover);
  }

  .song-row.playing {
    background: var(--accent-surface);
  }

  .song-row.playing .song-title-cell {
    color: var(--accent-primary);
  }

  .playing-indicator {
    color: var(--accent-primary);
    font-size: 10px;
  }

  .song-title-cell {
    font-weight: 500;
  }

  .tags-cell {
    font-size: 11px;
    color: var(--text-secondary);
  }

  .tags-btn {
    cursor: pointer;
    border-radius: 4px;
    transition: background 0.1s;
    padding: 2px 4px;
    margin: -2px -4px;
  }

  .tags-btn:hover {
    background: var(--accent-surface);
  }

  /* ── Empty State ─────────────────────────────────── */
  .empty-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 40px;
    color: var(--text-tertiary);
  }

  .empty-icon {
    opacity: 0.4;
    margin-bottom: 4px;
    display: flex;
  }

  .empty-title {
    font-size: 15px;
    font-weight: 500;
    color: var(--text-secondary);
  }

  .empty-desc {
    font-size: 12px;
    text-align: center;
    max-width: 240px;
    line-height: 1.5;
  }

  .retry-btn {
    margin-top: 8px;
    padding: 6px 14px;
    border-radius: 6px;
    background: var(--accent-primary);
    color: white;
    font-size: 12px;
    border: none;
  }

  .retry-btn:hover {
    background: var(--accent-hover);
  }
</style>
