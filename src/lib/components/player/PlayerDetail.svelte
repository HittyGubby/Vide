<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { player } from "$lib/stores/player.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import {
    Play,
    Pause,
    SkipBack,
    SkipForward,
    Music,
    Tag,
  } from "lucide-svelte";

  let pollTimer: ReturnType<typeof setInterval> | undefined;

  onMount(() => {
    player.poll();
    pollTimer = setInterval(() => player.poll(), 1000);
    return () => clearInterval(pollTimer);
  });

  let elapsedFormatted = $derived(formatTime(player.elapsed));
  let durationFormatted = $derived(formatTime(player.duration));
  let progressPct = $derived(
    player.duration > 0
      ? Math.min((player.elapsed / player.duration) * 100, 100)
      : 0,
  );

  function formatTime(secs: number): string {
    if (secs <= 0) return "0:00";
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  async function togglePlay() {
    if (player.isPlaying) {
      try {
        await invoke("stop_playback");
        player.isPlaying = false;
      } catch (e) {
        console.error("stop error:", e);
      }
    }
  }

  function close() {
    ui.showPlayerDetail = false;
  }

  // Tags display will be added when player store provides tag data
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="detail-overlay"
  onkeydown={(e) => {
    if (e.key === "Escape") close();
  }}
  role="presentation"
>
  <!-- Top: Album + Lyrics -->
  <div class="detail-top">
    <!-- Left: Album art + song info -->
    <div class="album-section">
      <div class="album-art-large">
        <Music size={64} class="album-placeholder-icon" />
      </div>
      <h1 class="detail-title">{player.title}</h1>
      <p class="detail-artist">{player.artist}</p>
      <!-- Tags will be shown when backend returns tag data -->
    </div>

    <!-- Right: Lyrics -->
    <div class="lyrics-section">
      <div class="lyrics-placeholder">
        <span class="lyrics-icon">𝅘𝅥𝅮</span>
        <p class="lyrics-text">歌词功能即将支持</p>
        <p class="lyrics-sub">需要插件系统提供歌词源</p>
      </div>
    </div>
  </div>

  <!-- Advanced settings -->
  <div class="advanced-settings">
    <button class="setting-chip" disabled>
      <span>速度</span>
      <span class="chip-value">1.0x</span>
    </button>
    <button class="setting-chip" disabled>
      <span>均衡器</span>
      <span class="chip-value">默认</span>
    </button>
    <button class="setting-chip" disabled>
      <span>提供者</span>
      <span class="chip-value">yt-dlp</span>
    </button>
    <button class="setting-chip" disabled>
      <span>音质</span>
      <span class="chip-value">最佳</span>
    </button>
  </div>

  <!-- Bottom: Progress + Controls on one row -->
  <div class="detail-bottom">
    <!-- Progress bar (left) -->
    <div class="progress-section">
      <div class="progress-track-large">
        <div class="progress-fill-large" style="width: {progressPct}%"></div>
      </div>
      <span class="detail-time">{elapsedFormatted} / {durationFormatted}</span>
    </div>

    <!-- Play controls (right) -->
    <div class="detail-controls">
      <button
        class="ctrl-btn-large"
        title="上一首"
        aria-label="上一首"
        disabled
      >
        <SkipBack size={20} />
      </button>
      <button
        class="ctrl-btn-large play-btn-large"
        onclick={togglePlay}
        aria-label="播放/暂停"
      >
        {#if player.isPlaying}
          <Pause size={24} fill="currentColor" />
        {:else}
          <Play size={24} fill="currentColor" />
        {/if}
      </button>
      <button
        class="ctrl-btn-large"
        title="下一首"
        aria-label="下一首"
        disabled
      >
        <SkipForward size={20} />
      </button>
    </div>
  </div>
</div>

<style>
  .detail-overlay {
    position: fixed;
    top: var(--titlebar-height);
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 300;
    background: var(--bg-base);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  /* ── Top Section ──────────────────────────────────── */
  .detail-top {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 60px;
    padding: 40px 60px;
    min-height: 0;
  }

  /* ── Album Art ────────────────────────────────────── */
  .album-section {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    max-width: 400px;
  }

  .album-art-large {
    width: 240px;
    height: 240px;
    border-radius: 12px;
    background: var(--bg-elevated);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .album-placeholder-icon {
    color: var(--text-tertiary);
    opacity: 0.5;
  }

  .detail-title {
    font-size: 22px;
    font-weight: 600;
    color: var(--text-primary);
    text-align: center;
    line-height: 1.2;
  }

  .detail-artist {
    font-size: 15px;
    color: var(--text-secondary);
    text-align: center;
  }

  .detail-tags {
    font-size: 12px;
    color: var(--accent-primary);
    text-align: center;
    opacity: 0.8;
  }

  /* ── Lyrics ───────────────────────────────────────── */
  .lyrics-section {
    flex: 1;
    display: flex;
    flex-direction: column;
    justify-content: center;
    overflow-y: auto;
    max-width: 400px;
    max-height: 100%;
  }

  .lyrics-placeholder {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    color: var(--text-tertiary);
    text-align: center;
  }

  .lyrics-icon {
    font-size: 48px;
    opacity: 0.3;
  }

  .lyrics-text {
    font-size: 14px;
    font-weight: 500;
    color: var(--text-secondary);
  }

  .lyrics-sub {
    font-size: 11px;
  }

  /* ── Advanced Settings ────────────────────────────── */
  .advanced-settings {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 12px 40px 8px;
    flex-shrink: 0;
  }

  .setting-chip {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px;
    border-radius: 16px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    color: var(--text-secondary);
    font-size: 11px;
    cursor: default;
    opacity: 0.7;
  }

  .chip-value {
    color: var(--text-tertiary);
    font-weight: 500;
  }

  /* ── Bottom Row ───────────────────────────────────── */
  .detail-bottom {
    display: flex;
    align-items: center;
    gap: 24px;
    padding: 12px 40px 24px;
    flex-shrink: 0;
  }

  .progress-section {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .progress-track-large {
    flex: 1;
    height: 6px;
    background: var(--bg-elevated);
    border-radius: 3px;
    overflow: hidden;
    cursor: pointer;
  }

  .progress-fill-large {
    height: 100%;
    background: var(--accent-primary);
    border-radius: 3px;
    transition: width 0.5s linear;
  }

  .detail-time {
    font-size: 13px;
    color: var(--text-tertiary);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    min-width: 90px;
    text-align: right;
  }

  /* ── Controls ─────────────────────────────────────── */
  .detail-controls {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .ctrl-btn-large {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 44px;
    height: 44px;
    border-radius: 50%;
    color: var(--text-secondary);
    background: none;
    border: none;
    transition:
      background 0.15s,
      color 0.15s;
  }

  .ctrl-btn-large:hover:not(:disabled) {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .ctrl-btn-large:disabled {
    opacity: 0.3;
    cursor: default;
  }

  .play-btn-large {
    width: 56px;
    height: 56px;
    background: var(--bg-elevated);
    color: var(--text-primary);
  }

  .play-btn-large:hover {
    background: var(--bg-hover);
    color: var(--accent-primary);
  }
</style>
