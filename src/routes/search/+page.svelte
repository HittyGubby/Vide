<script lang="ts">
  import { invoke } from "$lib/transport";
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { Search, History, Play, Loader, X, Tag, Info, Download } from "lucide-svelte";
  import { player } from "$lib/stores/player.svelte";
  import { searchStore } from "$lib/stores/search.svelte";
  import { searchPage } from "$lib/stores/searchPage.svelte";
  import ContextMenu, { type MenuItem } from "$lib/components/song/ContextMenu.svelte";
  import TagEditor from "$lib/components/tag/TagEditor.svelte";
  import MultiTagEditor from "$lib/components/song/MultiTagEditor.svelte";
  import { isTauri } from "$lib/isTauri";
  import { downloadTarget as remoteDownloadTarget, isRemoteMode } from "$lib/transport";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import type { PlaybackStatus, PluginState, SearchResultItem, Song } from "$lib/types";

  let pluginState = $state<PluginState | null>(null);
  let editingSong = $state<Song | null>(null);
  let playingKey = $state<string | null>(null);
  let actionError = $state<string | null>(null);
  let menu = $state<{ x: number; y: number; item: SearchResultItem } | null>(null);
  let selectedKeys = $state<Set<string>>(new Set());
  let selectionAnchor = $state<number | null>(null);
  let bulkTagSongs = $state<Song[] | null>(null);

  function keyOf(item: SearchResultItem) {
    return item.platform + ":" + item.source_id;
  }
  function selectedItems(): SearchResultItem[] {
    return searchPage.results.filter((item) => selectedKeys.has(keyOf(item)));
  }
  function clearSearchSelection() {
    selectedKeys = new Set();
    selectionAnchor = null;
  }
  function onSearchRowClick(item: SearchResultItem, index: number, e: MouseEvent) {
    if (e.ctrlKey || e.metaKey) {
      const next = new Set(selectedKeys);
      if (next.has(keyOf(item))) next.delete(keyOf(item));
      else next.add(keyOf(item));
      selectedKeys = next;
      selectionAnchor = index;
      return;
    }
    if (e.shiftKey) {
      const anchor = selectionAnchor ?? index;
      const start = Math.min(anchor, index);
      const end = Math.max(anchor, index);
      const next = new Set(selectedKeys);
      for (let i = start; i <= end; i += 1) {
        const entry = searchPage.results[i];
        if (entry) next.add(keyOf(entry));
      }
      selectedKeys = next;
      selectionAnchor = index;
      return;
    }
    if (selectedKeys.size > 0) clearSearchSelection();
    void playResult(item);
  }
  function onSearchRowContext(item: SearchResultItem, index: number, e: MouseEvent) {
    if (!selectedKeys.has(keyOf(item))) {
      selectedKeys = new Set([keyOf(item)]);
      selectionAnchor = index;
    }
    openMenu(e, item);
  }
  async function resolveSelected(): Promise<Song[]> {
    const songs: Song[] = [];
    for (const item of selectedItems()) {
      try {
        songs.push(await invoke<Song>("resolve_song", { platform: item.platform, sourceId: item.source_id }));
      } catch {
        /* skip unresolvable */
      }
    }
    return songs;
  }
  async function bulkSearchTag() {
    const songs = await resolveSelected();
    menu = null;
    if (songs.length > 0) bulkTagSongs = songs;
  }
  async function bulkSearchDownload() {
    const songs = await resolveSelected();
    menu = null;
    if (songs.length === 0) return;
    if (!isTauri) {
      for (const song of songs) {
        const target = remoteDownloadTarget(song.id);
        if (!target) continue;
        const link = document.createElement("a");
        link.href = target.url;
        link.download = "";
        link.click();
      }
      clearSearchSelection();
      return;
    }
    const directory = await open({ directory: true, multiple: false });
    if (!directory || Array.isArray(directory)) return;
    for (const song of songs) {
      const destination = directory + "/" + song.title.replace(/[\\/:*?"<>|]/g, "_") + ".mp3";
      if (isRemoteMode()) {
        const target = remoteDownloadTarget(song.id);
        if (!target) continue;
        await invoke("download_remote_to_path", { url: target.url, headers: target.headers, path: destination });
      } else {
        await invoke("download_song_to_path", { id: song.id, path: destination });
      }
    }
    clearSearchSelection();
  }

  const platformLabels: Record<string, string> = {
    netease: "网易云",
    tencent: "QQ音乐",
    kugou: "酷狗",
    kuwo: "酷我",
    bilibili: "Bilibili",
    youtube: "YouTube",
    soundcloud: "SoundCloud",
  };

  function platformLabel(id: string): string {
    return platformLabels[id] ?? id;
  }

  function platformsFrom(state: PluginState | null): { id: string; label: string }[] {
    const ids = new Set<string>();
    for (const route of state?.routes ?? []) {
      if (route.capability !== "search") continue;
      if (route.providers.some((provider) => provider.enabled)) {
        ids.add(route.platform);
      }
    }
    return [...ids].sort().map((id) => ({ id, label: platformLabel(id) }));
  }

  let platforms = $derived(platformsFrom(pluginState));
  let pageResults = $derived(
    searchPage.results.slice(
      (searchPage.page - 1) * searchPage.pageSize,
      searchPage.page * searchPage.pageSize,
    ),
  );
  let totalPages = $derived(
    Math.max(1, Math.ceil(searchPage.results.length / searchPage.pageSize)),
  );

  function isHttpUrl(value: string): boolean {
    try {
      const url = new URL(value);
      return url.protocol === "http:" || url.protocol === "https:";
    } catch {
      return false;
    }
  }

  async function loadPlatforms() {
    try {
      pluginState = await invoke<PluginState>("get_plugin_state");
    } catch (e) {
      console.error("load plugin platforms error:", e);
      pluginState = null;
    }
    const list = platformsFrom(pluginState);
    if (list.length > 0 && !list.some((item) => item.id === searchPage.activePlatform)) {
      searchPage.activePlatform = list[0].id;
    }
  }

  onMount(async () => {
    await loadPlatforms();
    const q = $page.url.searchParams.get("q");
    if (q && q !== searchPage.query) {
      searchPage.query = q;
      await doSearch();
    } else if (q && !searchPage.searched) {
      await doSearch();
    }
  });

  async function doSearch() {
    const q = searchPage.query.trim();
    if (!q) return;
    void searchStore.add(q);
    searchPage.loading = true;
    searchPage.error = null;
    actionError = null;
    searchPage.results = [];
    searchPage.searched = true;
    searchPage.page = 1;
    searchPage.urlMode = isHttpUrl(q);
    try {
      if (searchPage.urlMode) {
        searchPage.results = await invoke<SearchResultItem[]>("parse_online_url", { url: q });
        if (searchPage.results.length > 0) {
          const platform = searchPage.results[0].platform;
          if (platforms.some((item) => item.id === platform)) {
            searchPage.activePlatform = platform;
          }
        }
      } else {
        if (!searchPage.activePlatform) {
          await loadPlatforms();
        }
        if (!searchPage.activePlatform) {
          throw new Error("在线平台未配置，请先在设置中安装并启用插件");
        }
        searchPage.results = await invoke<SearchResultItem[]>("search_online", {
          query: q,
          platform: searchPage.activePlatform,
          limit: 60,
        });
      }
      if (searchPage.results.length === 0) {
        searchPage.error = searchPage.urlMode
          ? "无法识别该链接，或它不是受支持的歌曲/歌单"
          : "无结果";
      }
    } catch (e) {
      searchPage.error = String(e);
      searchPage.results = [];
    } finally {
      searchPage.loading = false;
    }
  }

  function selectPlatform(id: string) {
    searchPage.activePlatform = id;
    if (searchPage.searched && !searchPage.urlMode) doSearch();
  }

  async function playResult(item: SearchResultItem) {
    // A song URL plays just that song. A playlist URL queues the whole parsed
    // playlist. A normal keyword search result is a single-song queue.
    const queue = searchPage.urlMode ? searchPage.results : [item];
    const index = Math.max(
      0,
      queue.findIndex(
        (entry) => entry.platform === item.platform && entry.source_id === item.source_id,
      ),
    );
    try {
      const status = await invoke<PlaybackStatus>("play_online_results", {
        results: queue,
        index,
      });
      player.mediaId = status.media_id ?? null;
      player.title = item.title;
      player.artist = item.artist || "Unknown";
      player.album = item.album || "";
      player.cover = item.cover_url ?? null;
      player.isPlaying = true;
      playingKey = item.platform + ":" + item.source_id;
    } catch (e) {
      console.error("play error:", e);
      actionError = "播放失败：" + String(e);
    }
  }

  async function editTags(item: SearchResultItem) {
    try {
      editingSong = await invoke<Song>("resolve_song", {
        platform: item.platform,
        sourceId: item.source_id,
      });
    } catch (e) {
      actionError = "读取歌曲信息失败：" + String(e);
    }
  }

  async function openDownload(item: SearchResultItem) {
    try {
      const song = await invoke<Song>("resolve_song", {
        platform: item.platform,
        sourceId: item.source_id,
      });
      if (!isTauri) {
        const target = remoteDownloadTarget(song.id);
        if (!target) throw new Error("download endpoint unavailable");
        const link = document.createElement("a");
        link.href = target.url;
        link.download = "";
        link.click();
        return;
      }
      const suggested = song.title.replace(/[\\/:*?"<>|]/g, "_") + ".mp3";
      const path = await save({ defaultPath: suggested });
      if (!path) return;
      if (isRemoteMode()) {
        const target = remoteDownloadTarget(song.id);
        if (!target) throw new Error("download endpoint unavailable");
        await invoke("download_remote_to_path", { url: target.url, headers: target.headers, path });
      } else {
        await invoke("download_song_to_path", { id: song.id, path });
      }
    } catch (e) {
      actionError = "下载失败：" + String(e);
    }
  }

  function detailUrl(item: SearchResultItem): string | null {
    const id = item.source_id;
    switch (item.platform) {
      case "youtube":
        return "https://www.youtube.com/watch?v=" + id;
      case "bilibili":
        return "https://www.bilibili.com/video/" + id;
      case "soundcloud":
        return id;
      case "netease":
        return "https://music.163.com/#/song?id=" + id;
      case "kugou":
        return "https://www.kugou.com/song/#hash=" + id;
      case "kuwo":
        return "https://www.kuwo.cn/play_detail/" + id;
      case "tencent":
        return "https://y.qq.com/n/ryqq/songDetail/" + id;
      default:
        return null;
    }
  }

  async function showDetails(item: SearchResultItem) {
    const url = detailUrl(item);
    if (!url) {
      actionError = "该平台没有可打开的详情页";
      return;
    }
    try {
      await openUrl(url);
    } catch (e) {
      actionError = "无法打开浏览器：" + String(e);
    }
  }

  async function proxyCover(event: Event, item: SearchResultItem) {
    const img = event.currentTarget as HTMLImageElement;
    if (img.dataset.proxied) {
      img.style.display = "none";
      return;
    }
    img.dataset.proxied = "1";
    try {
      const referer =
        item.platform === "bilibili" ? "https://www.bilibili.com/" : null;
      img.src = await invoke<string>("fetch_remote_image", {
        url: item.cover_url,
        referer,
      });
    } catch {
      img.style.display = "none";
    }
  }

  function openMenu(event: MouseEvent, item: SearchResultItem) {
    event.preventDefault();
    event.stopPropagation();
    menu = { x: event.clientX, y: event.clientY, item };
  }

  function menuItems(item: SearchResultItem): MenuItem[] {
    if (selectedKeys.size > 1 && selectedKeys.has(keyOf(item))) {
      return [
        { label: "下载 " + selectedKeys.size + " 首", icon: Download, action: () => void bulkSearchDownload() },
        { label: "编辑标签", icon: Tag, action: () => void bulkSearchTag() },
        { label: "取消多选", icon: X, action: clearSearchSelection },
      ];
    }
    return [
      { label: "播放", icon: Play, action: () => playResult(item) },
      { label: "编辑标签", icon: Tag, action: () => editTags(item) },
      { label: "详情", icon: Info, action: () => showDetails(item) },
      { label: "下载", icon: Download, action: () => openDownload(item) },
    ];
  }

  function useHistory(value: string) {
    searchPage.query = value;
    doSearch();
  }

  function removeHistory(value: string) {
    searchStore.remove(value);
  }

  function keydown(e: KeyboardEvent) {
    if (e.key === "Enter") doSearch();
  }

  function goPage(next: number) {
    searchPage.page = Math.max(1, Math.min(totalPages, next));
  }

  function formatDuration(seconds?: number): string {
    if (!seconds || seconds <= 0) return "";
    const m = Math.floor(seconds / 60);
    const s = Math.floor(seconds % 60);
    return m + ":" + s.toString().padStart(2, "0");
  }

  $effect(() => {
    const q = $page.url.searchParams.get("q");
    if (q !== null && q !== searchPage.query) {
      searchPage.query = q;
    }
  });
</script>

<div class="search-view">
  <div class="inline-search">
    <div class="inline-input">
      <Search size={14} class="inline-icon" />
      <input
        type="text"
        placeholder="搜索歌曲或粘贴链接..."
        bind:value={searchPage.query}
        onkeydown={keydown}
      />
      {#if searchPage.loading}
        <Loader size={14} class="spin" />
      {/if}
    </div>
    <button class="search-btn" onclick={doSearch} disabled={!searchPage.query.trim()}>搜索</button>
  </div>

  <div class="source-tabs">
    {#each platforms as p}
      <button
        class="tab"
        class:active={searchPage.activePlatform === p.id}
        onclick={() => selectPlatform(p.id)}
      >
        {p.label}
      </button>
    {/each}
  </div>

  {#if actionError}
    <div class="action-error">{actionError}</div>
  {/if}

  {#if !searchPage.searched}
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
  {:else if searchPage.loading}
    <div class="state"><Loader size={24} class="spin" /> <p>搜索中...</p></div>
  {:else if searchPage.error}
    <div class="state">
      <Search size={28} class="empty-icon" />
      <p>{searchPage.error}</p>
    </div>
  {:else if searchPage.results.length > 0}
    <div class="results">
      <div class="result-header">
        <div class="res-index">#</div>
        <div class="res-main">标题</div>
        <div class="res-artist">作者</div>
        <div class="res-album">专辑</div>
        <div class="res-duration">时长</div>
        <div class="res-action"></div>
      </div>
      {#each pageResults as item, i}
        <div
          class="result-row"
          class:playing={playingKey === item.platform + ":" + item.source_id}
          role="button"
          tabindex="0"
          onclick={(e) => onSearchRowClick(item, i, e)}
          onkeydown={(e) => { if (e.key === "Enter") playResult(item); }}
          oncontextmenu={(e) => onSearchRowContext(item, i, e)}
        >
          <div class="res-index">
            {#if selectedKeys.size > 0}
              <input
                type="checkbox"
                class="row-check"
                checked={selectedKeys.has(keyOf(item))}
                onclick={(e) => e.stopPropagation()}
                onchange={(e) => {
                  const next = new Set(selectedKeys);
                  if (e.currentTarget.checked) next.add(keyOf(item));
                  else next.delete(keyOf(item));
                  selectedKeys = next;
                  selectionAnchor = i;
                }}
              />
            {:else if item.cover_url}
              <img class="res-cover" src={item.cover_url} alt="" loading="lazy" referrerpolicy="no-referrer" onerror={(e) => proxyCover(e, item)} />
            {:else}
              {(searchPage.page - 1) * searchPage.pageSize + i + 1}
            {/if}
          </div>
          <div class="res-main">
            <span class="res-title truncate">{item.title}</span>
            <span class="res-platform">{platformLabel(item.platform)}</span>
          </div>
          <div class="res-artist truncate">{item.artist || ""}</div>
          <div class="res-album truncate">{item.album || ""}</div>
          <div class="res-duration">{item.duration ? formatDuration(item.duration) : ""}</div>
          <button class="row-btn" onclick={(e) => { e.stopPropagation(); playResult(item); }} title="播放">
            <Play size={14} />
          </button>
        </div>
      {/each}
      {#if totalPages > 1}
        <div class="pager">
          <button class="page-btn" disabled={searchPage.page <= 1} onclick={() => goPage(searchPage.page - 1)}>上一页</button>
          <span class="page-label">第 {searchPage.page} / {totalPages} 页</span>
          <button class="page-btn" disabled={searchPage.page >= totalPages} onclick={() => goPage(searchPage.page + 1)}>下一页</button>
        </div>
      {/if}
    </div>
  {:else}
    <div class="state">
      <Search size={28} class="empty-icon" />
      <p>输入关键词或粘贴歌曲/歌单链接</p>
    </div>
  {/if}
</div>

{#if menu}
  <ContextMenu items={menuItems(menu.item)} x={menu.x} y={menu.y} onclose={() => (menu = null)} />
{/if}
{#if editingSong}
  <TagEditor song={editingSong} onclose={() => (editingSong = null)} />
{/if}
{#if bulkTagSongs}
  <MultiTagEditor
    songs={bulkTagSongs}
    onclose={() => {
      bulkTagSongs = null;
      clearSearchSelection();
    }}
    onsaved={() => {
      bulkTagSongs = null;
      clearSearchSelection();
    }}
  />
{/if}

<style>
  .search-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: 14px 24px;
    overflow-y: auto;
    user-select: none;
    gap: 10px;
  }

  .source-tabs {
    display: flex;
    gap: 2px;
    flex-shrink: 0;
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

  .action-error {
    padding: 6px 10px;
    border-radius: 6px;
    background: var(--color-error) / 0.1;
    border: 1px solid var(--color-error) / 0.25;
    color: var(--color-error);
    font-size: 11px;
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

  .results {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .result-header {
    display: flex;
    align-items: center;
    height: 28px;
    padding: 0 8px;
    border-bottom: 1px solid var(--color-base-300);
    color: var(--color-base-content) / 0.5;
    font-size: 11px;
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

  .result-row.playing {
    background: var(--color-primary) / 0.12;
  }

  .result-row.playing .res-title {
    color: var(--color-primary);
  }

  .res-index {
    width: 34px;
    color: var(--color-base-content) / 0.5;
    font-size: 12px;
    text-align: center;
    flex: none;
  }

  .row-check { width: 13px; height: 13px; accent-color: var(--color-primary); cursor: pointer; }

  .res-cover {
    display: block;
    width: 30px;
    height: 30px;
    border-radius: 4px;
    object-fit: cover;
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

  .res-album {
    flex: 1.5;
    font-size: 13px;
    color: var(--color-base-content) / 0.6;
  }

  .res-duration {
    width: 70px;
    text-align: right;
    font-size: 12px;
    color: var(--color-base-content) / 0.6;
  }

  .res-action {
    width: 30px;
  }

  .pager {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
    padding: 12px 0 4px;
    margin-top: auto;
  }

  .page-label {
    font-size: 12px;
    color: var(--color-base-content) / 0.6;
  }

  .page-btn {
    padding: 4px 10px;
    border: 1px solid var(--color-base-300);
    border-radius: 5px;
    background: var(--color-base-200);
    color: var(--color-base-content) / 0.8;
    font-size: 12px;
  }

  .page-btn:hover:not(:disabled) {
    border-color: var(--color-primary);
    color: var(--color-primary);
  }

  .page-btn:disabled {
    opacity: 0.35;
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
</style>
