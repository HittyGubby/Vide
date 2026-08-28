<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { player } from "$lib/stores/player.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import {
    Play,
    Pause,
    SkipBack,
    SkipForward,
    Tag,
  } from "lucide-svelte";
  import TagEditor from "$lib/components/tag/TagEditor.svelte";
  import VolumeControl from "$lib/components/player/VolumeControl.svelte";
  import PlayModeControl from "$lib/components/player/PlayModeControl.svelte";
  import SeekBar from "$lib/components/player/SeekBar.svelte";
  import type { Song } from "$lib/types";

  let pollTimer: ReturnType<typeof setInterval> | undefined;

  // Tag editor for the currently playing song
  let tagSong = $state<Song | null>(null);

  async function openTagEditor() {
    if (!player.mediaId) return;
    try {
      const song = await invoke<Song>("get_song", { id: player.mediaId });
      if (song) tagSong = song;
    } catch (e) {
      console.error("get song for tag error:", e);
    }
  }

  function closeTagEditor() {
    tagSong = null;
  }

  onMount(() => {
    player.poll();
    pollTimer = setInterval(() => player.poll(), 1000);
    return () => clearInterval(pollTimer);
  });

  async function togglePlay() {
    await player.togglePause();
  }

  async function seekTrack(targetSecs: number) {
    if (player.duration <= 0) return;
    try {
      await invoke("seek", { positionSecs: targetSecs });
      player.elapsed = targetSecs;
    } catch {
      /* seek not supported in this mode */
    }
  }

  function formatTime(secs: number): string {
    if (!secs || secs <= 0) return "0:00";
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    return m + ":" + s.toString().padStart(2, "0");
  }

  let currentTime = $derived(formatTime(player.elapsed));
  let totalTime = $derived(formatTime(player.duration));

  function openDetail() {
    if (player.mediaId) ui.showPlayerDetail = true;
  }

  // ── Right-click the cover/title/lyrics area → locate the current song. ──
  function locateCurrentSong(e: MouseEvent) {
    e.preventDefault();
    if (!player.mediaId) return;
    ui.showPlayerDetail = false;
    // Navigate to the library view, then signal the locate via a monotonic
    // counter so repeated right-clicks on the same song always work.
    void goto("/");
    player.requestLocate(player.mediaId);
  }

  // ── Lyrics line (bottom-left) ────────────────────
  let lyricsLine = $state<string | null>(null);
  let lyrics = $state<{ t: number; text: string }[]>([]);

  $effect(() => {
    const id = player.mediaId;
    if (!id) {
      lyrics = [];
      lyricsLine = null;
      return;
    }
    invoke<string | null>("get_lyrics", { songId: id })
      .then((raw) => {
        if (!raw) {
          lyrics = [];
          lyricsLine = null;
          return;
        }
        const parsed = parseLrc(raw);
        lyrics = parsed;
      })
      .catch(() => {
        lyrics = [];
        lyricsLine = null;
      });
  });

  function parseLrc(raw: string): { t: number; text: string }[] {
    const out: { t: number; text: string }[] = [];
    for (const line of raw.split("\n")) {
      const m = line.match(/\[(\d+):(\d+(?:\.\d+)?)\]\s*(.*)/);
      if (m) {
        const t = parseInt(m[1]) * 60 + parseFloat(m[2]);
        out.push({ t, text: m[3] || "" });
      }
    }
    out.sort((a, b) => a.t - b.t);
    return out;
  }

  let activeLyric = $derived.by(() => {
    if (lyrics.length === 0) return null;
    const el = player.elapsed;
    let cur: string | null = null;
    for (const l of lyrics) {
      if (l.t <= el) cur = l.text;
      else break;
    }
    return cur;
  });

  // Subtitle: current lyric line, else artist (Unknown shown as empty).
  let subtitle = $derived(
    activeLyric?.trim() || (player.artist && player.artist !== "Unknown" ? player.artist : ""),
  );
</script>

