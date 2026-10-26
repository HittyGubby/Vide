<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { page } from "$app/stores";
  import { ChevronDown } from "lucide-svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import QueryBar from "$lib/components/library/QueryBar.svelte";
  import OnlineSearchBar from "$lib/components/search/OnlineSearchBar.svelte";
  import { isTauri } from "$lib/isTauri";

  const appWindow = (isTauri ? getCurrentWindow() : null) as ReturnType<typeof getCurrentWindow>;

  // Library search uses QueryBar; online search reuses the same title-bar slot.
  let path = $derived(String($page.url.pathname) === "" || String($page.url.pathname) === "/index.html" ? "/" : String($page.url.pathname));
  let showSearch = $derived(path === "/" || path === "/search");
  let onlineSearch = $derived(path === "/search");

  let isMaximized = $state(false);

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
</script>

<header class="titlebar" data-tauri-drag-region>
  <div class="drag-space" data-tauri-drag-region></div>

  {#if showSearch}
    <div class="search-area" style="-webkit-app-region: no-drag">
      {#if onlineSearch}<OnlineSearchBar />{:else}<QueryBar />{/if}
    </div>
  {:else}
    <div class="drag-fill" data-tauri-drag-region></div>
  {/if}

  {#if ui.showPlayerDetail}
    <button
      class="back-btn"
      style="-webkit-app-region: no-drag"
      onclick={() => (ui.showPlayerDetail = false)}
      title="收起"
    >
      <ChevronDown size={16} />
    </button>
  {/if}

  <div class="window-controls" style="-webkit-app-region: no-drag">
    <button class="win-btn" onclick={minimize} aria-label="最小化">
      <svg width="12" height="12" viewBox="0 0 12 12">
        <rect x="1" y="5.5" width="10" height="1" fill="currentColor" />
      </svg>
    </button>
    <button class="win-btn" onclick={toggleMaximize} aria-label="最大化">
      {#if isMaximized}
        <svg width="12" height="12" viewBox="0 0 12 12">
          <rect x="2.5" y="0.5" width="9" height="9" rx="1" fill="none" stroke="currentColor" stroke-width="1" />
          <rect x="0.5" y="2.5" width="9" height="9" rx="1" fill="var(--bg-base)" stroke="currentColor" stroke-width="1" />
        </svg>
      {:else}
        <svg width="12" height="12" viewBox="0 0 12 12">
          <rect x="1" y="1" width="10" height="10" rx="1" fill="none" stroke="currentColor" stroke-width="1.2" />
        </svg>
      {/if}
    </button>
    <button class="win-btn win-close" onclick={close} aria-label="关闭">
      <svg width="12" height="12" viewBox="0 0 12 12">
        <path d="M2 2 L10 10 M10 2 L2 10" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
      </svg>
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

  .drag-fill {
    flex: 1;
    height: 100%;
    -webkit-app-region: drag;
  }

  .search-area {
    flex: 1 1 auto;
    display: flex;
    align-items: center;
    height: 100%;
    padding: 0 12px;
    min-width: 0;
    -webkit-app-region: no-drag;
  }

  .back-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 100%;
    padding: 0 10px;
    color: color-mix(in oklch, var(--color-base-content) 70%, transparent);
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
    color: color-mix(in oklch, var(--color-base-content) 70%, transparent);
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
