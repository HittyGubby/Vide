<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
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

  const appWindow: Window = getCurrentWindow();

  let lines = $state<LyricLine[]>([]);
  let rawLyrics = $state<string | null>(null);
  let locked = $derived(settingsStore.settings.desktop_lyrics_locked);

  // The desktop window has no player bar, so it polls the backend itself.
  onMount(() => {
    void settingsStore.load();
    void player.poll();
    const timer = setInterval(() => void player.poll(), 500);
    // Re-read the settings whenever the main window saves them, so CSS edits
    // show up here immediately.
    const unlisten = listen("vide:settings-changed", () => void settingsStore.load());
    return () => {
      clearInterval(timer);
      void unlisten.then((fn) => fn());
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
  // The window is undecorated, so we provide our own hit zones: the gutters
  // beside the lyrics drag the window, the outer strip resizes it, and the
  // lyrics themselves scroll instead.
  const EDGE = 8;
  const CURSORS: Record<string, string> = {
    n: "ns-resize",
    s: "ns-resize",
    e: "ew-resize",
    w: "ew-resize",
    ne: "nesw-resize",
    nw: "nwse-resize",
    se: "nwse-resize",
    sw: "nesw-resize",
  };

  let hovered = $state(false);
  let cursor = $state("default");
  let resize: {
    edge: string;
    startX: number;
    startY: number;
    x: number;
    y: number;
    w: number;
    h: number;
  } | null = null;

  function edgeAt(e: PointerEvent): string {
    const el = e.currentTarget as HTMLElement;
    const rect = el.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    let edge = "";
    if (y <= EDGE) edge += "n";
    else if (y >= rect.height - EDGE) edge += "s";
    if (x <= EDGE) edge += "w";
    else if (x >= rect.width - EDGE) edge += "e";
    return edge;
  }

  function onPointerMove(e: PointerEvent) {
    if (resize) {
      void applyResize(e);
      return;
    }
    if (locked) {
      cursor = "default";
      return;
    }
    cursor = CURSORS[edgeAt(e)] ?? "default";
  }

  async function onPointerDown(e: PointerEvent) {
    if (locked) return;
    const edge = edgeAt(e);
    if (!edge) return;
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
    if (r.edge.includes("e")) w = Math.max(240, r.w + dx);
    if (r.edge.includes("s")) h = Math.max(80, r.h + dy);
    if (r.edge.includes("w")) {
      w = Math.max(240, r.w - dx);
      x = r.x + (r.w - w);
    }
    if (r.edge.includes("n")) {
      h = Math.max(80, r.h - dy);
      y = r.y + (r.h - h);
    }
    try {
      await appWindow.setSize(new LogicalSize(Math.round(w), Math.round(h)));
      await appWindow.setPosition(new LogicalPosition(Math.round(x), Math.round(y)));
    } catch (err) {
      console.error("resize lyrics window error:", err);
    }
  }

  function onPointerUp(e: PointerEvent) {
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
  class:show-frame={hovered && !locked}
  style="cursor: {cursor}"
  onpointermove={onPointerMove}
  onpointerdown={onPointerDown}
  onpointerup={onPointerUp}
  onpointercancel={onPointerUp}
  onpointerenter={() => (hovered = true)}
  onpointerleave={() => {
    hovered = false;
    resize = null;
  }}
>
  <!-- Side gutters: dragging these moves the window. -->
  <div class="gutter" data-tauri-drag-region={locked ? undefined : ""}></div>
  <div class="lyrics-area">
    <LyricsView
      lines={lines}
      elapsed={player.elapsed}
      css={settingsStore.settings.desktop_lyrics_css}
      manualScroll={true}
    />
  </div>
  <div class="gutter" data-tauri-drag-region={locked ? undefined : ""}></div>
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
    display: flex;
    width: 100vw;
    height: 100vh;
    background: transparent;
    /* Nothing here is text to copy — the whole surface is a control. */
    user-select: none;
    -webkit-user-select: none;
  }

  /* Only while unlocked and hovered: a thin frame so the user can see where
     the (otherwise invisible) window edges are. */
  .lyrics-window.show-frame {
    outline: 1px solid rgba(0, 0, 0, 0.8);
    outline-offset: -1px;
  }

  .gutter {
    flex: 0 0 28px;
    align-self: stretch;
    cursor: grab;
  }
  .gutter:active {
    cursor: grabbing;
  }
  .lyrics-window.locked .gutter {
    cursor: default;
  }

  .lyrics-area {
    flex: 1;
    min-width: 0;
    height: 100%;
  }
</style>