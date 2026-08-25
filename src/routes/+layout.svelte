<script lang="ts">
  import "../app.css";
  import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
  import TitleBar from "$lib/components/layout/TitleBar.svelte";
  import Sidebar from "$lib/components/layout/Sidebar.svelte";
  import PlayerBar from "$lib/components/layout/PlayerBar.svelte";
  import PlayerDetail from "$lib/components/player/PlayerDetail.svelte";
  import { ui } from "$lib/stores/ui.svelte";

  let { children } = $props();

  const WINDOW_STATE_KEY = "vide_window_state";

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
          localStorage.setItem(WINDOW_STATE_KEY, JSON.stringify({ maximized: true }));
        } else {
          const size = await appWindow.outerSize();
          localStorage.setItem(
            WINDOW_STATE_KEY,
            JSON.stringify({ width: size.width, height: size.height, maximized: false }),
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
  <Sidebar />

  <div class="right">
    <TitleBar />
    <main class="content">
      {@render children()}
    </main>
    {#if !ui.showPlayerDetail}
      <PlayerBar />
    {/if}
  </div>
</div>

{#if ui.showPlayerDetail}
  <PlayerDetail />
{/if}

<style>
  .app-shell {
    display: flex;
    height: 100vh;
    background: var(--color-base-100);
    color: var(--color-base-content);
  }

  .right {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .content {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    background: var(--color-base-100);
    min-width: 0;
    min-height: 0;
  }
</style>
