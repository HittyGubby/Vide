<script lang="ts">
  import { invoke } from "$lib/transport";
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { player } from "$lib/stores/player.svelte";
  import { playerLyrics } from "$lib/player/lyrics.svelte";
  import { closePlayer } from "$lib/player/navigation";
  import {
    Play,
    Pause,
    SkipBack,
    SkipForward,
    Tag,
    Gauge,
    MessageSquareQuote,
    ChevronDown,
    Minus,
    Plus,
    Maximize,
    X,
  } from "lucide-svelte";
  import VolumeControl from "$lib/components/player/VolumeControl.svelte";
  import PlayModeControl from "$lib/components/player/PlayModeControl.svelte";
  import SeekBar from "$lib/components/player/SeekBar.svelte";
  import LyricsView from "$lib/components/lyrics/LyricsView.svelte";
  import { firstLyricText } from "$lib/lyrics";
  import TagEditor from "$lib/components/tag/TagEditor.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import type { Song } from "$lib/types";
  import { loadCoverWithFallback } from "$lib/cover";
  import { isTauri } from "$lib/isTauri";
  import { formatTime } from "$lib/songDisplay";

  const appWindow = (isTauri ? getCurrentWindow() : null) as ReturnType<typeof getCurrentWindow>;

  onMount(() => {
    player.startStatusUpdates();
  });

  let elapsedFormatted = $derived(formatTime(player.elapsed));
  let durationFormatted = $derived(formatTime(player.duration));

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

  function close() {
    closePlayer();
  }
  function minimize() {
    appWindow.minimize();
  }
  function toggleMaximize() {
    appWindow.toggleMaximize();
  }
  function closeWindow() {
    if (settingsStore.settings.close_to_tray) {
      appWindow.hide();
    } else {
      appWindow.close();
    }
  }

  // Lyrics are loaded per song, independent of playing/paused state. The store
  // is shared with the mobile player.
  $effect(() => {
    void playerLyrics.load(player.mediaId);
  });

  // Status text shown next to the time (like LX's "Failed to get lyrics").
  let lyricStatusText = $derived(
    playerLyrics.loading
      ? "加载歌词..."
      : playerLyrics.text
        ? firstLyricText(playerLyrics.text)
        : "",
  );

  // ── Playback rate popup ──────────────────────────
  let rateOpen = $state(false);
  let rateTimer: ReturnType<typeof setTimeout> | undefined;
  const rates = [0.5, 0.75, 1, 1.25, 1.5, 2];
  let rate = $state(1);
  // Keep the local rate in sync with the player's actual speed.
  $effect(() => {
    rate = player.speed;
  });

  function rateEnter() {
    clearTimeout(rateTimer);
    rateOpen = true;
  }
  function rateLeave() {
    rateTimer = setTimeout(() => (rateOpen = false), 150);
  }

  // Apply a playback rate to the player (mpv speed) and remember it.
  async function applyRate(rx: number) {
    if (!isFinite(rx) || rx <= 0) return;
    rate = rx;
    await player.setSpeed(rx);
  }

  // ── Tag editor for the current song ─────────────
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
</script>

