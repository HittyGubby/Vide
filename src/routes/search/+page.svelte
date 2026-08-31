<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { Search, History, Play, Loader, X, Download } from "lucide-svelte";
  import { player } from "$lib/stores/player.svelte";
  import { searchStore } from "$lib/stores/search.svelte";
  import type { SearchResultItem } from "$lib/types";

  let query = $state("");
  let activePlatform = $state<string | null>("youtube");
  let searchType = $state<"music" | "songlist">("music");
  let results = $state<SearchResultItem[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let searched = $state(false);

  // Platforms available for search. When the backend has actual platforms,
  // this list is derived from what the plugin system supports. Until then,
  // these mirror the LX Music screenshot.
  const platforms = [
    { id: "youtube", label: "YouTube" },
    { id: "bilibili", label: "Bilibili" },
    { id: "kugou", label: "Kugou" },
    { id: "kuwo", label: "Kuwo" },
    { id: "netease", label: "NetEase" },
    { id: "migu", label: "Migu" },
    { id: "tencent", label: "Tencent" },
  ];

  onMount(() => {
    const q = $page.url.searchParams.get("q");
    if (q) {
      query = q;
      doSearch();
    }
  });

  async function doSearch() {
    const q = query.trim();
    if (!q) return;
    loading = true;
    error = null;
    results = [];
    searched = true;
    try {
      results = await invoke<SearchResultItem[]>("search_online", {
        query: q,
        platform: activePlatform,
      });
      if (results.length === 0) {
        error = "无结果（需要配置在线搜索插件）";
      }
    } catch (e) {
      error = String(e);
      results = [];
    } finally {
      loading = false;
    }
  }

  function selectPlatform(id: string | null) {
    activePlatform = id;
    if (searched) doSearch();
  }

  function selectType(t: "music" | "songlist") {
    searchType = t;
    // Re-run the current query against the newly selected result type.
    if (searched && query.trim()) doSearch();
  }

  async function playResult(item: SearchResultItem) {
    try {
      const song = await invoke<{ id: string; title: string; artist: string }>("resolve_song", {
        platform: item.platform,
        id: item.id,
      });
      await invoke("play_song", { id: song.id });
      player.mediaId = song.id;
      player.title = song.title;
      player.artist = song.artist || "Unknown";
      player.isPlaying = true;
    } catch (e) {
      console.error("play error:", e);
    }
  }

  function useHistory(h: string) {
    query = h;
    doSearch();
  }

  // Build a plausible playlist URL for an online search result so it can be
  // imported via the plugin (works for single-video / playlist ids on supported
  // platforms like YouTube & Bilibili).
  function playlistUrlOf(item: SearchResultItem): string {
    const p = item.platform.toLowerCase();
    if (p.includes("youtube") || p === "yt") {
      return "https://www.youtube.com/playlist?list=" + item.id;
    }
    if (p.includes("bilibili") || p === "bili") {
      return "https://www.bilibili.com/video/" + item.id;
    }
    return "";
  }

  // Songlist mode: import the result (a playlist) into the library.
  let importingId = $state<string | null>(null);
  async function importResult(item: SearchResultItem) {
    const url = playlistUrlOf(item);
    if (!url) {
      error = "暂不支持导入该平台歌单";
      return;
    }
    importingId = item.id;
    try {
      await invoke("import_playlist_url", { url });
      error = "已导入歌单";
    } catch (e) {
      console.error("import playlist error:", e);
      error = String(e);
    } finally {
      importingId = null;
    }
  }

  function removeHistory(h: string) {
    searchStore.remove(h);
  }

  function keydown(e: KeyboardEvent) {
    if (e.key === "Enter") doSearch();
  }

  function formatDuration(seconds?: number): string {
    if (!seconds || seconds <= 0) return "";
    const m = Math.floor(seconds / 60);
    const s = Math.floor(seconds % 60);
    return m + ":" + s.toString().padStart(2, "0");
  }

  $effect(() => {
    const q = $page.url.searchParams.get("q");
    if (q !== null && q !== query) {
      query = q;
    }
  });
</script>

<div class="search-view">
  <div class="search-header">
    <div class="source-tabs">
      {#each platforms as p}
        <button
          class="tab"
          class:active={activePlatform === p.id}
          onclick={() => selectPlatform(p.id)}
        >
          {p.label}
        </button>
      {/each}
    </div>

    <div class="type-tabs">
      <button class="ttab" class:active={searchType === "music"} onclick={() => selectType("music")}>
        Song
      </button>
      <button class="ttab" class:active={searchType === "songlist"} onclick={() => selectType("songlist")}>
        Playlist
      </button>
    </div>
  </div>

  <!-- Search input (inline) -->
  <div class="inline-search">
    <div class="inline-input">
      <Search size={14} class="inline-icon" />
      <input
        type="text"
        placeholder="搜索..."
        bind:value={query}
        onkeydown={keydown}
        oninput={() => (searched = false)}
      />
      {#if loading}
        <Loader size={14} class="spin" />
      {/if}
    </div>
    <button class="search-btn" onclick={doSearch} disabled={!query.trim()}>搜索</button>
  </div>

  {#if !searched && searchType === "music"}
    <!-- Search history chips -->
    <div class="history-section">
      <div class="history-title">
        <History size={14} />
        <span>Search History</span>
        {#if searchStore.history.length > 0}
          <button class="clear-all" onclick={() => searchStore.clear()}>清除</button>
        {/if}
      </div>
      <div class="chips">
        {#each searchStore.history as h}
          <span class="chip-group">
            <button class="chip" onclick={() => useHistory(h)}>{h}</button>
            <button class="chip-remove" onclick={() => removeHistory(h)} title="移除">
              <X size={10} />
            </button>
          </span>
        {/each}
      </div>
    </div>
  {:else if loading}
    <div class="state"><Loader size={24} class="spin" /> <p>搜索中...</p></div>
  {:else if error}
    <div class="state">
      <Search size={28} class="empty-icon" />
      <p>{error}</p>
    </div>
  {:else if results.length > 0}
    <div class="results">
      {#each results as item, i}
        <div class="result-row" role="button" tabindex="0" onclick={() => (searchType === "songlist" ? importResult(item) : playResult(item))} onkeydown={(e) => { if (e.key === "Enter") (searchType === "songlist" ? importResult(item) : playResult(item)); }}>
          <div class="res-index">{i + 1}</div>
          <div class="res-main">
            <span class="res-title truncate">{item.title}</span>
            <span class="res-platform">{item.platform}</span>
          </div>
          <div class="res-artist truncate">{item.artist || ""}</div>
          <div class="res-duration">{item.duration ? formatDuration(item.duration) : ""}</div>
          <button class="row-btn" onclick={(e) => { e.stopPropagation(); searchType === "songlist" ? importResult(item) : playResult(item); }}>
            {#if importingId === item.id}
              <Loader size={14} class="spin" />
            {:else if searchType === "songlist"}
              <Download size={14} />
            {:else}
              <Play size={14} />
            {/if}
          </button>
        </div>
      {/each}
    </div>
  {:else}
    <div class="state">
      <Search size={28} class="empty-icon" />
      <p>输入关键词开始搜索</p>
    </div>
  {/if}
</div>

<style>
  .search-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: 14px 24px;
    overflow-y: auto;
    user-select: none;
    gap: 16px;
  }

  .search-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-shrink: 0;
  }

  .source-tabs {
    display: flex;
    gap: 2px;
  }

  .tab {
    padding: 6px 10px;
    font-size: 13px;
    color: var(--color-base-content) / 0.65;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
  }

  .tab:hover {
    color: var(--color-base-content);
  }

  .tab.active {
    color: var(--color-base-content);
    border-bottom-color: var(--color-base-content);
  }

  .type-tabs {
    display: flex;
    gap: 2px;
  }

  .ttab {
    padding: 6px 10px;
    font-size: 13px;
    color: var(--color-base-content) / 0.65;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
  }

  .ttab.active {
    color: var(--color-base-content);
    border-bottom-color: var(--color-base-content);
  }

  .inline-search {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-shrink: 0;
  }

  .inline-input {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 12px;
    background: var(--color-base-200);
    border: 1px solid var(--color-base-300);
    border-radius: 6px;
  }

  .inline-input:focus-within {
    border-color: var(--color-primary);
  }

  .inline-icon {
    color: var(--color-base-content) / 0.5;
  }

  .inline-input input {
    flex: 1;
    background: none;
    border: none;
    color: var(--color-base-content);
    font-size: 13px;
    outline: none;
    user-select: text;
  }

  .search-btn {
    padding: 8px 18px;
    border-radius: 6px;
    background: var(--color-primary);
    color: var(--color-primary-content);
    font-size: 13px;
    border: none;
  }

  .search-btn:disabled {
    opacity: 0.5;
  }

  .history-section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .history-title {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 14px;
    font-weight: 600;
    color: var(--color-base-content);
  }

  .clear-all {
    margin-left: auto;
    font-size: 11px;
    color: var(--color-base-content) / 0.5;
    background: none;
    border: none;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .chip-group {
    display: inline-flex;
    align-items: center;
    background: var(--color-base-200);
    border: 1px solid var(--color-base-300);
    border-radius: 6px;
    overflow: hidden;
  }

  .chip {
    padding: 7px 12px;
    font-size: 12.5px;
    color: var(--color-base-content) / 0.9;
    background: none;
    border: none;
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chip:hover {
    background: var(--color-base-300);
  }

  .chip-remove {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    color: var(--color-base-content) / 0.5;
    background: none;
    border: none;
  }

  .chip-remove:hover {
    color: var(--color-base-content);
  }

  .results {
    display: flex;
    flex-direction: column;
  }

  .result-row {
    display: flex;
    align-items: center;
    height: 40px;
    padding: 0 8px;
    cursor: pointer;
    border-radius: 4px;
    transition: background 0.1s;
  }

  .result-row:hover {
    background: var(--color-base-300) / 0.7;
  }

  .res-index {
    width: 34px;
    color: var(--color-base-content) / 0.5;
    font-size: 12px;
    text-align: center;
  }

  .res-main {
    flex: 2;
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .res-title {
    font-size: 13px;
    color: var(--color-base-content);
  }

  .res-platform {
    flex: none;
    font-size: 10px;
    padding: 1px 5px;
    border-radius: 4px;
    background: var(--color-primary) / 0.15;
    color: var(--color-primary);
  }

  .res-artist {
    flex: 1.5;
    font-size: 13px;
    color: var(--color-base-content) / 0.7;
  }

  .res-duration {
    width: 70px;
    text-align: right;
    font-size: 12px;
    color: var(--color-base-content) / 0.6;
  }

  .row-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border-radius: 5px;
    background: none;
    border: none;
    color: var(--color-base-content) / 0.6;
  }

  .row-btn:hover {
    background: var(--color-base-300);
    color: var(--color-base-content);
  }

  .state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: var(--color-base-content) / 0.55;
  }

  .empty-icon {
    opacity: 0.4;
  }

  .spin {
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
