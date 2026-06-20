<script lang="ts">
  import "../app.css";
  import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
  import TitleBar from "$lib/components/layout/TitleBar.svelte";
  import Sidebar from "$lib/components/layout/Sidebar.svelte";
  import ProgressBar from "$lib/components/layout/ProgressBar.svelte";
  import PlayerBar from "$lib/components/layout/PlayerBar.svelte";

  let { children } = $props();

  const WINDOW_STATE_KEY = "vide_window_state";

  // Restore window size on mount; save on resize
  $effect(() => {
    const appWindow = getCurrentWindow();
    let cancelled = false;

    try {
      const saved = localStorage.getItem(WINDOW_STATE_KEY);
      if (saved) {
        const state = JSON.parse(saved);
        if (state.maximized) {
          appWindow.maximize();
        } else if (state.width && state.height) {
          setTimeout(() => {
            if (!cancelled) {
              appWindow.setSize(new LogicalSize(state.width, state.height));
            }
          }, 0);
        }
      }
    } catch {
      /* ignore corrupt state */
    }

    const unlisten = appWindow.onResized(async () => {
      try {
        if (cancelled) return;
        const maximized = await appWindow.isMaximized();
        if (maximized) {
          localStorage.setItem(
            WINDOW_STATE_KEY,
            JSON.stringify({ maximized: true }),
          );
        } else {
          const size = await appWindow.outerSize();
          localStorage.setItem(
            WINDOW_STATE_KEY,
            JSON.stringify({
              width: size.width,
              height: size.height,
              maximized: false,
            }),
          );
        }
      } catch {
        /* ignore */
      }
    });

    return () => {
      cancelled = true;
      unlisten.then((fn) => fn());
    };
  });
</script>

<div class="app-shell">
  <TitleBar />

  <div class="main-area">
    <Sidebar />
    <main class="content">
      {@render children()}
    </main>
  </div>

  <ProgressBar />
  <PlayerBar />
</div>

<style>
  .app-shell {
    display: grid;
    grid-template-rows: var(--titlebar-height) 1fr auto var(--playerbar-height);
    height: 100vh;
    background: var(--bg-base);
    color: var(--text-primary);
  }

  .main-area {
    display: grid;
    grid-template-columns: var(--sidebar-width) 1fr;
    overflow: hidden;
    min-height: 0;
  }

  .content {
    overflow-y: auto;
    overflow-x: hidden;
    background: var(--bg-base);
  }
</style>