<div class="detail-overlay" onkeydown={(e) => { if (e.key === "Escape") close(); }} role="presentation">
  <div class="detail-bg"></div>

  <div class="detail-header" data-tauri-drag-region>
    <div class="hd-spacer"></div>
    <button class="hd-btn" onclick={close} title="收起"><ChevronDown size={18} /></button>
    {#if isTauri}
      <button class="hd-btn" onclick={minimize} title="最小化"><Minus size={16} /></button>
      <button class="hd-btn" onclick={toggleMaximize} title="最大化"><Maximize size={15} /></button>
      <button class="hd-btn hd-close" onclick={closeWindow} title="关闭"><X size={16} /></button>
    {/if}
  </div>

  <div class="detail-main">
    <div class="detail-left">
      <div class="cover-large">
        {#if player.cover}
          <img src={player.cover} alt="" class="cover-img" onerror={(e) => loadCoverWithFallback(e.currentTarget as HTMLImageElement)} />
        {:else}
          <span class="monogram-large">V</span>
        {/if}
      </div>
      <div class="meta">
        <p><span class="meta-label">Title:</span> {player.title}</p>
        <p><span class="meta-label">Artist:</span> {player.artist && player.artist !== "Unknown" ? player.artist : ""}</p>
        <p><span class="meta-label">Album:</span> {player.album && player.album !== "Unknown" ? player.album : ""}</p>
      </div>
    </div>

    <div class="detail-lyrics">
      {#if playerLyrics.loading}
        <p class="lyrics-hint">加载歌词...</p>
      {:else if playerLyrics.text}
        <!-- Same component and stylesheet as the desktop lyrics window, so the
             CSS edited in the settings applies here too. -->
        <div class="lyrics-content">
          <LyricsView
            lines={playerLyrics.lines}
            elapsed={player.elapsed}
            css={settingsStore.settings.detail_lyrics_css}
            manualScroll={true}
            selectable={true}
          />
        </div>
      {:else}
        <div class="lyrics-placeholder">
          <span class="lyrics-icon">&#9835;</span>
        </div>
      {/if}
    </div>
  </div>

  <div class="detail-bottom">
    <!-- Left block: controls (top) / progress (mid) / time (bottom) -->
    <div class="bottom-left">
      <!-- control icons, right-aligned -->
      <div class="tools-row">
        <div class="rate-wrap" onmouseenter={rateEnter} onmouseleave={rateLeave}>
          <button class="d-btn" title="播放速度" aria-label="播放速度" onclick={() => (rateOpen = !rateOpen)}>
            <Gauge size={16} />
          </button>
          {#if rateOpen}
            <div class="popup rate-popup">
              <div class="rate-header">
                <span>播放速度</span>
                <span class="rate-val">{rate.toFixed(2)}x</span>
              </div>
              <input
                type="range"
                min="0.25"
                max="3"
                step="0.05"
                value={rate}
                oninput={(e) => applyRate(parseFloat((e.currentTarget as HTMLInputElement).value))}
                class="rate-slider"
              />
              <div class="rate-presets">
                {#each rates as r}
                  <button class="pop-item" class:active={rate === r} onclick={() => applyRate(r)}>
                    {r}x
                  </button>
                {/each}
              </div>
              <div class="rate-custom">
                <span>自定义</span>
                <input
                  type="number"
                  min="0.1"
                  max="5"
                  step="0.1"
                  value={rate}
                  onchange={(e) => applyRate(parseFloat((e.currentTarget as HTMLInputElement).value))}
                  onkeydown={(e) => { if (e.key === "Enter") applyRate(parseFloat((e.currentTarget as HTMLInputElement).value)); }}
                  class="rate-input"
                />
                <span>×</span>
              </div>
            </div>
          {/if}
        </div>
        <VolumeControl />

        {#if isTauri}
          <button
            class="d-btn"
            class:active={settingsStore.settings.desktop_lyrics_enabled}
            title="桌面歌词"
            aria-label="桌面歌词"
            onclick={toggleDesktopLyrics}
          >
            <MessageSquareQuote size={16} />
          </button>
        {/if}

        <PlayModeControl />
        <button class="d-btn" title="编辑标签" aria-label="编辑标签" onclick={openTagEditor}>
          <Tag size={16} />
        </button>
      </div>

      <!-- progress bar fills left width -->
      <SeekBar
        elapsed={player.elapsed}
        duration={player.duration}
        playing={player.isPlaying}
        speed={player.speed}
        onseek={seekTrack}
      />

      <!-- time, right-aligned -->
      <div class="time-row">
        <span class="lyric-status truncate">{lyricStatusText}</span>
        <span class="detail-time">{elapsedFormatted} / {durationFormatted}</span>
      </div>
    </div>

    <!-- Right block: transport buttons -->
    <div class="transport-group">
      <button class="d-btn big" title="上一首" aria-label="上一首" onclick={() => player.playPrev()}><SkipBack size={22} /></button>
      <button class="d-btn play-big" onclick={togglePlay} title="播放/暂停">
        {#if player.isPlaying}
          <Pause size={26} fill="currentColor" />
        {:else}
          <Play size={26} fill="currentColor" />
        {/if}
      </button>
      <button class="d-btn big" title="下一首" aria-label="下一首" onclick={() => player.playNext()}><SkipForward size={22} /></button>
    </div>
  </div>
</div>

{#if tagSong}
  <TagEditor song={tagSong} onclose={closeTagEditor} />
{/if}

<style>
  .detail-overlay {
    position: fixed;
    inset: 0;
    z-index: 300;
    background: var(--color-base-200);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .detail-bg {
    position: absolute;
    inset: 0;
    background: linear-gradient(160deg, color-mix(in oklch, var(--color-base-300) 40%, transparent), var(--color-base-200));
    z-index: -1;
  }

  .detail-header {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 4px;
    padding: 8px 14px;
    flex-shrink: 0;
  }

  .hd-btn {
    display: flex; align-items: center; justify-content: center;
    width: 34px; height: 30px; border-radius: 6px;
    color: color-mix(in oklch, var(--color-base-content) 75%, transparent); background: none; border: none;
  }
  .hd-btn:hover { background: var(--color-base-300); color: var(--color-base-content); }
  .hd-close:hover { background: var(--color-error); color: var(--color-error-content); }

  .detail-main {
    flex: 1;
    display: flex;
    align-items: stretch;
    justify-content: flex-start;
    gap: 80px;
    padding: 20px 60px 0 130px;
    min-height: 0;
    overflow: hidden;
  }

  /* The lyrics block sits on the far right. */
  .detail-main .detail-lyrics {
    margin-left: auto;
  }

  .detail-left {
    flex: 0 0 auto;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 18px;
  }

  .cover-large {
    width: 280px;
    height: 280px;
    border-radius: 8px;
    background: var(--color-base-300);
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    box-shadow: 0 0 20px rgba(0, 0, 0, 0.4);
  }
  .cover-img { width: 100%; height: 100%; object-fit: cover; }
  .monogram-large {
    font-weight: 700; font-size: 64px;
    color: color-mix(in oklch, var(--color-base-content) 35%, transparent);
    font-family: Consolas, "Courier New", monospace;
  }

  .meta {
    width: 100%; display: flex; flex-direction: column; gap: 4px;
    font-size: 15px; color: var(--color-base-content);
  }
  .meta-label { color: color-mix(in oklch, var(--color-base-content) 55%, transparent); margin-right: 4px; }

  .detail-lyrics {
    flex: 1;
    max-width: 420px;
    height: 100%;
    min-height: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
  /* The lyrics markup and its look now come from LyricsView + the user CSS. */
  .lyrics-content {
    flex: 1;
    height: 100%;
    min-height: 0;
  }
  .lyrics-placeholder {
    display: flex; flex-direction: column; align-items: center; gap: 8px;
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent); text-align: center;
  }
  .lyrics-icon { font-size: 40px; opacity: 0.4; }
  .lyrics-text { font-size: 14px; font-weight: 500; }
  .lyrics-sub { font-size: 11px; }
  .lyrics-hint { text-align: center; color: color-mix(in oklch, var(--color-base-content) 55%, transparent); }

  .detail-bottom {
    flex-shrink: 0;
    padding: 0 28px 20px;
    display: flex;
    align-items: flex-end;
    gap: 22px;
  }

  .bottom-left {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .tools-row {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 4px;
  }

  .time-row {
    display: flex; align-items: center; justify-content: space-between;
    gap: 12px;
  }
  .detail-time { font-size: 12px; color: color-mix(in oklch, var(--color-base-content) 70%, transparent); font-variant-numeric: tabular-nums; white-space: nowrap; }
  .lyric-status { font-size: 12px; color: color-mix(in oklch, var(--color-base-content) 50%, transparent); max-width: 60%; }

  .transport-group { display: flex; align-items: center; gap: 8px; flex: none; }

  .pop-wrap, .rate-wrap { position: relative; display: flex; align-items: center; }

  .popup {
    position: absolute; bottom: 44px; left: 50%;
    transform: translateX(-50%);
    background: var(--color-base-300);
    border: 1px solid var(--panel-border);
    border-radius: 8px; padding: 4px;
    box-shadow: var(--shadow-md);
    z-index: 60; white-space: nowrap;
    display: flex; flex-direction: column; gap: 2px;
  }
  .pop-item {
    padding: 5px 12px; border-radius: 5px; font-size: 12px;
    background: none; border: none; color: color-mix(in oklch, var(--color-base-content) 85%, transparent);
    text-align: left;
  }
  .pop-item:hover { background: var(--color-base-200); color: var(--color-base-content); }
  .pop-item.active { color: var(--color-primary); }

  .rate-popup { width: 180px; padding: 10px; gap: 8px; }
  .rate-header {
    display: flex; justify-content: space-between; align-items: center;
    font-size: 12px; color: color-mix(in oklch, var(--color-base-content) 80%, transparent);
  }
  .rate-val { color: var(--color-primary); font-weight: 600; }
  .rate-slider { width: 100%; accent-color: var(--color-primary); }
  .rate-presets { display: flex; flex-wrap: wrap; gap: 4px; }
  .rate-presets .pop-item { padding: 3px 8px; font-size: 11px; }
  .rate-custom {
    display: flex; align-items: center; gap: 6px; font-size: 12px;
    color: color-mix(in oklch, var(--color-base-content) 80%, transparent);
  }
  .rate-input {
    width: 64px; height: 26px; padding: 0 6px; border-radius: 5px;
    background: var(--color-base-200); border: 1px solid var(--panel-border);
    color: var(--color-base-content); font-size: 12px; text-align: right;
    user-select: text;
  }
  .rate-input:focus { border-color: var(--color-primary); }

  .d-btn {
    display: flex; align-items: center; justify-content: center;
    width: 38px; height: 38px; border-radius: 50%;
    color: color-mix(in oklch, var(--color-base-content) 75%, transparent);
    background: none; border: none; transition: background 0.15s, color 0.15s;
  }
  .d-btn:hover:not(:disabled) { background: var(--color-base-300); color: var(--color-base-content); }
  .d-btn:disabled { opacity: 0.4; cursor: default; }
  .d-btn.big { width: 46px; height: 46px; }
  .d-btn.play-big {
    width: 60px; height: 60px;
    background: var(--color-base-300);
    color: var(--color-base-content);
  }
  .d-btn.play-big:hover { color: var(--color-primary); }

</style>
