<script lang="ts">
  import type { Song } from "$lib/types";
  import type { SelectionStore } from "$lib/library/selection.svelte";
  import { displayText, displayTitle, formatDuration } from "$lib/songDisplay";
  import { MoreVertical } from "lucide-svelte";
  import VirtualMobileList from "$lib/components/library/VirtualMobileList.svelte";

  /**
   * The phone equivalent of the desktop table.
   *
   * Rows are 56px, two lines: title plus a plain-text subtitle (source,
   * artist, album, then up to three tags — no pills, they eat the width).
   * Tap plays, long-press enters selection, the ⋮ button opens the action
   * sheet. Scrolling/virtualization/gestures come from VirtualMobileList.
   */
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

  let list = $state<{
    scrollToOffset: (top: number) => void;
    scrollToIndex: (index: number) => void;
  } | null>(null);

  export function scrollToOffset(top: number) {
    list?.scrollToOffset(top);
  }

  export function scrollToSong(id: string) {
    const index = songs.findIndex((song) => song.id === id);
    if (index >= 0) list?.scrollToIndex(index);
  }

  function rowClass(song: Song): string {
    return [
      playingId === song.id ? "playing" : "",
      selection.has(song) ? "selected" : "",
      locateId === song.id ? "located" : "",
    ]
      .filter(Boolean)
      .join(" ");
  }

  function handlePress(song: Song, index: number) {
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

{#snippet row(song: Song, i: number)}
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
{/snippet}

<VirtualMobileList
  items={songs}
  rowHeight={56}
  keyOf={(song) => song.id}
  {rowClass}
  onpress={handlePress}
  {onlongpress}
  {onscroll}
  bind:this={list}
  row={row}
/>

<style>
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
