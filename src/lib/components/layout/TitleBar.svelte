<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { goto } from "$app/navigation";
  import { Search, ChevronDown, X } from "lucide-svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { searchStore } from "$lib/stores/search.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import type { SearchResultItem } from "$lib/types";

  const appWindow = getCurrentWindow();

  let isMaximized = $state(false);
  let query = $state("");
  let autocomplete = $state<string[]>([]);
  let showDrop = $state(false);
  let loading = $state(false);
  let searchInputEl = $state<HTMLInputElement | null>(null);
  let dropTimer: ReturnType<typeof setTimeout> | undefined;

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

  function minimize() {
    appWindow.minimize();
  }
  function toggleMaximize() {
    appWindow.toggleMaximize();
  }
  function close() {
    // If "close to tray" is enabled, hide to tray instead of quitting.
    if (settingsStore.settings.close_to_tray) {
      appWindow.hide();
    } else {
      appWindow.close();
    }
  }

  function focusSearch() {
    searchInputEl?.focus();
  }

  // Autocomplete suggestions come from the local search autocomplete command.
  async function handleInput() {
    const q = query.trim();
    if (!q) {
      autocomplete = [];
      showDrop = false;
      return;
    }
    showDrop = true;
    try {
      autocomplete = await invoke<string[]>("autocomplete", { prefix: q });
    } catch {
      autocomplete = [];
    }
  }

  function submitSearch() {
    const q = query.trim();
    if (!q) return;
    searchStore.add(q);
    showDrop = false;
    goto("/search?q=" + encodeURIComponent(q));
  }

  function pickSuggestion(s: string) {
    query = s;
    submitSearch();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      submitSearch();
    } else if (e.key === "Escape") {
      showDrop = false;
    }
  }
</script>

<header class="titlebar" data-tauri-drag-region>
  <div class="drag-space" data-tauri-drag-region></div>
  <div class="search-area" style="-webkit-app-region: no-drag">
    <div class="search-field">
      <div class="search-inner">
        <span class="search-icon"><Search size={14} /></span>
        <input
          type="text"
          class="search-input"
          data-tauri-drag-region="false"
          placeholder="搜索歌曲、艺术家、标签..."
          bind:value={query}
          bind:this={searchInputEl}
          oninput={handleInput}
          onkeydown={handleKeydown}
          onfocus={() => (showDrop = query.trim().length > 0)}
          onblur={() => {
            dropTimer = setTimeout(() => (showDrop = false), 150);
          }}
          onmousedown={() => {
            clearTimeout(dropTimer);
          }}
        />
        {#if query}
          <button class="search-clear" onclick={() => (query = "")} aria-label="清除">
            <X size={12} />
          </button>
        {/if}
      </div>

      {#if showDrop && autocomplete.length > 0}
        <div class="autocomplete-drop">
          {#each autocomplete as s}
            <button
              class="ac-item"
              onclick={() => pickSuggestion(s)}
              onmousedown={(e) => e.preventDefault()}
            >
              {s}
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  <div class="drag-fill" data-tauri-drag-region></div>

  {#if ui.showPlayerDetail}
    <button class="back-btn" style="-webkit-app-region: no-drag" onclick={() => (ui.showPlayerDetail = false)} title="收起">
      <ChevronDown size={16} />
    </button>
  {/if}

  <div class="window-controls" style="-webkit-app-region: no-drag">
    <button class="win-btn" onclick={minimize} aria-label="最小化">
      <svg width="12" height="12" viewBox="0 0 12 12"><rect x="1" y="5.5" width="10" height="1" fill="currentColor" /></svg>
    </button>
    <button class="win-btn" onclick={toggleMaximize} aria-label="最大化">
      {#if isMaximized}
        <svg width="12" height="12" viewBox="0 0 12 12"><rect x="2.5" y="0.5" width="9" height="9" rx="1" fill="none" stroke="currentColor" stroke-width="1" /><rect x="0.5" y="2.5" width="9" height="9" rx="1" fill="var(--bg-base)" stroke="currentColor" stroke-width="1" /></svg>
      {:else}
        <svg width="12" height="12" viewBox="0 0 12 12"><rect x="1" y="1" width="10" height="10" rx="1" fill="none" stroke="currentColor" stroke-width="1.2" /></svg>
      {/if}
    </button>
    <button class="win-btn win-close" onclick={close} aria-label="关闭">
      <svg width="12" height="12" viewBox="0 0 12 12"><path d="M2 2 L10 10 M10 2 L2 10" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" /></svg>
    </button>
  </div>
</header>

<style>
  .titlebar {
    display: flex;
    align-items: center;
    height: var(--titlebar-height);
    background: var(--color-base-200);
    border-bottom: 1px solid var(--panel-border-strong);
    padding: 0;
    z-index: 100;
    position: relative;
    -webkit-app-region: drag;
  }

  .drag-space {
    flex: none;
    width: 18px;
    height: 100%;
  }

  .search-area {
    flex: 0 1 560px;
    display: flex;
    justify-content: flex-start;
    align-items: center;
    height: 100%;
    padding: 0 12px;
    -webkit-app-region: no-drag;
    min-width: 0;
  }

  .drag-fill {
    flex: 1;
    height: 100%;
    -webkit-app-region: drag;
  }

  .search-field {
    position: relative;
    width: 100%;
  }

  .search-inner {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    height: 28px;
    padding: 0 10px;
    background: var(--color-base-100);
    border: 1px solid var(--panel-border);
    border-radius: 6px;
    transition: border-color 0.15s;
  }

  .search-inner:focus-within {
    border-color: var(--color-primary);
  }

  .search-icon {
    color: var(--color-base-content) / 0.5;
    flex-shrink: 0;
    display: flex;
  }

  .search-input {
    flex: 1;
    height: 100%;
    background: none;
    border: none;
    color: var(--color-base-content);
    font-size: 12.5px;
    outline: none;
    user-select: text;
  }

  .search-input::placeholder {
    color: var(--color-base-content) / 0.45;
  }

  .search-clear {
    display: flex;
    color: var(--color-base-content) / 0.5;
    background: none;
    border: none;
  }

  .autocomplete-drop {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    background: var(--color-base-300);
    border: 1px solid var(--color-base-content) / 0.1;
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
    z-index: 200;
    overflow: hidden;
    padding: 2px;
    max-height: 260px;
    overflow-y: auto;
  }

  .ac-item {
    display: block;
    width: 100%;
    text-align: left;
    padding: 6px 10px;
    font-size: 12.5px;
    color: var(--color-base-content);
    background: none;
    border: none;
    border-radius: 4px;
  }

  .ac-item:hover {
    background: var(--color-primary) / 0.12;
  }

  .back-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 100%;
    padding: 0 10px;
    color: var(--color-base-content) / 0.7;
    background: none;
    border: none;
    font-size: 12px;
  }

  .back-btn:hover {
    color: var(--color-base-content);
  }

  .window-controls {
    display: flex;
    align-items: center;
    height: 100%;
    -webkit-app-region: no-drag;
  }

  .win-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 100%;
    color: var(--color-base-content) / 0.7;
    background: none;
    border: none;
  }

  .win-btn:hover {
    background: var(--color-base-300);
    color: var(--color-base-content);
  }

  .win-close:hover {
    background: var(--color-error);
    color: var(--color-error-content);
  }
</style>
