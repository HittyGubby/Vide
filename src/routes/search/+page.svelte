<script lang="ts">
  import { invoke } from "$lib/transport";
  import { onMount, untrack } from "svelte";
  import { page } from "$app/stores";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { Search, History, Play, Loader, X, Tag as TagIcon, Info, Download } from "lucide-svelte";
  import { player } from "$lib/stores/player.svelte";
  import { searchStore } from "$lib/stores/search.svelte";
  import { searchPage } from "$lib/stores/searchPage.svelte";
  import { loadClientState, saveClientState } from "$lib/clientState";
  import {
    defaultColumnPrefs,
    normalizeColumnPrefs,
    visibleColumns,
    type TableColumnDef,
    type TableColumnPrefs,
  } from "$lib/table/columns";
  import ContextMenu, { type MenuItem } from "$lib/components/song/ContextMenu.svelte";
  import TagEditor from "$lib/components/tag/TagEditor.svelte";
  import MultiTagEditor from "$lib/components/song/MultiTagEditor.svelte";
  import { isTauri } from "$lib/isTauri";
  import { downloadTarget as remoteDownloadTarget, getPlaybackTarget, isRemoteMode } from "$lib/transport";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { downloadManyWeb } from "$lib/download";
  import type { PlaybackStatus, PluginState, SearchResultItem, Song, Tag as SongTag } from "$lib/types";

  let pluginState = $state<PluginState | null>(null);
  let editingSong = $state<Song | null>(null);
  let playingKey = $state<string | null>(null);
  let actionError = $state<string | null>(null);
  let menu = $state<{ x: number; y: number; item: SearchResultItem } | null>(null);
  let selectedKeys = $state<Set<string>>(new Set());
  let selectionAnchor = $state<number | null>(null);
  let bulkTagSongs = $state<Song[] | null>(null);
  let tagsByKey = $state<Record<string, SongTag[]>>({});
  let editingItemKey = $state<string | null>(null);

  const SEARCH_COLUMNS_KEY = "search.columns.v2";
  const SEARCH_COLUMNS: TableColumnDef[] = [
    { id: "index", label: "#", defWidth: 40, min: 40, fixed: true },
    { id: "title", label: "名称", defWidth: 300, min: 120 },
    { id: "tag", label: "标签", defWidth: 160, min: 80 },
    { id: "artist", label: "歌手", defWidth: 130, min: 80 },
    { id: "album", label: "专辑", defWidth: 140, min: 80 },
    { id: "duration", label: "时长", defWidth: 52, min: 52, fixed: true },
  ];
  let columnPrefs = $state<TableColumnPrefs>(defaultColumnPrefs(SEARCH_COLUMNS));
  let columnMenu = $state<{ x: number; y: number } | null>(null);
  let columnMenuItems = $state<MenuItem[]>([]);
  let columnSaveTimer: ReturnType<typeof setTimeout> | undefined;
  let columns = $derived(visibleColumns(SEARCH_COLUMNS, columnPrefs));
  let resultsEl = $state<HTMLDivElement | null>(null);
  let dragColumn = $state<string | null>(null);

  function fitSearchToWidth(width = resultsEl?.clientWidth ?? 0) {
    if (width <= 0) return;
    const visible = columns;
    const fixedTotal = visible
      .filter((column) => column.fixed)
      .reduce((sum, column) => sum + columnWidth(column.id), 0);
    const resizable = visible.filter((column) => !column.fixed);
    const target = width - fixedTotal;
    const total = resizable.reduce((sum, column) => sum + columnWidth(column.id), 0);
    if (resizable.length === 0 || target <= 0 || total <= 0) return;
    const scale = target / total;
    if (Math.abs(scale - 1) < 0.01) return;
    const sizing = { ...columnPrefs.sizing };
    for (const column of resizable) {
      sizing[column.id] = Math.max(column.min, Math.round(columnWidth(column.id) * scale));
    }
    columnPrefs = { ...columnPrefs, sizing };
    saveColumns();
  }

  function startColumnDrag(e: DragEvent, id: string) {
    dragColumn = id;
    e.dataTransfer?.setData("text/plain", id);
    if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
  }

  function dragColumnOver(e: DragEvent, id: string) {
    e.preventDefault();
    if (dragColumn && dragColumn !== id) {
      (e.currentTarget as HTMLElement).classList.add("drop-target");
    }
  }

  function dragColumnLeave(e: DragEvent) {
    (e.currentTarget as HTMLElement).classList.remove("drop-target");
  }

  function dropColumn(e: DragEvent, targetId: string) {
    e.preventDefault();
    (e.currentTarget as HTMLElement).classList.remove("drop-target");
    const sourceId = dragColumn;
    dragColumn = null;
    if (!sourceId || sourceId === targetId) return;
    const from = columnPrefs.order.indexOf(sourceId);
    const to = columnPrefs.order.indexOf(targetId);
    if (from < 0 || to < 0) return;
    const order = [...columnPrefs.order];
    order.splice(to, 0, ...order.splice(from, 1));
    columnPrefs = { ...columnPrefs, order };
    saveColumns();
  }

  function endColumnDrag() {
    dragColumn = null;
  }

  function saveColumns() {
    if (columnSaveTimer) clearTimeout(columnSaveTimer);
    columnSaveTimer = setTimeout(() => {
      columnSaveTimer = undefined;
      void saveClientState(SEARCH_COLUMNS_KEY, $state.snapshot(columnPrefs));
    }, 250);
  }

  function columnWidth(id: string): number {
    const def = SEARCH_COLUMNS.find((column) => column.id === id)!;
    return Math.max(columnPrefs.sizing[id] ?? def.defWidth, def.min);
  }

  function columnStyle(id: string): string {
    return "flex:0 0 " + columnWidth(id) + "px;width:" + columnWidth(id) + "px;min-width:0;";
  }

  function openColumnMenu(e: MouseEvent) {
    e.preventDefault();
    const visibleCount = columns.length;
    columnMenuItems = SEARCH_COLUMNS.map((def) => {
      const shown = columnPrefs.visibility[def.id] !== false;
      return {
        label: (shown ? "✓ " : "   ") + def.label,
        disabled: shown && visibleCount <= 1,
        action: () => {
          if (shown && visibleCount <= 1) return;
          columnPrefs = {
            ...columnPrefs,
            visibility: { ...columnPrefs.visibility, [def.id]: !shown },
          };
          saveColumns();
        },
      };
    });
    columnMenu = { x: e.clientX, y: e.clientY };
  }

  $effect(() => {
    const el = resultsEl;
    if (!el) return;
    const run = () => untrack(() => fitSearchToWidth(el.clientWidth));
    const observer = new ResizeObserver(run);
    observer.observe(el);
    run();
    return () => observer.disconnect();
  });

  async function loadSearchColumns() {
    const stored = await loadClientState<unknown>(SEARCH_COLUMNS_KEY);
    columnPrefs = normalizeColumnPrefs(SEARCH_COLUMNS, stored);
  }

  function startColumnResize(e: PointerEvent, def: TableColumnDef) {
    e.preventDefault();
    e.stopPropagation();
    const startX = e.clientX;
    const startWidth = columnWidth(def.id);
    function move(event: PointerEvent) {
      const width = Math.max(def.min, startWidth + event.clientX - startX);
      columnPrefs = {
        ...columnPrefs,
        sizing: { ...columnPrefs.sizing, [def.id]: width },
      };
    }
    function up() {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      saveColumns();
    }
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

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
      await downloadManyWeb(songs.map((song) => ({ id: song.id, title: song.title })));
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

  onMount(() => {
    const handler = () => void doSearch();
    window.addEventListener("vide:online-search", handler);
    return () => window.removeEventListener("vide:online-search", handler);
  });

  onMount(async () => {
    void loadSearchColumns();
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
      const clientPlayback = !isTauri || (isRemoteMode() && getPlaybackTarget() === "client");
      if (clientPlayback) {
        const songs: Song[] = [];
        for (const entry of queue) {
          songs.push(
            await invoke<Song>("resolve_song", {
              platform: entry.platform,
              sourceId: entry.source_id,
            }),
          );
        }
        await player.playFromList(
          songs.map((song) => ({
            id: song.id,
            title: song.title,
            artist: song.artist,
            album: song.album,
          })),
          songs[index].id,
        );
      } else {
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
      }
      playingKey = item.platform + ":" + item.source_id;
    } catch (e) {
      console.error("play error:", e);
      actionError = "播放失败：" + String(e);
    }
  }

  async function editTags(item: SearchResultItem) {
    try {
      const song = await invoke<Song>("resolve_song", {
        platform: item.platform,
        sourceId: item.source_id,
      });
      editingSong = song;
      editingItemKey = keyOf(item);
      tagsByKey = { ...tagsByKey, [keyOf(item)]: song.tags };
    } catch (e) {
      actionError = "读取歌曲信息失败：" + String(e);
    }
  }

  async function closeTagEditor() {
    editingSong = null;
    const key = editingItemKey;
    editingItemKey = null;
    if (!key) return;
    const item = searchPage.results.find((entry) => keyOf(entry) === key);
    if (!item) return;
    try {
      const song = await invoke<Song>("resolve_song", {
        platform: item.platform,
        sourceId: item.source_id,
      });
      tagsByKey = { ...tagsByKey, [key]: song.tags };
    } catch {
      /* keep previous tags */
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
        { label: "编辑标签", icon: TagIcon, action: () => void bulkSearchTag() },
        { label: "取消多选", icon: X, action: clearSearchSelection },
      ];
    }
    return [
      { label: "播放", icon: Play, action: () => playResult(item) },
      { label: "编辑标签", icon: TagIcon, action: () => editTags(item) },
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
    <div class="results" bind:this={resultsEl}>
      <div class="table-header" oncontextmenu={openColumnMenu}>
        {#each columns as column (column.id)}
          <div
            class="th"
            style={columnStyle(column.id)}
            draggable="true"
            ondragstart={(e) => startColumnDrag(e, column.id)}
            ondragover={(e) => dragColumnOver(e, column.id)}
            ondragleave={dragColumnLeave}
            ondrop={(e) => dropColumn(e, column.id)}
            ondragend={endColumnDrag}
            title="拖动调序，右键显示/隐藏列"
          >
            <span class="th-label">{column.id === "index" ? "#" : column.label}</span>
            {#if !column.fixed}
              <div class="resize-handle" onpointerdown={(e) => startColumnResize(e, column)} title="拖动调宽"></div>
            {/if}
          </div>
        {/each}
        <div class="col-filler"></div>
      </div>
      {#each pageResults as item, i}
        <div
          class="song-row"
          class:playing={playingKey === item.platform + ":" + item.source_id}
          role="button"
          tabindex="0"
          onclick={(e) => onSearchRowClick(item, i, e)}
          onkeydown={(e) => { if (e.key === "Enter") playResult(item); }}
          oncontextmenu={(e) => onSearchRowContext(item, i, e)}
        >
          {#each columns as column (column.id)}
            <div class="td" style={columnStyle(column.id)}>
              {#if column.id === "index"}
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
                  <img class="row-cover" src={item.cover_url} alt="" loading="lazy" referrerpolicy="no-referrer" onerror={(e) => void proxyCover(e, item)} />
                {:else}
                  <span class="cell-num">{(searchPage.page - 1) * searchPage.pageSize + i + 1}</span>
                {/if}
              {:else if column.id === "title"}
                <div class="name-cell">
                  <span class="name-text">{item.title}</span>
                  <span class="tag-badge">{platformLabel(item.platform)}</span>
                </div>
              {:else if column.id === "tag"}
                <div class="tag-cell">
                  {#each tagsByKey[keyOf(item)] ?? [] as tag}
                    <span class="tag-pill">{tag.namespace}:{tag.value}</span>
                  {/each}
                </div>
              {:else if column.id === "artist"}
                <span class="cell-text">{item.artist || ""}</span>
              {:else if column.id === "album"}
                <span class="cell-text">{item.album || ""}</span>
              {:else if column.id === "duration"}
                <span class="cell-time">{item.duration ? formatDuration(item.duration) : ""}</span>
              {/if}
            </div>
          {/each}
          <div class="col-filler"></div>
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
{#if columnMenu}
  <ContextMenu items={columnMenuItems} x={columnMenu.x} y={columnMenu.y} onclose={() => (columnMenu = null)} />
{/if}
{#if editingSong}
  <TagEditor song={editingSong} onclose={() => void closeTagEditor()} />
{/if}
{#if bulkTagSongs}
  <MultiTagEditor
    songs={bulkTagSongs}
    onclose={() => {
      bulkTagSongs = null;
      clearSearchSelection();
    }}
    onsaved={(updated) => {
      const next = { ...tagsByKey };
      for (const song of updated) {
        const source = song.sources[0];
        if (source) next[source.platform + ":" + source.id] = song.tags;
      }
      tagsByKey = next;
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

  .res-col {
    position: relative;
    display: flex;
    align-items: center;
    font-size: 11px;
    color: var(--color-base-content) / 0.5;
    overflow: hidden;
  }

  .col-resize {
    position: absolute;
    top: 0;
    right: -3px;
    width: 6px;
    height: 100%;
    cursor: col-resize;
    z-index: 2;
  }

  .column-btn {
    margin-left: auto;
    border: 1px solid var(--panel-border);
    border-radius: 4px;
    background: var(--color-base-200);
    color: var(--color-base-content) / 0.6;
    font-size: 12px;
    padding: 0 6px;
    cursor: pointer;
  }

  .res-source {
    font-size: 12px;
    color: var(--color-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

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
  .th { position: relative; display: flex; align-items: center; padding: 0 8px; min-width: 0; border-right: 1px solid var(--panel-border); cursor: grab; }
  .th:active { cursor: grabbing; }
  .th.drop-target::after { content: ""; position: absolute; right: -1px; top: 4px; bottom: 4px; width: 2px; background: var(--color-primary); border-radius: 1px; }
  .th-label { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .resize-handle { position: absolute; right: 0; top: 0; bottom: 0; width: 7px; cursor: col-resize; z-index: 3; touch-action: none; }
  .resize-handle::after { content: ""; position: absolute; left: 50%; top: 6px; bottom: 6px; width: 1px; background: transparent; }
  .resize-handle:hover::after { background: var(--color-primary) / 0.6; }
  .col-filler { flex: 1 1 0; min-width: 0; }
  .song-row { display: flex; align-items: center; height: 36px; min-width: 0; color: var(--color-base-content); font-size: 13px; cursor: pointer; transition: background .1s; border-radius: 0; }
  .song-row:hover { background: var(--color-base-300) / 0.7; }
  .song-row.playing { background: var(--color-primary) / 0.12; }
  .song-row.playing .name-text { color: var(--color-primary); }
  .td { display: flex; align-items: center; padding: 0 8px; min-width: 0; user-select: text; overflow: hidden; }
  .cell-num { color: var(--color-base-content) / 0.5; font-size: 12px; }
  .row-check { width: 13px; height: 13px; accent-color: var(--color-primary); cursor: pointer; }
  .row-cover { width: 30px; height: 30px; border-radius: 3px; object-fit: cover; }
  .name-cell { display: flex; align-items: center; gap: 6px; min-width: 0; width: 100%; }
  .name-text { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tag-badge { flex: none; font-size: 10px; padding: 1px 5px; border-radius: 4px; background: var(--color-primary) / 0.15; color: var(--color-primary); }
  .tag-cell { width: 100%; min-width: 0; display: flex; align-items: center; gap: 3px; overflow: hidden; white-space: nowrap; }
  .tag-pill { flex: none; max-width: 180px; overflow: hidden; text-overflow: ellipsis; font-size: 10px; line-height: 1.5; padding: 1px 5px; border-radius: 999px; background: var(--color-base-300); color: var(--color-base-content) / 0.7; }
  .cell-text { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cell-time { color: var(--color-base-content) / 0.6; font-size: 12px; }
  .column-btn { border: 1px solid var(--panel-border); border-radius: 4px; background: var(--color-base-200); color: var(--color-base-content) / 0.6; font-size: 12px; padding: 0 6px; margin: 4px; cursor: pointer; }

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
