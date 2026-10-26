<script lang="ts">
  import { settingsStore } from "$lib/stores/settings.svelte";
  import {
    activeLyricIndex,
    normalizeCssColors,
    scrollActiveLineIntoView,
    type LyricLine,
  } from "$lib/lyrics";

  interface Props {
    lines: LyricLine[];
    elapsed: number;
    /** Stylesheet for the lyrics area (user-authored, hot-reloaded). */
    css: string;
    /** Let the user scroll the lyrics manually (detail view and floating window). */
    manualScroll?: boolean;
    /** Clicking a line highlights it without changing playback position. */
    selectable?: boolean;
    /** Overrides the configured position of the active line. */
    position?: string;
  }

  let {
    lines,
    elapsed,
    css,
    manualScroll = false,
    selectable = true,
    position,
  }: Props = $props();

  let activeIndex = $derived(activeLyricIndex(lines, elapsed));
  let resolvedPosition = $derived(position ?? settingsStore.settings.desktop_lyrics_position);

  let viewportEl = $state<HTMLDivElement | null>(null);
  let linesEl = $state<HTMLDivElement | null>(null);
  let selectedIndex = $state(-1);

  // Manual scrolling pauses auto-follow for 2s, then the active line is
  // brought back into view. The user can still select a line (copy it or keep
  // it highlighted) without changing the playback position.
  const MANUAL_SCROLL_PAUSE_MS = 2000;
  let lastManualScrollAt = 0;
  let manualTimer: ReturnType<typeof setTimeout> | undefined;
  let ignoreScroll = false;

  function scrollToActive() {
    if (activeIndex < 0 || !viewportEl || !linesEl) return;
    ignoreScroll = true;
    scrollActiveLineIntoView(
      viewportEl,
      linesEl.children[activeIndex] as HTMLElement | undefined,
      resolvedPosition as "top" | "center" | "bottom",
    );
    setTimeout(() => (ignoreScroll = false), 350);
  }

  function markManualScroll() {
    lastManualScrollAt = Date.now();
    if (manualTimer) clearTimeout(manualTimer);
    manualTimer = setTimeout(() => {
      lastManualScrollAt = 0;
      scrollToActive();
    }, MANUAL_SCROLL_PAUSE_MS);
  }

  function onWheel(e: WheelEvent) {
    if (!manualScroll || !viewportEl) return;
    e.preventDefault();
    viewportEl.scrollTop += e.deltaY;
    markManualScroll();
  }

  function onScroll() {
    if (!manualScroll || ignoreScroll) return;
    markManualScroll();
  }

  function selectLine(index: number) {
    if (!selectable) return;
    selectedIndex = selectedIndex === index ? -1 : index;
  }

  // Auto-scrolling needs real geometry, so it is the one behaviour kept in JS
  // rather than in the user stylesheet.
  $effect(() => {
    const index = activeIndex;
    if (index < 0 || !viewportEl || !linesEl) return;
    if (Date.now() - lastManualScrollAt < MANUAL_SCROLL_PAUSE_MS) return;
    scrollToActive();
  });
</script>

<!-- The stylesheet is injected globally on purpose: the same string is edited
     in the settings and applied here, wherever this component is used. -->
{@html "<style>" + normalizeCssColors(css) + "</style>"}

<div class="lyrics-root" data-position={resolvedPosition}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="lyrics-viewport"
    class:manual={manualScroll}
    bind:this={viewportEl}
    onwheel={onWheel}
    onscroll={onScroll}
  >
    <div class="lyrics-lines" bind:this={linesEl}>
      {#each lines as line, i}
        <div
          class="line"
          class:playing={i === activeIndex}
          class:selected={i === selectedIndex}
          class:selectable
          role={selectable ? "button" : undefined}
          tabindex={selectable ? 0 : undefined}
          onclick={() => selectLine(i)}
          onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") selectLine(i); }}
        >
          {#each line.parts as part}
            <p class={part.lang}>{part.text}</p>
          {/each}
        </div>
      {/each}
    </div>
  </div>
</div>

<style>
  /* Manual scrolling is only enabled in views that expose it. !important
     overrides the user stylesheet's default overflow:hidden. */
  .lyrics-viewport.manual {
    overflow-y: auto !important;
    scrollbar-width: thin;
  }

  .line.selectable {
    cursor: pointer;
  }

  .line.selected {
    background: color-mix(in oklch, var(--color-primary) 14%, transparent);
    border-radius: 6px;
  }

  .line {
    user-select: text;
  }
</style>
