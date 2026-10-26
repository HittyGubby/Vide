<script lang="ts">
  import { onMount } from "svelte";
  import {
    ChevronDown,
    MoreHorizontal,
    Pause,
    Play,
    Repeat,
    SkipBack,
    SkipForward,
    Tag,
  } from "lucide-svelte";
  import { player } from "$lib/stores/player.svelte";
  import { playerLyrics } from "$lib/player/lyrics.svelte";
  import { closePlayer } from "$lib/player/navigation";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { canManage } from "$lib/stores/auth.svelte";
  import SeekBar from "$lib/components/player/SeekBar.svelte";
  import LyricsView from "$lib/components/lyrics/LyricsView.svelte";
  import TagEditor from "$lib/components/tag/TagEditor.svelte";
  import Dialog from "$lib/ui/Dialog.svelte";
  import { loadCoverWithFallback } from "$lib/cover";
  import { formatTime } from "$lib/songDisplay";
  import { firstLyricText } from "$lib/lyrics";
  import { invoke } from "$lib/transport";
  import type { Song } from "$lib/types";

  const RATES = [0.75, 1, 1.25, 1.5, 2];

  onMount(() => {
    player.startStatusUpdates();
  });

  $effect(() => {
    void playerLyrics.load(player.mediaId);
  });

  let page = $state<"cover" | "lyrics">("cover");
  let pagerEl = $state<HTMLDivElement | null>(null);
  let moreOpen = $state(false);
  let tagSong = $state<Song | null>(null);

  let subtitle = $derived(
    player.artist && player.artist !== "Unknown" ? player.artist : "",
  );
  let album = $derived(
    player.album && player.album !== "Unknown" ? player.album : "",
  );
  let elapsedText = $derived(formatTime(player.elapsed));
  let durationText = $derived(formatTime(player.duration));

  let lyricStatus = $derived(
    playerLyrics.loading
      ? "加载歌词..."
      : playerLyrics.text
        ? firstLyricText(playerLyrics.text)
        : "",
  );

  function onPagerScroll() {
    if (!pagerEl || pagerEl.clientWidth === 0) return;
    page = pagerEl.scrollLeft > pagerEl.clientWidth * 0.5 ? "lyrics" : "cover";
  }

  function goPage(next: "cover" | "lyrics") {
    if (!pagerEl) return;
    pagerEl.scrollTo({
      left: next === "lyrics" ? pagerEl.clientWidth : 0,
      behavior: "smooth",
    });
  }

  async function seekTrack(target: number) {
    await player.seekTo(target);
  }

  function setMode(patch: Partial<{ single: boolean; reverse: boolean; shuffle: boolean; repeat: boolean }>) {
    player.playMode = { ...player.playMode, ...patch };
    player.syncPlayMode();
  }

  let modeName = $derived.by(() => {
    const scope = player.playMode.single ? "单曲" : "列表";
    const order = player.playMode.shuffle
      ? "随机"
      : player.playMode.reverse
        ? "逆序"
        : "按序";
    return scope + " · " + order;
  });

  async function openTagEditor() {
    if (!player.mediaId) return;
    try {
      const song = await invoke<Song>("get_song", { id: player.mediaId });
      if (song) tagSong = song;
    } catch (e) {
      console.error("get song for tag error:", e);
    }
  }
</script>

