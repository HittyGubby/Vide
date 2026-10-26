<script lang="ts">
  import { invoke } from "$lib/transport";
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { player } from "$lib/stores/player.svelte";
  import { openPlayer } from "$lib/player/navigation";
  import {
    Play,
    Pause,
    SkipBack,
    SkipForward,
    Tag,
    MessageSquareQuote,
  } from "lucide-svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import TagEditor from "$lib/components/tag/TagEditor.svelte";
  import VolumeControl from "$lib/components/player/VolumeControl.svelte";
  import PlayModeControl from "$lib/components/player/PlayModeControl.svelte";
  import SeekBar from "$lib/components/player/SeekBar.svelte";
  import type { Song } from "$lib/types";
  import { loadCoverWithFallback } from "$lib/cover";
  import { isTauri } from "$lib/isTauri";
  import { canManage } from "$lib/stores/auth.svelte";
  import { formatTime } from "$lib/songDisplay";
  import {
    activeLyricIndex as activeLineIndex,
    parseLrc,
    type LyricLine,
  } from "$lib/lyrics";

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
    player.startStatusUpdates();
  });

  async function togglePlay() {
    await player.togglePause();
  }

  // Visibility of the desktop lyrics window is a setting; the root layout
  // mirrors it onto the window itself.
  async function toggleDesktopLyrics() {
    await settingsStore.update(
      "desktop_lyrics_enabled",
      !settingsStore.settings.desktop_lyrics_enabled,
    );
  }

  async function seekTrack(targetSecs: number) {
    await player.seekTo(targetSecs);
  }

  let currentTime = $derived(formatTime(player.elapsed));
  let totalTime = $derived(formatTime(player.duration));

  function openDetail() {
    if (player.mediaId) openPlayer();
  }

  // ── Right-click the cover/title/lyrics area → locate the current song. ──
  function locateCurrentSong(e: MouseEvent) {
    e.preventDefault();
    if (!player.mediaId) return;
    // Navigate to the library view, then signal the locate via a monotonic
    // counter so repeated right-clicks on the same song always work.
    void goto("/");
    player.requestLocate(player.mediaId);
  }

  // ── Lyrics line (bottom-left) ────────────────────
  // Parsed with the shared LRC parser so the player bar, the detail view and
  // the floating window can never disagree about what is playing.
  let lyrics = $state<LyricLine[]>([]);

  $effect(() => {
    const id = player.mediaId;
    if (!id) {
      lyrics = [];
      return;
    }
    invoke<string | null>("get_lyrics", { songId: id })
      .then((raw) => {
        lyrics = raw ? parseLrc(raw) : [];
      })
      .catch(() => {
        lyrics = [];
      });
  });

  let activeIndex = $derived(activeLineIndex(lyrics, player.elapsed));
  let activeLyric = $derived(
    activeIndex >= 0 ? lyrics[activeIndex].parts.map((part) => part.text).join(" / ") : null,
  );

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
        <img src={player.cover} alt="" class="cover" onerror={(e) => loadCoverWithFallback(e.currentTarget as HTMLImageElement)} />
      {:else}
        <span class="monogram">V</span>
      {/if}
    </button>

    <button class="info" onclick={openDetail} title="播放详情" aria-label="播放详情">
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
    </button>
  </div>

  <!-- Time indicator -->
  <div class="time-indicator">
    <span class="time-badge">{currentTime}</span>
    <span class="time-sep">/</span>
    <span class="time-total">{totalTime}</span>
  </div>

  <!-- Control buttons (localized) -->
  <div class="player-tools">
    {#if canManage()}
      <button class="ctrl-btn" title="编辑标签" aria-label="编辑标签" onclick={openTagEditor}>
        <Tag size={15} />
      </button>
    {/if}

    <VolumeControl />

    {#if isTauri}
    <button
      class="ctrl-btn"
      class:active={settingsStore.settings.desktop_lyrics_enabled}
      title="桌面歌词"
      aria-label="桌面歌词"
      onclick={toggleDesktopLyrics}
    >
      <MessageSquareQuote size={15} />
    </button>
    {/if}

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
    color: color-mix(in oklch, var(--color-base-content) 60%, transparent);
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
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .info-title {
    font-size: 13px;
    font-weight: 500;
    color: var(--color-base-content);
  }

  .info-sub {
    font-size: 11px;
    color: color-mix(in oklch, var(--color-base-content) 60%, transparent);
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
    color: color-mix(in oklch, var(--color-base-content) 60%, transparent);
    font-variant-numeric: tabular-nums;
    min-width: 34px;
    text-align: center;
  }
  .time-sep { color: color-mix(in oklch, var(--color-base-content) 40%, transparent); margin: 0 2px; }

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
    color: color-mix(in oklch, var(--color-base-content) 70%, transparent);
    background: none;
    border: none;
    transition: background 0.15s, color 0.15s;
  }
  .ctrl-btn:hover:not(:disabled) { background: var(--color-base-300); color: var(--color-base-content); }
  .ctrl-btn.active { color: var(--color-primary); }
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
