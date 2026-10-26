<script lang="ts">
  import type { SongDetails } from "$lib/types";
  import { displayAlbum, displayArtist, displayTitle, formatDuration } from "$lib/songDisplay";

  let {
    details,
    x,
    y,
    onenter,
    onleave,
  }: {
    details: SongDetails;
    x: number;
    y: number;
    onenter?: () => void;
    onleave?: () => void;
  } = $props();

  let el = $state<HTMLDivElement | null>(null);
  let styleX = $state(0);
  let styleY = $state(0);

  $effect(() => {
    if (!el) return;
    const rect = el.getBoundingClientRect();
    let left = x + 16;
    let top = y + 16;
    if (left + rect.width > window.innerWidth - 8) {
      left = Math.max(8, x - rect.width - 16);
    }
    if (top + rect.height > window.innerHeight - 8) {
      top = Math.max(8, window.innerHeight - rect.height - 8);
    }
    styleX = left;
    styleY = top;
  });

  function formatBytes(bytes: number): string {
    const units = ["B", "KB", "MB", "GB", "TB"];
    let value = bytes;
    let index = 0;
    while (value >= 1024 && index < units.length - 1) {
      value /= 1024;
      index++;
    }
    const digits = index === 0 || value >= 100 ? 0 : value >= 10 ? 1 : 2;
    return value.toFixed(digits) + units[index];
  }

  function formatDate(unixSeconds?: number): string {
    if (!unixSeconds) return "—";
    const date = new Date(unixSeconds * 1000);
    return date.toLocaleString();
  }

  let song = $derived(details.song);
  let title = $derived(displayTitle(song));
  let artist = $derived(displayArtist(song));
  let album = $derived(displayAlbum(song));
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="hover-card"
  bind:this={el}
  style="left: {styleX}px; top: {styleY}px"
  onmouseenter={() => onenter?.()}
  onmouseleave={() => onleave?.()}
>
  <div class="card-title">{title}</div>
  <div class="card-sub">{artist || "未知艺术家"}<span class="sep">·</span>{album || "未知专辑"}</div>

  {#if details.files.length > 0}
    <div class="section">
      <div class="section-title">文件</div>
      {#each details.files as file}
        <div class="file">
          <div class="file-path">{file.path}</div>
          <div class="file-meta">
            {#if !file.exists}
              <span class="bad">文件不存在</span>
            {:else}
              <span>{file.format?.toUpperCase() || "未知格式"}</span>
              {#if file.size != null}<span>{formatBytes(file.size)}</span>{/if}
              {#if file.bitrate != null}<span>{file.bitrate} kbps</span>{/if}
              {#if file.sample_rate != null}<span>{file.sample_rate} Hz</span>{/if}
              {#if file.channels != null}<span>{file.channels} ch</span>{/if}
              {#if file.bit_depth != null}<span>{file.bit_depth} bit</span>{/if}
              {#if file.duration != null}<span>{formatDuration(file.duration) || "—"}</span>{/if}
              {#if file.modified != null}<span>{formatDate(file.modified)}</span>{/if}
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}

  {#if song.tags.length > 0}
    <div class="section">
      <div class="section-title">标签</div>
      <div class="chips">
        {#each song.tags as tag}
          <span class="chip">{tag.namespace}:{tag.value}</span>
        {/each}
      </div>
    </div>
  {/if}

  {#if song.paths.length > 0 || song.local || song.sources.length > 0}
    <div class="section">
      <div class="section-title">来源</div>
      {#if song.paths.length > 0 || song.local}
        <div class="kv"><span>平台</span><b>local</b></div>
      {/if}
      {#each song.sources as source}
        <div class="kv"><span>平台</span><b>{source.platform}:{source.id}</b></div>
      {/each}
    </div>
  {/if}

  <div class="section">
    <div class="section-title">元数据</div>
    <div class="kv"><span>原始标题</span><b>{song.title}</b></div>
    <div class="kv"><span>原始艺术家</span><b>{song.artist}</b></div>
    <div class="kv"><span>原始专辑</span><b>{song.album}</b></div>
    <div class="kv"><span>时长</span><b>{formatDuration(song.metadata.duration) || "—"}</b></div>
    <div class="kv"><span>添加时间</span><b>{song.metadata.added_time || "—"}</b></div>
    <div class="kv"><span>文件名</span><b>{song.metadata.file_name || "—"}</b></div>
    {#if song.metadata.display_title}
      <div class="kv"><span>标题覆盖</span><b>{song.metadata.display_title}</b></div>
    {/if}
    {#if song.metadata.artist_override}
      <div class="kv"><span>艺术家覆盖</span><b>{song.metadata.artist_override}</b></div>
    {/if}
    {#if song.metadata.album_override}
      <div class="kv"><span>专辑覆盖</span><b>{song.metadata.album_override}</b></div>
    {/if}
    <div class="kv"><span>歌词</span><b>{song.metadata.lyrics ? song.metadata.lyrics.length + " 字" : "—"}</b></div>
    {#if song.metadata.notes}
      <div class="kv"><span>笔记</span><b>{song.metadata.notes}</b></div>
    {/if}
    <div class="kv"><span>ID</span><b class="mono">{song.id}</b></div>
  </div>
</div>

<style>
  .hover-card {
    position: fixed;
    z-index: 450;
    width: 360px;
    max-height: 460px;
    overflow-y: auto;
    padding: 10px 12px;
    background: var(--color-base-300);
    border: 1px solid var(--panel-border-strong);
    border-radius: 8px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
    color: var(--color-base-content);
    font-size: 12px;
    cursor: text;
    user-select: text;
    -webkit-user-select: text;
  }

  .card-title {
    font-size: 13px;
    font-weight: 600;
    line-height: 1.35;
    overflow-wrap: anywhere;
  }
  .card-sub {
    margin-top: 2px;
    font-size: 11px;
    line-height: 1.4;
    color: color-mix(in oklch, var(--color-base-content) 60%, transparent);
    overflow-wrap: anywhere;
  }
  .sep { margin: 0 5px; }

  .section {
    margin-top: 8px;
    padding-top: 6px;
    border-top: 1px solid var(--panel-border);
  }
  .section-title {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.4px;
    text-transform: uppercase;
    color: var(--color-primary);
    margin-bottom: 4px;
  }

  .file + .file { margin-top: 6px; }
  .file-path {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 10.5px;
    word-break: break-all;
    color: color-mix(in oklch, var(--color-base-content) 80%, transparent);
  }
  .file-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 10px;
    margin-top: 2px;
    font-size: 10.5px;
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent);
  }
  .bad { color: var(--color-error); }

  .chips { display: flex; flex-wrap: wrap; gap: 4px; }
  .chip {
    padding: 1px 6px;
    border-radius: 4px;
    background: color-mix(in oklch, var(--color-primary) 14%, transparent);
    color: var(--color-primary);
    font-size: 10.5px;
  }

  .kv {
    display: flex;
    gap: 10px;
    line-height: 1.5;
  }
  .kv span {
    flex: none;
    width: 72px;
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent);
  }
  .kv b {
    flex: 1;
    min-width: 0;
    font-weight: 400;
    word-break: break-word;
  }
  .mono { font-family: ui-monospace, monospace; font-size: 10px; }
</style>
