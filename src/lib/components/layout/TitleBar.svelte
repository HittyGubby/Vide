<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { Search, ChevronDown, Play, Loader, X } from "lucide-svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { player } from "$lib/stores/player.svelte";

  const appWindow = getCurrentWindow();

  let isMaximized = $state(false);
  let searchInputEl = $state<HTMLInputElement | null>(null);

  $effect(() => {
    let cancelled = false;
    appWindow.isMaximized().then((m) => {
      if (!cancelled) isMaximized = m;
    });
    const unlisten = appWindow.onResized(async () => {
      if (!cancelled) isMaximized = await appWindow.isMaximized();
    });
    return () => {
      cancelled = true;
      unlisten.then((fn) => fn());
    };
  });

  // React to sidebar search button clicks
  $effect(() => {
    const _ = ui.focusTitleBarSearch;
    searchInputEl?.focus();
  });

  function minimize() {
    appWindow.minimize();
  }
  function toggleMaximize() {
    appWindow.toggleMaximize();
  }
  function close() {
    appWindow.close();
  }

  // ── Online Search ──────────────────────────────────

  let searchQuery = $state("");
  let searchResults = $state<any[]>([]);
  let searchLoading = $state(false);
  let searchError = $state<string | null>(null);
  let showResults = $state(false);
  let resultTimer: ReturnType<typeof setTimeout> | undefined;
  let activeTab = $state<string | null>(null);

  // Group results by platform for tab display
  let groupedResults = $derived.by(() => {
    const groups = new Map<string, any[]>();
    for (const item of searchResults) {
      const arr = groups.get(item.platform) ?? [];
      arr.push(item);
      groups.set(item.platform, arr);
    }
    return groups;
  });

  let platformTabs = $derived(Array.from(groupedResults.keys()));

  let platformResults = $derived(
    activeTab ? (groupedResults.get(activeTab) ?? []) : searchResults,
  );

  async function doSearch() {
    const q = searchQuery.trim();
    if (!q) return;
    searchLoading = true;
    searchError = null;
    searchResults = [];
    showResults = true;
    try {
      searchResults = await invoke("search_online", {
        query: q,
        platform: null,
      });
    } catch (e) {
      searchError = String(e);
      searchResults = [];
    } finally {
      searchLoading = false;
      // Set active tab to the first platform with results
      if (searchResults.length > 0) {
        const groups = new Map<string, any[]>();
        for (const item of searchResults) {
          const arr = groups.get(item.platform) ?? [];
          arr.push(item);
          groups.set(item.platform, arr);
        }
        activeTab = Array.from(groups.keys())[0];
      } else {
        activeTab = null;
      }
    }
  }

  async function playResult(item: {
    platform: string;
    id: string;
    title: string;
  }) {
    try {
      const song: { id: string; title: string; artist: string } = await invoke(
        "resolve_song",
        {
          platform: item.platform,
          id: item.id,
        },
      );
      await invoke("play_song", { id: song.id });
      player.mediaId = song.id;
      player.title = song.title;
      player.artist = song.artist || "Unknown";
      player.isPlaying = true;
      showResults = false;
      searchQuery = "";
    } catch (e) {
      console.error("play error:", e);
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      doSearch();
    } else if (e.key === "Escape") {
      showResults = false;
    }
  }

  function handleBlur() {
    resultTimer = setTimeout(() => {
      showResults = false;
    }, 200);
  }

  function handleFocus() {
    clearTimeout(resultTimer);
  }

  function switchTab(platform: string) {
    activeTab = platform;
  }
</script>

