<script lang="ts" generics="T">
  import { virtualSlice } from "$lib/virtual";
  import type { Snippet } from "svelte";

  /**
   * Scroll + virtualization + touch gestures for a phone list.
   *
   * The row shell (height, highlight states, tap/long-press) lives here; the
   * content is a snippet, so the library and the online-search list share the
   * scrolling behaviour without sharing their cell layout.
   */
  let {
    items,
    rowHeight = 56,
    keyOf,
    rowClass,
    onpress,
    onlongpress,
    onscroll,
    onreachbottom,
    row,
  }: {
    items: T[];
    rowHeight?: number;
    keyOf: (item: T) => string;
    rowClass?: (item: T) => string;
    onpress?: (item: T, index: number) => void;
    onlongpress?: (item: T, index: number) => void;
    onscroll?: (top: number) => void;
    /** Fired when the scroll position is close to the end (infinite scroll). */
    onreachbottom?: () => void;
    row: Snippet<[T, number]>;
  } = $props();

  const LONG_PRESS_MS = 450;

  let el = $state<HTMLDivElement | null>(null);
  let top = $state(0);
  let height = $state(0);
  let slice = $derived(virtualSlice(items.length, rowHeight, top, height));

  let pressTimer: ReturnType<typeof setTimeout> | undefined;
  let pressX = 0;
  let pressY = 0;
  let suppressClick = false;

  $effect(() => {
    const node = el;
    if (!node) return;
    height = node.clientHeight || 0;
    top = node.scrollTop;
  });

  function handleScroll() {
    if (!el) return;
    top = el.scrollTop;
    height = el.clientHeight || height;
    onscroll?.(top);
    if (onreachbottom && el.scrollHeight - el.scrollTop - el.clientHeight < 240) {
      onreachbottom();
    }
  }

  export function scrollToOffset(value: number) {
    if (!el) return;
    el.scrollTop = Math.max(0, value);
    top = el.scrollTop;
    height = el.clientHeight || height;
    onscroll?.(top);
  }

  export function scrollToIndex(index: number) {
    if (!el || index < 0) return;
    scrollToOffset(index * rowHeight - el.clientHeight / 2 + rowHeight / 2);
  }

  function startPress(e: PointerEvent, item: T, index: number) {
    // Only touch/pen long-press; the desktop keeps Ctrl/Shift selection.
    if (e.pointerType === "mouse") return;
    if ((e.target as HTMLElement).closest("button")) return;
    pressX = e.clientX;
    pressY = e.clientY;
    cancelPress();
    pressTimer = setTimeout(() => {
      pressTimer = undefined;
      suppressClick = true;
      onlongpress?.(item, index);
      // Safety net: never swallow the next real tap.
      setTimeout(() => (suppressClick = false), 700);
    }, LONG_PRESS_MS);
  }

  function movePress(e: PointerEvent) {
    if (!pressTimer) return;
    if (Math.abs(e.clientX - pressX) > 10 || Math.abs(e.clientY - pressY) > 10) {
      cancelPress();
    }
  }

  function cancelPress() {
    if (pressTimer) {
      clearTimeout(pressTimer);
      pressTimer = undefined;
    }
  }

  function handleClick(item: T, index: number) {
    if (suppressClick) {
      suppressClick = false;
      return;
    }
    onpress?.(item, index);
  }
</script>

<div class="vlist" bind:this={el} onscroll={handleScroll} role="list">
  <div style="height: {slice.padTop}px"></div>
  {#each items.slice(slice.start, slice.end) as item, k (keyOf(item))}
    {@const i = slice.start + k}
    {@const state = rowClass?.(item) ?? ""}
    <div
      class="vrow"
      class:playing={state.includes("playing")}
      class:selected={state.includes("selected")}
      class:located={state.includes("located")}
      role="button"
      tabindex="0"
      style="height: {rowHeight}px"
      onclick={() => handleClick(item, i)}
      onkeydown={(e) => {
        if (e.key === "Enter") handleClick(item, i);
      }}
      onpointerdown={(e) => startPress(e, item, i)}
      onpointermove={movePress}
      onpointerup={cancelPress}
      onpointercancel={cancelPress}
      onpointerleave={cancelPress}
      oncontextmenu={(e) => e.preventDefault()}
    >
      {@render row(item, i)}
    </div>
  {/each}
  <div style="height: {slice.padBottom}px"></div>
</div>

<style>
  .vlist {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    overflow-anchor: none;
    overscroll-behavior: contain;
    -webkit-tap-highlight-color: transparent;
  }

  .vrow {
    display: flex;
    align-items: center;
    gap: 6px;
    padding-right: 2px;
    border-bottom: 1px solid color-mix(in oklch, var(--color-base-content) 7%, transparent);
    color: var(--color-base-content);
    cursor: pointer;
    user-select: none;
  }

  .vrow:active {
    background: var(--color-base-300);
  }

  .vrow.playing {
    background: color-mix(in oklch, var(--color-primary) 10%, transparent);
    color: var(--color-primary);
  }

  .vrow.selected {
    background: color-mix(in oklch, var(--color-primary) 14%, transparent);
  }

  .vrow.located {
    background: color-mix(in oklch, var(--color-primary) 20%, transparent);
  }
</style>
