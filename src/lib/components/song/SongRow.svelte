<script lang="ts">
  import type { Source, Tag } from "$lib/types";

  export interface SongRowData {
    id: string;
    title: string;
    artist: string;
    album: string;
    duration?: number;
    cover?: string;
    tags?: Tag[];
    sources?: Source[];
    local?: boolean;
    platform?: string;
  }

  export interface SongRowColumn {
    id: string;
    style: string;
  }

  let {
    data,
    columns,
    index = 0,
    selected = false,
    selectionMode = false,
    playing = false,
    showCoverInIndex = false,
    height = 40,
    onclick,
    oncontext,
    onselect,
    oncovererror,
  }: {
    data: SongRowData;
    columns: SongRowColumn[];
    index?: number;
    selected?: boolean;
    selectionMode?: boolean;
    playing?: boolean;
    showCoverInIndex?: boolean;
    height?: number;
    onclick?: (event: MouseEvent) => void;
    oncontext?: (event: MouseEvent) => void;
    onselect?: (checked: boolean) => void;
    oncovererror?: (event: Event) => void;
  } = $props();

  function sourceLabel(): { label: string; local: boolean } | null {
    if (data.local) return { label: "local", local: true };
    const source = data.sources?.[0];
    if (source) return { label: source.platform, local: false };
    if (data.platform) return { label: data.platform, local: false };
    return null;
  }

  function formatDuration(seconds?: number): string {
    if (!seconds || seconds <= 0) return "";
    return Math.floor(seconds / 60) + ":" + String(Math.floor(seconds % 60)).padStart(2, "0");
  }
</script>

<div
  class="song-row"
  class:playing
  style="height:{height}px"
  role="button"
  tabindex="0"
  onclick={(event) => onclick?.(event)}
  onkeydown={(event) => { if (event.key === "Enter") onclick?.(event as unknown as MouseEvent); }}
  oncontextmenu={(event) => oncontext?.(event)}
>
  {#each columns as column (column.id)}
    <div class="cell td" style={column.style}>
      {#if column.id === "index" || column.id === "num"}
        {#if selectionMode}
          <input
            type="checkbox"
            class="row-check"
            checked={selected}
            onclick={(event) => event.stopPropagation()}
            onchange={(event) => onselect?.(event.currentTarget.checked)}
          />
        {:else if playing}
          <span class="playing-indicator">&#9654;</span>
        {:else if showCoverInIndex && data.cover}
          <img class="cover" src={data.cover} alt="" loading="lazy" referrerpolicy="no-referrer" onerror={oncovererror} />
        {:else}
          <span class="cell-num">{index + 1}</span>
        {/if}
      {:else if column.id === "title" || column.id === "name"}
        <div class="title-cell">
          <span class="title truncate">{data.title}</span>
          {#if sourceLabel()}
            <span class="badge" class:local={sourceLabel()!.local}>{sourceLabel()!.label}</span>
          {/if}
        </div>
      {:else if column.id === "tag"}
        <div class="tag-cell">
          {#each data.tags ?? [] as tag}
            <span class="pill">{tag.namespace}:{tag.value}</span>
          {/each}
        </div>
      {:else if column.id === "artist"}
        <span class="text truncate">{data.artist}</span>
      {:else if column.id === "album"}
        <span class="text truncate">{data.album}</span>
      {:else if column.id === "source"}
        <span class="text truncate">{sourceLabel()?.label ?? ""}</span>
      {:else if column.id === "duration" || column.id === "time"}
        <span class="time">{formatDuration(data.duration)}</span>
      {/if}
    </div>
  {/each}
</div>

<style>
  .song-row { display: flex; align-items: center; padding: 0 8px; cursor: pointer; border-radius: 4px; transition: background .1s; }
  .song-row:hover { background: var(--color-base-300) / 0.6; }
  .song-row.playing { background: var(--color-primary) / 0.12; }
  .cell { display: flex; align-items: center; min-width: 0; height: 100%; overflow: hidden; }
  .title-cell { display: flex; align-items: center; gap: 6px; min-width: 0; width: 100%; }
  .title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .text { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; color: var(--color-base-content) / 0.7; }
  .time { font-size: 11px; color: var(--color-base-content) / 0.55; margin-left: auto; }
  .cell-num { color: var(--color-base-content) / 0.5; font-size: 12px; margin: auto; }
  .playing-indicator { color: var(--color-primary); font-size: 10px; margin: auto; }
  .row-check { width: 13px; height: 13px; accent-color: var(--color-primary); cursor: pointer; margin: auto; }
  .cover { width: 30px; height: 30px; border-radius: 3px; object-fit: cover; margin: auto; }
  .badge { flex: none; font-size: 10px; padding: 1px 5px; border-radius: 4px; background: var(--color-primary) / 0.15; color: var(--color-primary); }
  .badge.local { background: var(--color-success) / 0.15; color: var(--color-success); }
  .tag-cell { display: flex; gap: 4px; overflow: hidden; align-items: center; }
  .pill { flex: none; font-size: 10px; padding: 1px 5px; border-radius: 4px; background: var(--color-base-300); color: var(--color-base-content) / 0.7; }
</style>
