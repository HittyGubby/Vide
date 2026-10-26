<script lang="ts">
  import type { Song } from "$lib/types";
  import type { SelectionStore } from "$lib/library/selection.svelte";
  import { virtualSlice } from "$lib/virtual";
  import { displayText, displayTitle, formatDuration } from "$lib/songDisplay";
  import { MoreVertical } from "lucide-svelte";

  /**
   * The phone equivalent of the desktop table.
   *
   * Rows are 56px, two lines: title plus a plain-text subtitle (source,
   * artist, album, then up to three tags — no pills, they eat the width).
   * Tap plays, long-press enters selection, the ⋮ button opens the action
   * sheet. Scrolling and virtualization live here so the page only deals with
   * data and actions.
   */
  const ROW_H = 56;
  const LONG_PRESS_MS = 450;

  let {
    songs,
    playingId,
    locateId,
    selection,
    onplay,
    onmenu,
    onlongpress,
    onscroll,
  }: {
    songs: Song[];
    playingId: string | null;
    locateId: string | null;
    selection: SelectionStore<Song>;
    onplay: (song: Song) => void;
    onmenu: (song: Song) => void;
    onlongpress: (song: Song, index: number) => void;
    onscroll?: (top: number) => void;
  } = $props();

  let el = $state<HTMLDivElement | null>(null);
  let top = $state(0);
  let height = $state(0);
  let slice = $derived(virtualSlice(songs.length, ROW_H, top, height));

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
  }

  export function scrollToOffset(value: number) {
    if (!el) return;
    el.scrollTop = Math.max(0, value);
    top = el.scrollTop;
    height = el.clientHeight || height;
  }

  export function scrollToSong(id: string) {
    if (!el) return;
    const index = songs.findIndex((song) => song.id === id);
    if (index < 0) return;
    scrollToOffset(index * ROW_H - el.clientHeight / 2 + ROW_H / 2);
    onscroll?.(top);
  }

  function startPress(e: PointerEvent, song: Song, index: number) {
    // Only touch/pen long-press; the desktop keeps Ctrl/Shift selection.
    if (e.pointerType === "mouse") return;
    if ((e.target as HTMLElement).closest("button")) return;
    pressX = e.clientX;
    pressY = e.clientY;
    cancelPress();
    pressTimer = setTimeout(() => {
      pressTimer = undefined;
      suppressClick = true;
      onlongpress(song, index);
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

  function handleClick(song: Song, index: number) {
    if (suppressClick) {
      suppressClick = false;
      return;
    }
    if (selection.size > 0) selection.toggle(song, index);
    else onplay(song);
  }

  function subtitle(song: Song): string {
    const parts: string[] = [];
    // The server strips absolute paths for privacy and sets `local` instead.
    const source =
      song.paths.length > 0 || song.local ? "本地" : song.sources[0]?.platform;
    if (source) parts.push(source);
    const artist = displayText(song, "artist");
    const album = displayText(song, "album");
    if (artist) parts.push(artist);
    if (album) parts.push(album);
    for (const tag of song.tags.slice(0, 3)) {
      parts.push(tag.namespace + ":" + tag.value);
    }
    return parts.join(" · ");
  }
</script>

<div class="mobile-song-list" bind:this={el} onscroll={handleScroll} role="list">
  <div style="height: {slice.padTop}px"></div>
  {#each songs.slice(slice.start, slice.end) as song, k (song.id)}
    {@const i = slice.start + k}
    <div
      class="row"
      class:playing={playingId === song.id}
      class:selected={selection.has(song)}
      class:located={locateId === song.id}
      role="button"
      tabindex="0"
      style="height: {ROW_H}px"
      onclick={() => handleClick(song, i)}
      onkeydown={(e) => {
        if (e.key === "Enter") handleClick(song, i);
      }}
      onpointerdown={(e) => startPress(e, song, i)}
      onpointermove={movePress}
      onpointerup={cancelPress}
      onpointercancel={cancelPress}
      onpointerleave={cancelPress}
      oncontextmenu={(e) => e.preventDefault()}
    >
      <div class="lead">
        {#if selection.size > 0}
          <input
            type="checkbox"
            class="check"
            checked={selection.has(song)}
            onclick={(e) => e.stopPropagation()}
            onchange={(e) => selection.setChecked(song, e.currentTarget.checked, i)}
          />
        {:else if playingId === song.id}
          <span class="playing-mark">&#9654;</span>
        {:else}
          <span class="index">{i + 1}</span>
        {/if}
      </div>

      <div class="info">
        <span class="title">{displayTitle(song)}</span>
        <span class="sub">{subtitle(song)}</span>
      </div>

      <span class="time">{formatDuration(song.metadata.duration)}</span>

      <button
        class="more"
        aria-label="更多操作"
        onclick={(e) => {
          e.stopPropagation();
          onmenu(song);
        }}
      >
        <MoreVertical size={17} />
      </button>
    </div>
  {/each}
  <div style="height: {slice.padBottom}px"></div>
</div>

<style>
  .mobile-song-list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    overflow-anchor: none;
    overscroll-behavior: contain;
    -webkit-tap-highlight-color: transparent;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding-right: 2px;
    border-bottom: 1px solid color-mix(in oklch, var(--color-base-content) 7%, transparent);
    color: var(--color-base-content);
    cursor: pointer;
    user-select: none;
  }

  .row:active {
    background: var(--color-base-300);
  }

  .row.playing {
    background: color-mix(in oklch, var(--color-primary) 10%, transparent);
  }
  .row.playing .title {
    color: var(--color-primary);
  }

  .row.selected {
    background: color-mix(in oklch, var(--color-primary) 14%, transparent);
  }

  .row.located {
    background: color-mix(in oklch, var(--color-primary) 20%, transparent);
  }

  .lead {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 100%;
  }

  .index,
  .playing-mark {
    font-size: 11px;
    color: color-mix(in oklch, var(--color-base-content) 45%, transparent);
    font-variant-numeric: tabular-nums;
  }

  .playing-mark {
    color: var(--color-primary);
    font-size: 10px;
  }

  .check {
    width: 16px;
    height: 16px;
    accent-color: var(--color-primary);
  }

  .info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 6px 0;
  }

  .title {
    font-size: 13px;
    line-height: 1.25;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sub {
    font-size: 11px;
    line-height: 1.25;
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .time {
    flex: none;
    width: 38px;
    text-align: right;
    font-size: 11px;
    color: color-mix(in oklch, var(--color-base-content) 50%, transparent);
    font-variant-numeric: tabular-nums;
  }

  .more {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 100%;
    border: 0;
    background: none;
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent);
    cursor: pointer;
  }

  .more:active {
    color: var(--color-primary);
  }
</style>
