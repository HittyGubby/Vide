<script lang="ts">
  import "../app.css";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
  import TitleBar from "$lib/components/layout/TitleBar.svelte";
  import Sidebar from "$lib/components/layout/Sidebar.svelte";
  import PlayerBar from "$lib/components/layout/PlayerBar.svelte";
  import PlayerDetail from "$lib/components/player/PlayerDetail.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { player } from "$lib/stores/player.svelte";

  let { children } = $props();

  const WINDOW_STATE_KEY = "vide_window_state";

  // Load settings once (for auto-play resume).
  let settingsLoaded = false;
  $effect(() => {
    if (settingsLoaded) return;
    settingsStore.load().then(async () => {
      settingsLoaded = true;
      // "启动时自动播放音乐": resume the last-played song at its saved position.
      if (settingsStore.settings.auto_play_on_start) {
        const last = await player.getLastPlayed();
        if (last && last.songId) {
          try {
            await invoke("play_song", { id: last.songId });
            if (last.position > 0) {
              await invoke("seek", { positionSecs: last.position });
            }
          } catch (e) {
            console.error("auto-play resume error:", e);
          }
        }
      }
    });
  });

  // Reactively apply font + window scale so setting changes take effect
  // immediately (no restart needed).
  $effect(() => {
    const font = settingsStore.settings.font_family;
    if (font) document.body.style.fontFamily = font;
    const scale = (settingsStore.settings.window_scale || 100) / 100;
    document.body.style.zoom = String(scale);
  });

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