<footer class="playerbar">
  <!-- Full-width progress strip pinned to the top of the player bar -->
  {#if player.mediaId && player.duration > 0}
    <div class="full-progress">
      <SeekBar
        elapsed={player.elapsed}
        duration={player.duration}
        playing={player.isPlaying}
        speed={player.speed}
        heightPx={2}
        knob={true}
        onseek={seekTrack}
      />
    </div>
  {/if}

  <div class="player-left" oncontextmenu={locateCurrentSong}>
    <button class="album-art" onclick={openDetail} title="播放详情" aria-label="播放详情">
      {#if player.cover}
        <img src={player.cover} alt="" class="cover" />
      {:else}
        <span class="monogram">V</span>
      {/if}
    </button>

    <div class="info">
      <div class="info-title truncate">{player.title}</div>
      <div class="info-sub truncate">
        {#if subtitle}
          <span class="lyric-text">{subtitle}</span>
        {:else if player.mediaId}
          <!-- A song is loaded but has no lyrics/artist — keep it blank. -->
          <span class="idle-text" style="visibility: hidden">&nbsp;</span>
        {:else}
          <span class="idle-text">选择一首歌曲开始播放</span>
        {/if}
      </div>
    </div>
  </div>

  <!-- Time indicator -->
  <div class="time-indicator">
    <span class="time-badge">{currentTime}</span>
    <span class="time-sep">/</span>
    <span class="time-total">{totalTime}</span>
  </div>

  <!-- Control buttons (localized) -->
  <div class="player-tools">
    <button class="ctrl-btn" title="编辑标签" aria-label="编辑标签" onclick={openTagEditor}>
      <Tag size={15} />
    </button>

    <VolumeControl />
    <PlayModeControl />
  </div>

  <div class="play-controls">
    <button class="ctrl-btn skip" title="上一首" aria-label="上一首" onclick={() => player.playPrev()}>
      <SkipBack size={18} />
    </button>
    <button class="ctrl-btn play-btn" onclick={togglePlay} aria-label="播放/暂停">
      {#if player.isPlaying}
        <Pause size={20} fill="currentColor" />
      {:else}
        <Play size={20} fill="currentColor" />
      {/if}
    </button>
    <button class="ctrl-btn skip" title="下一首" aria-label="下一首" onclick={() => player.playNext()}>
      <SkipForward size={18} />
    </button>
  </div>
</footer>

{#if tagSong}
  <TagEditor song={tagSong} onclose={closeTagEditor} />
{/if}

<style>
  .playerbar {
    position: relative;
    display: flex;
    align-items: center;
    height: var(--playerbar-height);
    background: var(--color-base-200);
    border-top: 1px solid var(--panel-border-strong);
    padding: 0 8px 0 10px;
    gap: 12px;
    user-select: none;
    flex-shrink: 0;
  }

  /* Slim full-width progress strip pinned to the very top of the bar. */
  .full-progress {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    z-index: 1;
  }
  .full-progress :global(.seekbar) {
    border-radius: 0;
    background: transparent;
  }
  .full-progress :global(.seek-track) {
    border-radius: 0;
  }

  .player-left {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    flex: 1;
  }

  .album-art {
    width: 48px;
    height: 48px;
    border-radius: 6px;
    background: var(--color-base-300);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    color: var(--color-base-content) / 0.6;
    cursor: pointer;
    overflow: hidden;
    border: none;
    transition: opacity 0.15s;
  }
  .album-art:hover { opacity: 0.85; }
  .cover { width: 100%; height: 100%; object-fit: cover; }
  .monogram { font-weight: 700; font-size: 18px; font-family: Consolas, "Courier New", monospace; }

  .info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    justify-content: center;
  }

  .info-title {
    font-size: 13px;
    font-weight: 500;
    color: var(--color-base-content);
  }

  .info-sub {
    font-size: 11px;
    color: var(--color-base-content) / 0.6;
    height: 14px;
    display: flex;
    align-items: center;
  }

  .time-indicator {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    flex: none;
  }

  .time-badge, .time-total {
    font-size: 12px;
    color: var(--color-base-content) / 0.6;
    font-variant-numeric: tabular-nums;
    min-width: 34px;
    text-align: center;
  }
  .time-sep { color: var(--color-base-content) / 0.4; margin: 0 2px; }

  .player-tools {
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
    color: var(--color-base-content) / 0.7;
    background: none;
    border: none;
    transition: background 0.15s, color 0.15s;
  }
  .ctrl-btn:hover:not(:disabled) { background: var(--color-base-300); color: var(--color-base-content); }
  .ctrl-btn:disabled { opacity: 0.4; cursor: default; }

  .play-controls { display: flex; align-items: center; gap: 2px; }
  .ctrl-btn.skip { width: 36px; height: 36px; }
  .play-btn {
    width: 42px; height: 42px; border-radius: 50%;
    background: var(--color-base-300);
    color: var(--color-base-content);
  }
  .play-btn:hover { color: var(--color-primary); }
</style>
