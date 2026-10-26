<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "$lib/transport";
  import { listen } from "@tauri-apps/api/event";
  import {
    getCurrentWindow,
    LogicalPosition,
    LogicalSize,
    type Window,
  } from "@tauri-apps/api/window";
  import { player } from "$lib/stores/player.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import LyricsView from "$lib/components/lyrics/LyricsView.svelte";
  import { parseLyrics, type LyricLine } from "$lib/lyrics";
  import { saveClientState } from "$lib/clientState";
  import { isTauri } from "$lib/isTauri";
  import { goto } from "$app/navigation";

  // Only the Tauri lyrics window may load this route; on the web it is
  // unreachable except by typing the URL, so send the user back to the app.
  const appWindow = (isTauri ? getCurrentWindow() : null) as Window;

  let lines = $state<LyricLine[]>([]);
  let rawLyrics = $state<string | null>(null);
  let locked = $derived(settingsStore.settings.desktop_lyrics_locked);

  // A temporary frame + hint flashed whenever the window is shown. The window
  // is usually fully transparent (sometimes with no lyrics at all) and may be
  // locked/click-through, so without this there is no way to find it.
  let pulse = $state(false);
  let pulseTimer: ReturnType<typeof setTimeout> | null = null;
  let geometryTimer: ReturnType<typeof setTimeout> | null = null;
  const geometryOffs: (() => void)[] = [];

  function scheduleGeometrySave() {
    if (geometryTimer) clearTimeout(geometryTimer);
    geometryTimer = setTimeout(() => {
      geometryTimer = null;
      void (async () => {
        try {
          const [pos, size] = await Promise.all([
            appWindow.outerPosition(),
            appWindow.outerSize(),
          ]);
          // Physical pixels, matching what applyLyricsWindowSettings restores.
          await saveClientState("window.lyrics", {
            x: pos.x,
            y: pos.y,
            w: size.width,
            h: size.height,
          });
        } catch {
          /* non-fatal */
        }
      })();
    }, 300);
  }

  function flashFrame() {
    pulse = true;
    if (pulseTimer) clearTimeout(pulseTimer);
    pulseTimer = setTimeout(() => {
      pulse = false;
      pulseTimer = null;
    }, 1800);
  }

  // The desktop window has no player bar, so it polls the backend itself.
  onMount(() => {
    if (!isTauri) {
      void goto("/");
      return;
    }
    void settingsStore.load();
    void player.poll();
    const timer = setInterval(() => void player.poll(), 500);

    // The main window announces "just shown" after it clamps the position.
    const unlistenShown = listen("vide:lyrics-shown", () => flashFrame());

    // Belt and braces: if the event raced the page mount, notice the window
    // becoming visible on our own.
    let wasVisible = false;
    const visibilityTimer = setInterval(() => {
      void appWindow
        .isVisible()
        .then((visible) => {
          if (visible && !wasVisible) flashFrame();
          wasVisible = visible;
        })
        .catch(() => {});
    }, 500);

    // Re-read the settings whenever the main window saves them, so CSS edits
    // show up here immediately.
    const unlistenSettings = listen("vide:settings-changed", () => void settingsStore.load());

    // Persist the window geometry so it re-opens where the user left it.
    let disposed = false;
    void (async () => {
      const offs = await Promise.all([
        appWindow.onMoved(scheduleGeometrySave),
        appWindow.onResized(scheduleGeometrySave),
      ]);
      if (disposed) {
        for (const off of offs) off();
      } else {
        geometryOffs.push(...offs);
      }
    })();

    return () => {
      disposed = true;
      clearInterval(timer);
      clearInterval(visibilityTimer);
      if (pulseTimer) clearTimeout(pulseTimer);
      if (geometryTimer) clearTimeout(geometryTimer);
      for (const off of geometryOffs.splice(0)) off();
      void unlistenShown.then((fn) => fn());
      void unlistenSettings.then((fn) => fn());
    };
  });

  $effect(() => {
    const id = player.mediaId;
    if (!id) {
      rawLyrics = null;
      lines = [];
      return;
    }
    invoke<string | null>("get_lyrics", { songId: id })
      .then((raw) => {
        rawLyrics = raw ?? null;
      })
      .catch(() => {
        rawLyrics = null;
      });
  });

  $effect(() => {
    lines = rawLyrics ? parseLyrics(rawLyrics) : [];
  });

  // ── Moving / resizing ───────────────────────────────
  // The window is undecorated, so we provide our own hit zones. The whole
  // surface is a Tauri drag region (a Tauri 2.11 "deep" region), which means
  // the window can be grabbed anywhere over the lyrics. Eight thin overlays
  // along the border resize it; because they are real elements they also give
  // every edge and corner the right resize cursor.
  type Edge = "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw";
  const RESIZE_EDGES: Edge[] = ["n", "s", "e", "w", "ne", "nw", "se", "sw"];

  const MIN_WIDTH = 240;
  const MIN_HEIGHT = 80;

  let hovered = $state(false);
  let resize: {
    edge: Edge;
    startX: number;
    startY: number;
    x: number;
    y: number;
    w: number;
    h: number;
  } | null = null;

  async function onResizeStart(e: PointerEvent, edge: Edge) {
    if (locked || e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    const [factor, pos, size] = await Promise.all([
      appWindow.scaleFactor(),
      appWindow.outerPosition(),
      appWindow.outerSize(),
    ]);
    resize = {
      edge,
      startX: e.screenX,
      startY: e.screenY,
      x: pos.x / factor,
      y: pos.y / factor,
      w: size.width / factor,
      h: size.height / factor,
    };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  async function applyResize(e: PointerEvent) {
    if (!resize) return;
    const r = resize;
    const dx = e.screenX - r.startX;
    const dy = e.screenY - r.startY;
    let { x, y, w, h } = r;
    if (r.edge.includes("e")) w = Math.max(MIN_WIDTH, r.w + dx);
    if (r.edge.includes("s")) h = Math.max(MIN_HEIGHT, r.h + dy);
    if (r.edge.includes("w")) {
      w = Math.max(MIN_WIDTH, r.w - dx);
      x = r.x + (r.w - w);
    }
    if (r.edge.includes("n")) {
      h = Math.max(MIN_HEIGHT, r.h - dy);
      y = r.y + (r.h - h);
    }
    try {
      await appWindow.setSize(new LogicalSize(Math.round(w), Math.round(h)));
      await appWindow.setPosition(new LogicalPosition(Math.round(x), Math.round(y)));
    } catch (err) {
      console.error("resize lyrics window error:", err);
    }
  }

  function onResizeEnd(e: PointerEvent) {
    if (!resize) return;
    resize = null;
    const el = e.currentTarget as HTMLElement;
    if (el?.hasPointerCapture?.(e.pointerId)) el.releasePointerCapture(e.pointerId);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="lyrics-window"
  class:locked
  class:show-frame={pulse || (hovered && !locked)}
  data-tauri-drag-region={locked ? undefined : "deep"}
  onpointerenter={() => (hovered = true)}
  onpointerleave={() => (hovered = false)}
>
  <div class="lyrics-area">
    <LyricsView
      lines={lines}
      elapsed={player.elapsed}
      css={settingsStore.settings.desktop_lyrics_css}
      manualScroll={!locked}
    />
  </div>

  {#if pulse && lines.length === 0}
    <div class="empty-hint">
      {locked
        ? "桌面歌词已锁定 · 在设置中解锁后可拖动"
        : "暂无歌词 · 拖动窗口可调整位置"}
    </div>
  {/if}

  <!-- Eight resize overlays. `data-tauri-drag-region="false"` keeps the
       drag script from starting a window move on top of them, and each one
       owns the cursor for its edge/corner. -->
  {#if !locked}
    {#each RESIZE_EDGES as edge}
      <div
        class="resize-handle {edge}"
        data-tauri-drag-region="false"
        role="presentation"
        onpointerdown={(e) => onResizeStart(e, edge)}
        onpointermove={applyResize}
        onpointerup={onResizeEnd}
        onpointercancel={onResizeEnd}
      ></div>
    {/each}
  {/if}
</div>

<style>
  :global(html),
  :global(body) {
    /* The window itself is transparent; only the text is painted. */
    background: transparent;
    margin: 0;
    overflow: hidden;
  }

  .lyrics-window {
    position: relative;
    display: flex;
    width: 100vw;
    height: 100vh;
    background: transparent;
    /* Nothing here is text to copy — the whole surface is a control. */
    user-select: none;
    -webkit-user-select: none;
    /* The whole window is draggable, so advertise it. Edge handles below
       override this with their own resize cursor. */
    cursor: grab;
  }

  .lyrics-window.locked {
    cursor: default;
  }

  /* A frame strong enough to find over arbitrary wallpaper. It is shown while
     hovered (unlocked) and flashed for a moment whenever the window appears,
     even when locked/click-through. */
  .lyrics-window.show-frame {
    outline: 2px solid rgba(255, 255, 255, 0.85);
    outline-offset: -2px;
    background: rgba(0, 0, 0, 0.18);
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.7);
  }

  .empty-hint {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    padding: 6px 12px;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.6);
    color: rgba(255, 255, 255, 0.92);
    font-size: 12px;
    white-space: nowrap;
    pointer-events: none;
    z-index: 20;
  }

  .lyrics-area {
    flex: 1;
    min-width: 0;
    height: 100%;
  }

  /* Thin invisible strips along every edge and corner. Their only job is to
     own the resize gesture and the matching cursor. */
  .resize-handle {
    position: absolute;
    z-index: 10;
  }
  .resize-handle.n {
    top: 0;
    left: 10px;
    right: 10px;
    height: 8px;
    cursor: ns-resize;
  }
  .resize-handle.s {
    bottom: 0;
    left: 10px;
    right: 10px;
    height: 8px;
    cursor: ns-resize;
  }
  .resize-handle.e {
    top: 10px;
    bottom: 10px;
    right: 0;
    width: 8px;
    cursor: ew-resize;
  }
  .resize-handle.w {
    top: 10px;
    bottom: 10px;
    left: 0;
    width: 8px;
    cursor: ew-resize;
  }
  .resize-handle.ne {
    top: 0;
    right: 0;
    width: 12px;
    height: 12px;
    cursor: nesw-resize;
  }
  .resize-handle.nw {
    top: 0;
    left: 0;
    width: 12px;
    height: 12px;
    cursor: nwse-resize;
  }
  .resize-handle.se {
    bottom: 0;
    right: 0;
    width: 12px;
    height: 12px;
    cursor: nwse-resize;
  }
  .resize-handle.sw {
    bottom: 0;
    left: 0;
    width: 12px;
    height: 12px;
    cursor: nesw-resize;
  }
</style>
