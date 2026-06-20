<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { player } from "$lib/stores/player.svelte";
  import {
    Play,
    Pause,
    SkipBack,
    SkipForward,
    Volume2,
    Tag,
  } from "lucide-svelte";
  import { ui } from "$lib/stores/ui.svelte";

  let pollTimer: ReturnType<typeof setInterval> | undefined;

  onMount(() => {
    player.poll();
    pollTimer = setInterval(() => player.poll(), 1000);
    return () => clearInterval(pollTimer);
  });

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

  function formatTime(secs: number): string {
    if (secs <= 0) return "0:00";
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  let currentTime = $derived(formatTime(player.elapsed));
</script>

<footer class="playerbar">
  <!-- Left: Album art + song info -->
  <div class="player-left">
    <div class="album-art" onclick={() => (ui.showPlayerDetail = true)}>
      {#if player.mediaId && player.isPlaying}
        <span class="equalizer">𝅘𝅥𝅮</span>
      {:else}
        <svg
          class="note-icon"
          width="20"
          height="20"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path d="M9 18V5l12-2v13" />
          <circle cx="6" cy="18" r="3" />
          <circle cx="18" cy="16" r="3" />
        </svg>
      {/if}
    </div>
    <div class="song-info">
      <span class="song-title truncate">{player.title}</span>
      <span class="song-artist truncate">{player.artist}</span>
    </div>
  </div>

  <!-- Center: selected state -->
  <div class="player-center">
    <span class="time-badge">{currentTime}</span>
  </div>

  <!-- Right: playback controls + extras -->
  <div class="player-actions">
    <button class="ctrl-btn" title="标签编辑" aria-label="标签">
      <Tag size={15} />
    </button>
    <button class="ctrl-btn" title="音量" aria-label="音量">
      <Volume2 size={16} />
    </button>
  </div>

  <div class="play-controls">
    <button class="ctrl-btn ctrl-skip" title="上一首" aria-label="上一首">
      <SkipBack size={18} />
    </button>
    <button
      class="ctrl-btn play-btn"
      onclick={togglePlay}
      aria-label="播放/暂停"
    >
      {#if player.isPlaying}
        <Pause size={20} fill="currentColor" />
      {:else}
        <Play size={20} fill="currentColor" />
      {/if}
    </button>
    <button class="ctrl-btn ctrl-skip" title="下一首" aria-label="下一首">
      <SkipForward size={18} />
    </button>
  </div>

  <!-- Resize grip -->
  <div class="resize-grip">
    <svg width="10" height="10" viewBox="0 0 10 10">
      <line
        x1="7"
        y1="10"
        x2="10"
        y2="7"
        stroke="currentColor"
        stroke-width="1"
        opacity="0.3"
      />
      <line
        x1="4"
        y1="10"
        x2="10"
        y2="4"
        stroke="currentColor"
        stroke-width="1"
        opacity="0.3"
      />
      <line
        x1="1"
        y1="10"
        x2="10"
        y2="1"
        stroke="currentColor"
        stroke-width="1"
        opacity="0.3"
      />
    </svg>
  </div>
</footer>

<style>
  .playerbar {
    display: flex;
    align-items: center;
    height: var(--playerbar-height);
    background: var(--bg-surface);
    border-top: none;
    padding: 0 8px 0 12px;
    gap: 8px;
  }

  /* ── Left Section ────────────────────────────────── */
  .player-left {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    flex: 1;
  }

  .album-art {
    width: 44px;
    height: 44px;
    border-radius: 6px;
    background: var(--bg-elevated);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    color: var(--text-tertiary);
    cursor: pointer;
    transition: background 0.15s;
  }

  .album-art:hover {
    background: var(--bg-hover);
  }

  .note-icon {
    opacity: 0.5;
  }

  .equalizer {
    font-size: 22px;
    color: var(--accent-primary);
    animation: pulse 1s ease-in-out infinite;
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 0.6;
    }
    50% {
      opacity: 1;
    }
  }

  .song-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .song-title {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-primary);
  }

  .song-artist {
    font-size: 11px;
    color: var(--text-secondary);
  }

  /* ── Center ──────────────────────────────────────── */
  .player-center {
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .time-badge {
    font-size: 12px;
    color: var(--text-tertiary);
    font-variant-numeric: tabular-nums;
    min-width: 36px;
    text-align: center;
  }

  /* ── Action Buttons ──────────────────────────────── */
  .player-actions {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .ctrl-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border-radius: 6px;
    color: var(--text-secondary);
    background: none;
    border: none;
    transition:
      background 0.15s,
      color 0.15s;
  }

  .ctrl-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .ctrl-btn:active {
    background: var(--bg-active);
  }

  /* ── Play Controls (far right) ───────────────────── */
  .play-controls {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .ctrl-skip {
    width: 36px;
    height: 36px;
  }

  .play-btn {
    width: 40px;
    height: 40px;
    border-radius: 50%;
    background: var(--bg-elevated);
    color: var(--text-primary);
  }

  .play-btn:hover {
    background: var(--bg-hover);
    color: var(--accent-primary);
  }

  .play-btn:active {
    background: var(--bg-active);
  }

  /* ── Resize Grip ─────────────────────────────────── */
  .resize-grip {
    position: fixed;
    bottom: 0;
    right: 0;
    width: 16px;
    height: 16px;
    display: flex;
    align-items: flex-end;
    justify-content: flex-end;
    padding: 3px;
    color: var(--text-tertiary);
    cursor: se-resize;
    z-index: 200;
    pointer-events: none;
  }
</style>
