<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { Search } from "lucide-svelte";

  const appWindow = getCurrentWindow();

  let isMaximized = $state(false);

  $effect(() => {
    let cancelled = false;

    appWindow.isMaximized().then((m) => {
      if (!cancelled) isMaximized = m;
    });

    const unlisten = appWindow.onResized(async () => {
      if (!cancelled) {
        isMaximized = await appWindow.isMaximized();
      }
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
    appWindow.close();
  }
</script>

<header class="titlebar" data-tauri-drag-region>
  <div class="titlebar-drag">
    <span class="app-logo">Vide</span>
  </div>

  <div class="search-area" style="-webkit-app-region: no-drag">
    <div class="search-inner">
      <span class="search-icon">
        <Search size={14} />
      </span>
      <input
        type="text"
        class="search-input"
        placeholder="搜索 YouTube, Bilibili, 全网..."
        onfocus={(e) => e.currentTarget.select()}
      />
    </div>
  </div>

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
  }

  .titlebar-drag {
    display: flex;
    align-items: center;
    height: 100%;
    padding: 0 12px;
    min-width: fit-content;
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

  .search-inner {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    max-width: 480px;
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

  /* ── Window Controls ────────────────────────────────── */
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
