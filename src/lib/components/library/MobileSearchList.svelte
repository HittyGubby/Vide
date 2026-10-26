<script lang="ts">
  import type { SearchResultItem, Tag } from "$lib/types";
  import type { SelectionStore } from "$lib/library/selection.svelte";
  import { formatDuration } from "$lib/songDisplay";
  import { MoreVertical } from "lucide-svelte";
  import VirtualMobileList from "$lib/components/library/VirtualMobileList.svelte";

  /**
   * Phone rows for online search results: cover (or index), title, a plain
   * second line with platform/artist/album/tags, duration and the ⋮ button.
   * Shares scrolling and gestures with the library list.
   */
  let {
    results,
    playingKey,
    selection,
    tagsByKey,
    platformLabel,
    coverSrc,
    onpress,
    onmenu,
    onlongpress,
    oncovererror,
    onscroll,
  }: {
    results: SearchResultItem[];
    playingKey: string | null;
    selection: SelectionStore<SearchResultItem>;
    tagsByKey: Record<string, Tag[]>;
    platformLabel: (platform: string) => string;
    /** Preferred cover URL (global proxy / referer handling lives in the page). */
    coverSrc?: (item: SearchResultItem) => string;
    onpress: (item: SearchResultItem, index: number) => void;
    onmenu: (item: SearchResultItem) => void;
    onlongpress: (item: SearchResultItem, index: number) => void;
    oncovererror?: (item: SearchResultItem, img: HTMLImageElement) => void;
    onscroll?: (top: number) => void;
  } = $props();

  function keyOf(item: SearchResultItem): string {
    return item.platform + ":" + item.source_id;
  }

  function rowClass(item: SearchResultItem): string {
    return [
      playingKey === keyOf(item) ? "playing" : "",
      selection.has(item) ? "selected" : "",
    ]
      .filter(Boolean)
      .join(" ");
  }

  function subtitle(item: SearchResultItem): string {
    const parts: string[] = [platformLabel(item.platform)];
    if (item.artist) parts.push(item.artist);
    if (item.album) parts.push(item.album);
    for (const tag of tagsByKey[keyOf(item)] ?? []) {
      if (parts.length >= 6) break;
      parts.push(tag.namespace + ":" + tag.value);
    }
    return parts.filter(Boolean).join(" · ");
  }
</script>

{#snippet row(item: SearchResultItem, i: number)}
  <div class="lead">
    {#if item.cover_url}
      <img
        class="cover"
        src={coverSrc?.(item) ?? item.cover_url}
        alt=""
        loading="lazy"
        decoding="async"
        referrerpolicy="no-referrer"
        onerror={(e) => oncovererror?.(item, e.currentTarget as HTMLImageElement)}
      />
    {:else}
      <span class="index">{i + 1}</span>
    {/if}
  </div>

  <div class="info">
    <span class="title">{item.title}</span>
    <span class="sub">{subtitle(item)}</span>
  </div>

  <span class="time">{item.duration ? formatDuration(item.duration) : ""}</span>

  <button
    class="more"
    aria-label="更多操作"
    onclick={(e) => {
      e.stopPropagation();
      onmenu(item);
    }}
  >
    <MoreVertical size={17} />
  </button>
{/snippet}

<VirtualMobileList
  items={results}
  rowHeight={56}
  {keyOf}
  {rowClass}
  {onpress}
  {onlongpress}
  {onscroll}
  row={row}
/>

<style>
  .lead {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 44px;
    height: 100%;
  }

  .cover {
    width: 38px;
    height: 38px;
    border-radius: 5px;
    object-fit: cover;
    background: var(--color-base-300);
  }

  .index {
    font-size: 11px;
    color: color-mix(in oklch, var(--color-base-content) 45%, transparent);
    font-variant-numeric: tabular-nums;
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