<header class="titlebar" data-tauri-drag-region>
  <div class="titlebar-drag">
    <span class="app-logo">Vide</span>
  </div>

  <div class="search-area" style="-webkit-app-region: no-drag">
    <div class="search-field">
      <div class="search-inner">
        <span class="search-icon">
          <Search size={14} />
        </span>
        <input
          type="text"
          class="search-input"
          placeholder="搜索 YouTube, Bilibili, 全网..."
          bind:value={searchQuery}
          bind:this={searchInputEl}
          onkeydown={handleKeydown}
          onfocus={handleFocus}
          onblur={handleBlur}
        />
        {#if searchLoading}
          <span class="search-spinner"><Loader size={14} class="spin" /></span>
        {/if}
      </div>

      {#if showResults && (searchResults.length > 0 || searchLoading || searchError)}
        <div class="results-dropdown">
          {#if searchLoading}
            <div class="result-status">搜索中...</div>
          {:else if searchError}
            <div class="result-status result-error">
              搜索失败: {searchError}
            </div>
          {:else}
            <div class="result-header">
              搜索结果
              <button
                class="result-close"
                onclick={() => (showResults = false)}
              >
                <X size={12} />
              </button>
            </div>
            <!-- Tab switcher -->
            {#if platformTabs.length > 1}
              <div class="result-tabs">
                {#each platformTabs as tab}
                  <button
                    class="result-tab"
                    class:active-tab={activeTab === tab}
                    onclick={() => switchTab(tab)}
                  >
                    {tab}
                    <span class="tab-count"
                      >{groupedResults.get(tab)?.length ?? 0}</span
                    >
                  </button>
                {/each}
              </div>
            {/if}
            <div class="result-list">
              {#each platformResults as item}
                <button class="result-item" onclick={() => playResult(item)}>
                  <span class="result-platform">{item.platform}</span>
                  <span class="result-title truncate">{item.title}</span>
                  <span class="result-artist truncate">{item.artist || ""}</span
                  >
                  <span class="result-play"><Play size={12} /></span>
                </button>
              {/each}
            </div>
          {/if}
        </div>
      {/if}
    </div>
  </div>

  {#if ui.showPlayerDetail}
    <button
      class="back-btn"
      onclick={() => (ui.showPlayerDetail = false)}
      title="收起"
    >
      <ChevronDown size={16} />
      <span>收起</span>
    </button>
  {/if}

  <div class="window-controls">
    <button class="win-btn" onclick={minimize} aria-label="最小化">
      <svg width="12" height="12" viewBox="0 0 12 12">
        <rect x="1" y="5.5" width="10" height="1" fill="currentColor" />
      </svg>
    </button>
    <button class="win-btn" onclick={toggleMaximize} aria-label="最大化">
      {#if isMaximized}
        <svg width="12" height="12" viewBox="0 0 12 12">
          <rect
            x="2.5"
            y="0.5"
            width="9"
            height="9"
            rx="1"
            fill="none"
            stroke="currentColor"
            stroke-width="1"
          />
          <rect
            x="0.5"
            y="2.5"
            width="9"
            height="9"
            rx="1"
            fill="var(--bg-base)"
            stroke="currentColor"
            stroke-width="1"
          />
        </svg>
      {:else}
        <svg width="12" height="12" viewBox="0 0 12 12">
          <rect
            x="1"
            y="1"
            width="10"
            height="10"
            rx="1"
            fill="none"
            stroke="currentColor"
            stroke-width="1.2"
          />
        </svg>
      {/if}
    </button>
    <button class="win-btn win-close" onclick={close} aria-label="关闭">
      <svg width="12" height="12" viewBox="0 0 12 12">
        <path
          d="M2 2 L10 10 M10 2 L2 10"
          stroke="currentColor"
          stroke-width="1.2"
          stroke-linecap="round"
        />
      </svg>
    </button>
  </div>
</header>

<style>
  .titlebar {
    display: flex;
    align-items: center;
    height: var(--titlebar-height);
    background: var(--bg-base);
    border-bottom: 1px solid var(--border-subtle);
    padding: 0;
    z-index: 100;
    position: relative;
    -webkit-app-region: drag;
  }

  .titlebar-drag {
    display: flex;
    align-items: center;
    height: 100%;
    padding: 0 12px;
    min-width: fit-content;
    -webkit-app-region: drag;
  }

  .app-logo {
    font-weight: 600;
    font-size: 14px;
    color: var(--accent-primary);
    letter-spacing: 0.5px;
  }

  .search-area {
    flex: 1;
    display: flex;
    justify-content: center;
    align-items: center;
    height: 100%;
    padding: 0 12px;
  }

  .search-field {
    position: relative;
    width: 100%;
    max-width: 480px;
  }

  .search-inner {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    height: 26px;
    padding: 0 10px;
    background: var(--bg-input);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    transition: border-color 0.15s;
  }

  .search-inner:focus-within {
    border-color: var(--accent-primary);
  }

  .search-icon {
    color: var(--text-tertiary);
    flex-shrink: 0;
    display: flex;
  }

  .search-input {
    flex: 1;
    height: 100%;
    background: none;
    border: none;
    color: var(--text-primary);
    font-size: 12.5px;
    outline: none;
    user-select: text;
  }

  .search-input::placeholder {
    color: var(--text-tertiary);
  }

  .search-spinner {
    color: var(--accent-primary);
    flex-shrink: 0;
    display: flex;
  }

  .spin {
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* ── Results Dropdown ───────────────────────────── */
  .results-dropdown {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    background: var(--bg-elevated);
    border: 1px solid var(--border-normal);
    border-radius: 8px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
    z-index: 200;
    max-height: 320px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .result-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px 4px;
    font-size: 10px;
    font-weight: 600;
    color: var(--text-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .result-close {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border-radius: 3px;
    color: var(--text-tertiary);
    background: none;
    border: none;
  }

  .result-close:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .result-status {
    padding: 16px;
    text-align: center;
    font-size: 12px;
    color: var(--text-tertiary);
  }

  .result-error {
    color: var(--status-error);
  }

  /* ── Result Tabs ────────────────────────────────── */
  .result-tabs {
    display: flex;
    gap: 2px;
    padding: 2px 12px 4px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .result-tab {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 3px 8px;
    border-radius: 4px;
    background: none;
    border: none;
    color: var(--text-tertiary);
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.3px;
    cursor: pointer;
    transition:
      background 0.1s,
      color 0.1s;
  }

  .result-tab:hover {
    background: var(--bg-hover);
    color: var(--text-secondary);
  }
  .result-tab.active-tab {
    color: var(--accent-primary);
    background: var(--accent-surface);
  }

  .tab-count {
    font-size: 9px;
    color: inherit;
    opacity: 0.6;
  }

  .result-list {
    overflow-y: auto;
    padding: 2px 0;
  }

  .result-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 12px;
    background: none;
    border: none;
    color: var(--text-primary);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }

  .result-item:hover {
    background: var(--bg-hover);
  }

  .result-platform {
    font-size: 9px;
    font-weight: 600;
    color: var(--accent-primary);
    text-transform: uppercase;
    background: var(--accent-surface);
    padding: 1px 4px;
    border-radius: 3px;
    flex-shrink: 0;
  }

  .result-title {
    flex: 1;
    font-weight: 500;
  }

  .result-artist {
    flex: 1;
    color: var(--text-secondary);
    font-size: 11px;
    display: none;
  }

  .result-play {
    color: var(--text-tertiary);
    flex-shrink: 0;
    opacity: 0;
    transition: opacity 0.1s;
  }

  .result-item:hover .result-play {
    opacity: 1;
    color: var(--accent-primary);
  }

  /* ── Back Button ────────────────────────────────── */
  .back-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 12px;
    height: 100%;
    color: var(--text-secondary);
    background: none;
    border: none;
    font-size: 12px;
    transition:
      background 0.1s,
      color 0.1s;
    -webkit-app-region: no-drag;
  }

  .back-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  /* ── Window Controls ────────────────────────────── */
  .window-controls {
    display: flex;
    align-items: stretch;
    height: 100%;
    -webkit-app-region: no-drag;
  }

  .win-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 46px;
    height: 100%;
    color: var(--text-secondary);
    background: none;
    border: none;
    border-radius: 0;
    transition:
      background 0.1s,
      color 0.1s;
    -webkit-app-region: no-drag;
  }

  .win-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .win-btn:active {
    background: var(--bg-active);
  }
  .win-close:hover {
    background: #e81123;
    color: white;
  }
  .win-close:active {
    background: #bf0f1d;
  }
</style>
