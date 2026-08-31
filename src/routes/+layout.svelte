<script lang="ts">
  import "../app.css";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow, LogicalPosition, LogicalSize } from "@tauri-apps/api/window";
  import { LYRICS_WINDOW_LABEL, applyLyricsWindowSettings } from "$lib/desktopLyrics";
  import { eventMatchesAccelerator, runShortcutAction } from "$lib/shortcuts";
  import type { ShortcutAction } from "$lib/types";
  import TitleBar from "$lib/components/layout/TitleBar.svelte";
  import Sidebar from "$lib/components/layout/Sidebar.svelte";
  import PlayerBar from "$lib/components/layout/PlayerBar.svelte";
  import PlayerDetail from "$lib/components/player/PlayerDetail.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { player } from "$lib/stores/player.svelte";

  let { children } = $props();

  const WINDOW_STATE_KEY = "vide_window_state";

  // This layout is shared with the desktop lyrics window, which must not run
  // the main window's restore/poll/shortcut machinery.
  const isLyricsWindow = getCurrentWindow().label === LYRICS_WINDOW_LABEL;

  // Load settings once (for auto-play resume).
  let settingsLoaded = false;
  $effect(() => {
    if (isLyricsWindow || settingsLoaded) return;
    settingsStore.load().then(async () => {
      settingsLoaded = true;
      // Always restore the last-played song's identity & position so the UI
      // reflects where the user left off even without auto-play.
      const last = await player.getLastPlayed();
      if (last && last.songId) {
        try {
          const song = await invoke<{
            id: string;
            title: string;
            artist: string;
            album: string;
            metadata?: { display_title?: string; artist_override?: string };
          }>("get_song", { id: last.songId });
          if (song) {
            player.mediaId = last.songId;
            player.title = song.metadata?.display_title ?? song.title;
            player.artist = song.metadata?.artist_override ?? song.artist;
            player.album = song.album ?? "";
            player.elapsed = last.position;
            // Remember the resume position so pressing play seeks there.
            player._pendingSeek = last.position;
          }
        } catch (e) {
          console.error("restore last-played song error:", e);
        }

        // "启动时自动播放音乐": resume playback at the saved position. The
        // position is passed to play_song (not seeked afterwards) because mpv
        // rejects a seek issued before it has finished loading the file.
        if (settingsStore.settings.auto_play_on_start) {
          try {
            await invoke("play_song", {
              id: last.songId,
              positionSecs: last.position > 0 ? last.position : null,
            });
            player.elapsed = last.position;
            player.isPlaying = true;
          } catch (e) {
            console.error("auto-play resume error:", e);
          }
        }
      }
    });
  });

  // ── Shortcuts ──────────────────────────────────────
  // In-app bindings fire while the window has focus. Global ones are registered
  // with the OS by the backend, which reports them back as "shortcut" events.
  $effect(() => {
    if (isLyricsWindow) return;
    const bindings = settingsStore.settings.shortcuts;
    const onKeyDown = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      // Never steal keys from text fields (settings inputs, the CSS editor),
      // nor while a shortcut cell in the settings is recording a new binding.
      if (
        target &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.tagName === "SELECT" ||
          target.isContentEditable ||
          target.closest("[data-shortcut-capture]"))
      ) {
        return;
      }
      for (const action of Object.keys(bindings) as ShortcutAction[]) {
        const accelerator = bindings[action]?.in_app;
        if (accelerator && eventMatchesAccelerator(e, accelerator)) {
          e.preventDefault();
          void runShortcutAction(action);
          return;
        }
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  // Push the global bindings to the backend whenever they change.
  $effect(() => {
    if (isLyricsWindow) return;
    const bindings = settingsStore.settings.shortcuts;
    const items = (Object.keys(bindings) as ShortcutAction[])
      .map((action) => ({ action, accelerator: bindings[action]?.global ?? "" }))
      .filter((i) => i.accelerator.trim().length > 0);
    void invoke<string[]>("set_global_shortcuts", { items })
      .then((failures) => {
        if (failures?.length) console.warn("global shortcuts not registered:", failures);
      })
      .catch((e) => console.error("set_global_shortcuts error:", e));
  });

  // Global shortcuts arrive as events so both runtimes share one code path.
  $effect(() => {
    if (isLyricsWindow) return;
    const unlisten = listen<string>("shortcut", (event) => {
      void runShortcutAction(event.payload as ShortcutAction);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  });

  // Keep the desktop lyrics window in step with its settings (visibility,
  // always-on-top, lock) — only the main window owns that window.
  $effect(() => {
    if (isLyricsWindow) return;
    const s = settingsStore.settings;
    void applyLyricsWindowSettings({
      desktop_lyrics_enabled: s.desktop_lyrics_enabled,
      desktop_lyrics_locked: s.desktop_lyrics_locked,
      desktop_lyrics_always_on_top: s.desktop_lyrics_always_on_top,
    } as typeof s);
  });

  // Reactively apply font + window scale so setting changes take effect
  // immediately (no restart needed).
  $effect(() => {
    if (isLyricsWindow) return;
    const font = settingsStore.settings.font_family;
    if (font) document.body.style.fontFamily = font;
    const scale = (settingsStore.settings.window_scale || 100) / 100;
    document.body.style.zoom = String(scale);
  });

  interface WindowState {
    width: number;
    height: number;
    x: number;
    y: number;
    maximized: boolean;
  }

  // Save the window geometry on every move/resize (debounced so dragging the
  // window does not thrash localStorage) and restore it on the next launch.
  const WINDOW_SAVE_DELAY = 200;
  let windowSaveTimer: ReturnType<typeof setTimeout> | null = null;
  let windowStateOffs: (() => void)[] = [];

  /**
   * `outerSize`/`outerPosition` are physical pixels while `setSize`/
   * `setPosition` take logical ones, so convert through the scale factor —
   * otherwise the window grows a little on every restart on HiDPI displays.
   */
  async function saveWindowState() {
    const appWindow = getCurrentWindow();
    try {
      const [maximized, factor, size, pos] = await Promise.all([
        appWindow.isMaximized(),
        appWindow.scaleFactor(),
        appWindow.outerSize(),
        appWindow.outerPosition(),
      ]);
      const state: WindowState = {
        width: Math.round(size.width / factor),
        height: Math.round(size.height / factor),
        x: Math.round(pos.x / factor),
        y: Math.round(pos.y / factor),
        maximized,
      };
      localStorage.setItem(WINDOW_STATE_KEY, JSON.stringify(state));
    } catch {
      /* ignore */
    }
  }

  function scheduleWindowSave() {
    if (windowSaveTimer) clearTimeout(windowSaveTimer);
    windowSaveTimer = setTimeout(() => {
      windowSaveTimer = null;
      void saveWindowState();
    }, WINDOW_SAVE_DELAY);
  }

  $effect(() => {
    if (isLyricsWindow) return;
    const appWindow = getCurrentWindow();
    let cancelled = false;

    void (async () => {
      try {
        const saved = localStorage.getItem(WINDOW_STATE_KEY);
        if (saved) {
          const state = JSON.parse(saved) as Partial<WindowState>;
          // Size and position first: they are what the window falls back to
          // as soon as the user restores it from maximized.
          if (state.width && state.height) {
            await appWindow.setSize(new LogicalSize(state.width, state.height));
          }
          if (typeof state.x === "number" && typeof state.y === "number") {
            await appWindow.setPosition(new LogicalPosition(state.x, state.y));
          }
          if (state.maximized) await appWindow.maximize();
        }
      } catch {
        /* ignore corrupt state */
      }
    })();

    const listen = async () => {
      const offResized = await appWindow.onResized(() => {
        if (!cancelled) scheduleWindowSave();
      });
      const offMoved = await appWindow.onMoved(() => {
        if (!cancelled) scheduleWindowSave();
      });
      if (cancelled) {
        offResized();
        offMoved();
        return;
      }
      windowStateOffs.push(offResized, offMoved);
    };
    void listen();

    return () => {
      cancelled = true;
      if (windowSaveTimer) clearTimeout(windowSaveTimer);
      for (const off of windowStateOffs.splice(0)) off();
    };
  });
</script>

<!-- The desktop lyrics window shares this layout (SvelteKit always nests the
     root layout), so it renders the page bare — no sidebar, title bar or
     player bar — and skips every main-window-only effect. -->
{#if isLyricsWindow}
  {@render children()}
{:else}
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
