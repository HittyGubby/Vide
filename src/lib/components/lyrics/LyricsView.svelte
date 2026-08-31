<script lang="ts">
  import { settingsStore } from "$lib/stores/settings.svelte";
  import {
    activeLyricIndex,
    scrollActiveLineIntoView,
    type LyricLine,
  } from "$lib/lyrics";

  interface Props {
    lines: LyricLine[];
    elapsed: number;
    /** Stylesheet for the lyrics area (user-authored, hot-reloaded). */
    css: string;
    /** Let the user drag the lyrics to scroll (floating window only). */
    manualScroll?: boolean;
    /** Overrides the configured position of the active line. */
    position?: string;
  }

  let { lines, elapsed, css, manualScroll = false, position }: Props = $props();

  let activeIndex = $derived(activeLyricIndex(lines, elapsed));
  let resolvedPosition = $derived(position ?? settingsStore.settings.desktop_lyrics_position);

  let viewportEl = $state<HTMLDivElement | null>(null);
  let linesEl = $state<HTMLDivElement | null>(null);

  // Dragging the lyrics scrolls them; auto-scroll backs off for a moment so
  // the two do not fight each other.
  const MANUAL_SCROLL_PAUSE_MS = 3000;
  let lastManualScrollAt = 0;
  let dragScroll: { startY: number; startTop: number } | null = null;

  function onScrollPointerDown(e: PointerEvent) {
    if (!manualScroll || !viewportEl) return;
    dragScroll = { startY: e.clientY, startTop: viewportEl.scrollTop };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function onScrollPointerMove(e: PointerEvent) {
    if (!dragScroll || !viewportEl) return;
    viewportEl.scrollTop = dragScroll.startTop - (e.clientY - dragScroll.startY);
    lastManualScrollAt = Date.now();
  }

  function onScrollPointerUp(e: PointerEvent) {
    if (!dragScroll) return;
    dragScroll = null;
    const el = e.currentTarget as HTMLElement;
    if (el?.hasPointerCapture?.(e.pointerId)) el.releasePointerCapture(e.pointerId);
  }

  // Auto-scrolling needs real geometry, so it is the one behaviour kept in JS
  // rather than in the user stylesheet.
  $effect(() => {
    const index = activeIndex;
    const pos = resolvedPosition;
    if (index < 0 || !viewportEl || !linesEl) return;
    if (Date.now() - lastManualScrollAt < MANUAL_SCROLL_PAUSE_MS) return;
    scrollActiveLineIntoView(
      viewportEl,
      linesEl.children[index] as HTMLElement | undefined,
      pos as "top" | "center" | "bottom",
    );
  });
</script>

<!-- The stylesheet is injected globally on purpose: the same string is edited
     in the settings and applied here, wherever this component is used. -->
{@html "<style>" + css + "</style>"}

<div class="lyrics-root" data-position={resolvedPosition}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="lyrics-viewport"
    class:manual={manualScroll}
    bind:this={viewportEl}
    onpointerdown={onScrollPointerDown}
    onpointermove={onScrollPointerMove}
    onpointerup={onScrollPointerUp}
    onpointercancel={onScrollPointerUp}
  >
    <div class="lyrics-lines" bind:this={linesEl}>
      {#each lines as line, i}
        <div class="line" class:playing={i === activeIndex}>
          {#each line.parts as part}
            <p class={part.lang}>{part.text}</p>
          {/each}
        </div>
      {/each}
    </div>
  </div>
</div>