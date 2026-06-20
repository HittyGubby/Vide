<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { Search, Tag, Play } from "lucide-svelte";
  import { player } from "$lib/stores/player.svelte";
  import type { Song } from "$lib/types";

  const columns = [
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
</script>

<div class="library-view">
  <!-- Local search bar -->
  <div class="search-bar">
    <div class="search-input-wrapper">
      <span class="search-icon">
        <Search size={14} />
      </span>
      <input
        type="text"
        class="local-search"
        placeholder="搜索我的音乐... (留空 = 我的库)"
        bind:value={query}
      />
    </div>
    {#if filteredSongs.length > 0}
      <span class="song-count">{filteredSongs.length}</span>
    {/if}
  </div>

  <!-- Song table header -->
  <div class="table-header" role="row">
    <div class="col col-index">#</div>
    {#each columns as col}
      <button
        class="col"
        style="flex: {col.width}"
        onclick={() => toggleSort(col.id)}
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
        >
          <div class="col col-index">
            {#if isPlaying(song)}
              <span class="playing-indicator">▶</span>
            {:else}
              {i + 1}
            {/if}
          </div>
          <div class="col" style="flex: 2fr">
            <span class="truncate song-title-cell">{song.title}</span>
          </div>
          <div class="col" style="flex: 1.5fr">
            <span class="truncate">{song.artist}</span>
          </div>
          <div class="col" style="flex: 1.5fr">
            <span class="truncate">{song.album}</span>
          </div>
          <div class="col" style="flex: 1.5fr">
            <span class="truncate tags-cell">{displayTags(song)}</span>
          </div>
          <div class="col col-duration">
            {formatDuration(song.metadata.duration)}
          </div>
        </button>
      {/each}
    </div>
  {/if}
</div>

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