<div class="mobile-player">
  <header class="mp-header">
    <button class="mp-btn" onclick={closePlayer} aria-label="返回">
      <ChevronDown size={22} />
    </button>
    <div class="mp-heading">
      <span class="mp-title">{player.mediaId ? player.title : "未在播放"}</span>
      <span class="mp-sub">{subtitle}</span>
    </div>
    {#if canManage()}
      <button class="mp-btn" onclick={() => void openTagEditor()} aria-label="编辑标签">
        <Tag size={18} />
      </button>
    {/if}
  </header>

  {#if !player.mediaId}
    <div class="mp-empty">
      <span class="mp-empty-mark">V</span>
      <p>还没有正在播放的歌曲</p>
      <button class="mp-empty-btn" onclick={closePlayer}>回到列表</button>
    </div>
  {:else}
    <div class="mp-pager" bind:this={pagerEl} onscroll={onPagerScroll}>
      <section class="mp-page cover-page">
        <div class="cover">
          {#if player.cover}
            <img
              src={player.cover}
              alt=""
              onerror={(e) => loadCoverWithFallback(e.currentTarget as HTMLImageElement)}
            />
          {:else}
            <span class="monogram">V</span>
          {/if}
        </div>
        <h1 class="song-title">{player.title}</h1>
        {#if subtitle}<p class="song-sub">{subtitle}</p>{/if}
        {#if album}<p class="song-album">{album}</p>{/if}
      </section>

      <section class="mp-page lyrics-page">
        {#if playerLyrics.loading}
          <p class="hint">加载歌词...</p>
        {:else if playerLyrics.text}
          <div class="lyrics-box">
            <LyricsView
              lines={playerLyrics.lines}
              elapsed={player.elapsed}
              css={settingsStore.settings.detail_lyrics_css}
              manualScroll={true}
              selectable={true}
            />
          </div>
        {:else}
          <p class="hint">暂无歌词</p>
        {/if}
      </section>
    </div>

    <div class="mp-dots">
      <button
        class="dot"
        class:on={page === "cover"}
        aria-label="封面"
        onclick={() => goPage("cover")}
      ></button>
      <button
        class="dot"
        class:on={page === "lyrics"}
        aria-label="歌词"
        onclick={() => goPage("lyrics")}
      ></button>
    </div>

    <footer class="mp-footer">
      <SeekBar
        elapsed={player.elapsed}
        duration={player.duration}
        playing={player.isPlaying}
        speed={player.speed}
        onseek={seekTrack}
      />
      <div class="times">
        <span>{elapsedText}</span>
        <span class="lyric-status">{lyricStatus}</span>
        <span>{durationText}</span>
      </div>

      <div class="transport">
        <button class="t-btn small" aria-label="播放设置" onclick={() => (moreOpen = true)}>
          <Repeat size={18} />
        </button>
        <button class="t-btn" aria-label="上一首" onclick={() => player.playPrev()}>
          <SkipBack size={26} />
        </button>
        <button class="t-btn play" aria-label="播放/暂停" onclick={() => void player.togglePause()}>
          {#if player.isPlaying}
            <Pause size={30} fill="currentColor" />
          {:else}
            <Play size={30} fill="currentColor" />
          {/if}
        </button>
        <button class="t-btn" aria-label="下一首" onclick={() => player.playNext()}>
          <SkipForward size={26} />
        </button>
        <button class="t-btn small" aria-label="更多设置" onclick={() => (moreOpen = true)}>
          <MoreHorizontal size={20} />
        </button>
      </div>
    </footer>
  {/if}
</div>

<Dialog bind:open={moreOpen} placement="bottom" label="播放设置">
  <div class="sheet">
    <div class="sheet-title">播放设置</div>

    <div class="field">
      <div class="field-head"><span>播放模式</span><span class="value">{modeName}</span></div>
      <div class="seg-row">
        <span class="seg">
          <button class="seg-btn" class:on={player.playMode.single} onclick={() => setMode({ single: true })}>单曲</button>
          <button class="seg-btn" class:on={!player.playMode.single} onclick={() => setMode({ single: false })}>列表</button>
        </span>
        <span class="seg">
          <button class="seg-btn" class:on={!player.playMode.shuffle} onclick={() => setMode({ shuffle: false, reverse: false })}>按序</button>
          <button class="seg-btn" class:on={player.playMode.shuffle} onclick={() => setMode({ shuffle: true })}>随机</button>
        </span>
        {#if !player.playMode.shuffle}
          <span class="seg">
            <button class="seg-btn" class:on={!player.playMode.reverse} onclick={() => setMode({ reverse: false })}>顺序</button>
            <button class="seg-btn" class:on={player.playMode.reverse} onclick={() => setMode({ reverse: true })}>逆序</button>
          </span>
        {/if}
        <span class="seg">
          <button class="seg-btn" class:on={!player.playMode.repeat} onclick={() => setMode({ repeat: false })}>播完暂停</button>
          <button class="seg-btn" class:on={player.playMode.repeat} onclick={() => setMode({ repeat: true })}>循环</button>
        </span>
      </div>
    </div>

    <div class="field">
      <div class="field-head"><span>音量</span><span class="value">{player.volume}%</span></div>
      <input
        type="range"
        min="0"
        max="100"
        step="1"
        value={player.volume}
        oninput={(e) => void player.setVolume(Number(e.currentTarget.value))}
        class="slider"
      />
    </div>

    <div class="field">
      <div class="field-head"><span>播放速度</span><span class="value">{player.speed.toFixed(2)}x</span></div>
      <input
        type="range"
        min="0.25"
        max="3"
        step="0.05"
        value={player.speed}
        oninput={(e) => void player.setSpeed(parseFloat(e.currentTarget.value))}
        class="slider"
      />
      <div class="presets">
        {#each RATES as rate}
          <button
            class="preset"
            class:active={Math.abs(player.speed - rate) < 0.01}
            onclick={() => void player.setSpeed(rate)}
          >{rate}x</button>
        {/each}
      </div>
    </div>

    <button class="sheet-close" onclick={() => (moreOpen = false)}>完成</button>
  </div>
</Dialog>

{#if tagSong}
  <TagEditor song={tagSong} onclose={() => (tagSong = null)} />
{/if}

<style>
  .mobile-player {
    display: flex;
    flex-direction: column;
    height: 100vh;
    height: 100dvh;
    background: var(--color-base-100);
    color: var(--color-base-content);
    overflow: hidden;
    padding-top: env(safe-area-inset-top);
    padding-bottom: env(safe-area-inset-bottom);
  }

  .mp-header {
    flex: none;
    display: flex;
    align-items: center;
    gap: 6px;
    height: 52px;
    padding: 0 6px;
  }
  .mp-btn {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 40px;
    border: 0;
    border-radius: 9px;
    background: none;
    color: inherit;
  }
  .mp-btn:active {
    background: var(--color-base-300);
  }
  .mp-heading {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1px;
  }
  .mp-title,
  .mp-sub {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mp-title {
    font-size: 14px;
    font-weight: 600;
  }
  .mp-sub {
    font-size: 11px;
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent);
  }

  .mp-pager {
    flex: 1;
    min-height: 0;
    display: flex;
    overflow-x: auto;
    overflow-y: hidden;
    scroll-snap-type: x mandatory;
    scrollbar-width: none;
    overscroll-behavior-x: contain;
  }
  .mp-pager::-webkit-scrollbar {
    display: none;
  }
  .mp-page {
    flex: 0 0 100%;
    width: 100%;
    height: 100%;
    scroll-snap-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 0 24px;
    min-width: 0;
  }
  .cover-page {
    gap: 6px;
  }
  .cover {
    width: min(64vw, 260px);
    aspect-ratio: 1;
    margin-bottom: 18px;
    border-radius: 14px;
    overflow: hidden;
    background: var(--color-base-300);
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: var(--shadow-lg);
  }
  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .monogram {
    font-size: 64px;
    font-weight: 700;
    color: color-mix(in oklch, var(--color-base-content) 30%, transparent);
    font-family: ui-monospace, "SFMono-Regular", Menlo, Consolas, monospace;
  }
  .song-title {
    margin: 0;
    font-size: 19px;
    font-weight: 600;
    text-align: center;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .song-sub,
  .song-album {
    margin: 0;
    font-size: 12px;
    color: color-mix(in oklch, var(--color-base-content) 60%, transparent);
    text-align: center;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .song-album {
    font-size: 11px;
    color: color-mix(in oklch, var(--color-base-content) 45%, transparent);
  }

  .lyrics-page {
    justify-content: stretch;
    padding: 0;
  }
  .lyrics-box {
    width: 100%;
    height: 100%;
    min-height: 0;
  }
  .hint {
    margin: auto;
    font-size: 12px;
    color: color-mix(in oklch, var(--color-base-content) 50%, transparent);
  }

  .mp-dots {
    flex: none;
    display: flex;
    justify-content: center;
    gap: 6px;
    padding: 6px 0;
  }
  .dot {
    width: 6px;
    height: 6px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: color-mix(in oklch, var(--color-base-content) 25%, transparent);
  }
  .dot.on {
    background: var(--color-primary);
  }

  .mp-footer {
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 0 14px 6px;
  }
  .times {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    font-size: 11px;
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent);
    font-variant-numeric: tabular-nums;
  }
  .lyric-status {
    flex: 1;
    min-width: 0;
    text-align: center;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .transport {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 4px;
    padding: 2px 0 4px;
  }
  .t-btn {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 52px;
    height: 52px;
    border: 0;
    border-radius: 50%;
    background: none;
    color: var(--color-base-content);
  }
  .t-btn.small {
    width: 42px;
    height: 42px;
    color: color-mix(in oklch, var(--color-base-content) 65%, transparent);
  }
  .t-btn:active {
    background: var(--color-base-300);
  }
  .t-btn.play {
    width: 64px;
    height: 64px;
    background: var(--color-base-300);
  }
  .t-btn.play:active {
    background: var(--color-primary);
    color: var(--color-primary-content);
  }

  /* ── Playback settings sheet ─────────────────────── */
  .sheet {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 14px 18px calc(12px + env(safe-area-inset-bottom));
  }
  .sheet-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--color-base-content);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .field-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 12px;
    color: color-mix(in oklch, var(--color-base-content) 70%, transparent);
  }
  .value {
    color: var(--color-primary);
    font-variant-numeric: tabular-nums;
  }
  .seg-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .seg {
    display: inline-flex;
    gap: 2px;
    padding: 2px;
    border-radius: 7px;
    background: var(--color-base-300);
  }
  .seg-btn {
    padding: 5px 10px;
    border: 0;
    border-radius: 5px;
    background: none;
    color: color-mix(in oklch, var(--color-base-content) 65%, transparent);
    font-size: 12px;
  }
  .seg-btn.on {
    background: var(--color-base-100);
    color: var(--color-primary);
    font-weight: 600;
  }
  .slider {
    width: 100%;
    accent-color: var(--color-primary);
  }
  .presets {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .preset {
    padding: 4px 10px;
    border: 1px solid var(--panel-border);
    border-radius: 6px;
    background: var(--color-base-200);
    color: color-mix(in oklch, var(--color-base-content) 75%, transparent);
    font-size: 11px;
  }
  .preset.active {
    border-color: var(--color-primary);
    color: var(--color-primary);
  }
  .sheet-close {
    height: 44px;
    border: 0;
    border-radius: 9px;
    background: var(--color-base-300);
    color: var(--color-base-content);
    font-size: 14px;
  }

  .mp-empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
  }
  .mp-empty-mark {
    font-size: 56px;
    font-weight: 700;
    color: color-mix(in oklch, var(--color-base-content) 25%, transparent);
  }
  .mp-empty p {
    margin: 0;
    font-size: 13px;
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent);
  }
  .mp-empty-btn {
    padding: 7px 16px;
    border: 1px solid var(--panel-border);
    border-radius: 8px;
    background: var(--color-base-200);
    color: inherit;
  }
</style>
