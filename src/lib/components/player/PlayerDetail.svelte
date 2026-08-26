<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { player } from "$lib/stores/player.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import {
    Play,
    Pause,
    SkipBack,
    SkipForward,
    Tag,
    Gauge,
    ChevronDown,
    Minus,
    Plus,
    Maximize,
    X,
  } from "lucide-svelte";
  import VolumeControl from "$lib/components/player/VolumeControl.svelte";
  import PlayModeControl from "$lib/components/player/PlayModeControl.svelte";
  import TagEditor from "$lib/components/tag/TagEditor.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import type { Song } from "$lib/types";

  const appWindow = getCurrentWindow();

  let pollTimer: ReturnType<typeof setInterval> | undefined;

  onMount(() => {
    player.poll();
    pollTimer = setInterval(() => player.poll(), 1000);
    return () => clearInterval(pollTimer);
  });

  let progressPct = $derived(
    player.duration > 0 ? Math.min((player.elapsed / player.duration) * 100, 100) : 0,
  );

  function formatTime(secs: number): string {
    if (!secs || secs <= 0) return "0:00";
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    return m + ":" + s.toString().padStart(2, "0");
  }

  let elapsedFormatted = $derived(formatTime(player.elapsed));
  let durationFormatted = $derived(formatTime(player.duration));

  async function togglePlay() {
    if (player.isPlaying) {
      try {
        await invoke("stop_playback");
        player.isPlaying = false;
      } catch (e) {
        console.error("stop error:", e);
      }
    } else if (player.mediaId) {
      try {
        await invoke("play_song", { id: player.mediaId });
        player.isPlaying = true;
      } catch (e) {
        console.error("play error:", e);
      }
    }
  }

  async function seekTrack(e: MouseEvent) {
    if (!player.isPlaying || player.duration <= 0) return;
    const target = e.currentTarget as HTMLElement;
    const rect = target.getBoundingClientRect();
    const pct = (e.clientX - rect.left) / rect.width;
    const targetSecs = pct * player.duration;
    try {
      await invoke("seek", { positionSecs: targetSecs });
      player.elapsed = targetSecs;
    } catch {
      /* seek not supported */
    }
  }

  function close() {
    ui.showPlayerDetail = false;
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

  // Lyrics
  let lyricsText = $state<string | null>(null);
  let lyricsLoading = $state(false);

  $effect(() => {
    const id = player.mediaId;
    if (id && player.isPlaying) {
      lyricsLoading = true;
      invoke("get_lyrics", { songId: id })
        .then((lyrics) => {
          lyricsText = (lyrics as string | null) ?? null;
        })
        .catch(() => {
          lyricsText = null;
        })
        .finally(() => {
          lyricsLoading = false;
        });
    } else {
      lyricsText = null;
    }
  });

  // Status text shown next to the time (like LX's "Failed to get lyrics").
  let lyricStatusText = $derived(
    lyricsLoading ? "加载歌词..." : lyricsText ? firstLyricLine(lyricsText) : "暂无歌词",
  );

  function firstLyricLine(t: string): string {
    const l = t.split("\n").map((s) => s.trim()).find((s) => s.length > 0);
    return l ?? "暂无歌词";
  }

  // ── Timed lyrics: parse [mm:ss] markers, highlight the active line ──
  let lrcLines = $state<{ t: number; text: string }[]>([]);
  function parseLrc(raw: string): { t: number; text: string }[] {
    const out: { t: number; text: string }[] = [];
    for (const line of raw.split("\n")) {
      const m = line.match(/\[(\d+):(\d+(?:\.\d+)?)\]\s*(.*)/);
      if (m) out.push({ t: parseInt(m[1]) * 60 + parseFloat(m[2]), text: m[3] || "" });
    }
    out.sort((a, b) => a.t - b.t);
    return out;
  }
  $effect(() => {
    if (lyricsText) lrcLines = parseLrc(lyricsText);
    else lrcLines = [];
  });
  let activeLineIdx = $derived.by(() => {
    if (lrcLines.length === 0) return -1;
    let idx = -1;
    for (let i = 0; i < lrcLines.length; i++) {
      if (lrcLines[i].t <= player.elapsed) idx = i;
      else break;
    }
    return idx;
  });

  // ── Right-click lyrics → font size adjuster (minus / current / plus) ──
  let lyrCtx = $state<{ x: number; y: number } | null>(null);
  function openLyrMenu(e: MouseEvent) {
    e.preventDefault();
    lyrCtx = { x: e.clientX, y: e.clientY };
  }
  function closeLyrMenu() {
    lyrCtx = null;
  }
  function lyrDec() {
    const cur = settingsStore.settings.lyrics_font_size ?? 14;
    settingsStore.update("lyrics_font_size", Math.max(10, cur - 1));
  }
  function lyrInc() {
    const cur = settingsStore.settings.lyrics_font_size ?? 14;
    settingsStore.update("lyrics_font_size", Math.min(48, cur + 1));
  }

  // ── Drag on lyrics to seek ──
  let dragging = $state(false);
  function onLyrPointerDown(e: PointerEvent) {
    if (player.duration <= 0) return;
    dragging = true;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    seekFromLyr(e);
  }
  function seekFromLyr(e: PointerEvent) {
    if (!dragging || player.duration <= 0) return;
    const target = e.currentTarget as HTMLElement;
    const rect = target.getBoundingClientRect();
    const pct = (e.clientY - rect.top) / rect.height;
    const secs = Math.max(0, Math.min(1, pct)) * player.duration;
    void invoke("seek", { positionSecs: secs });
    player.elapsed = secs;
  }
  function onLyrPointerMove(e: PointerEvent) {
    if (dragging) seekFromLyr(e);
  }
  function onLyrPointerUp(e: PointerEvent) {
    dragging = false;
    const el = e.currentTarget as HTMLElement;
    if (el?.hasPointerCapture?.(e.pointerId)) el.releasePointerCapture(e.pointerId);
  }

  // Apply user config to the lyrics block (alignment + font size).
  let LyricsStyle = $derived.by(() => {
    const align = settingsStore.settings.lyrics_align ?? "center";
    const size = settingsStore.settings.lyrics_font_size ?? 14;
    const ta = align === "left" ? "left" : align === "right" ? "right" : "center";
    return "text-align: " + ta + "; font-size: " + size + "px;";
  });

  // ── Playback rate popup ──────────────────────────
  let rateOpen = $state(false);
  let rateTimer: ReturnType<typeof setTimeout> | undefined;
  const rates = [0.5, 0.75, 1, 1.25, 1.5, 2];
  let rate = $state(1);

  function rateEnter() {
    clearTimeout(rateTimer);
    rateOpen = true;
  }
  function rateLeave() {
    rateTimer = setTimeout(() => (rateOpen = false), 150);
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

  <div class="detail-header">
    <div class="hd-spacer"></div>
    <button class="hd-btn" onclick={close} title="收起"><ChevronDown size={18} /></button>
    <button class="hd-btn" onclick={minimize} title="最小化"><Minus size={16} /></button>
    <button class="hd-btn" onclick={toggleMaximize} title="最大化"><Maximize size={15} /></button>
    <button class="hd-btn hd-close" onclick={closeWindow} title="关闭"><X size={16} /></button>
  </div>

  <div class="detail-main">
    <div class="detail-left">
      <div class="cover-large">
        {#if player.cover}
          <img src={player.cover} alt="" class="cover-img" />
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
      {#if lyricsLoading}
        <p class="lyrics-hint">加载歌词...</p>
      {:else if lyricsText}
        <div
          class="lyrics-content"
          style={LyricsStyle}
          oncontextmenu={openLyrMenu}
          onpointerdown={onLyrPointerDown}
          onpointermove={onLyrPointerMove}
          onpointerup={onLyrPointerUp}
          onpointercancel={onLyrPointerUp}
        >
          {#if lrcLines.length > 0}
            {#each lrcLines as lrc, i}
              <p
                class="lyrics-line"
                class:active={i === activeLineIdx}
                class:lyrics-empty={!lrc.text.trim()}
              >{lrc.text}</p>
            {/each}
          {:else}
            {#each lyricsText.split("\n") as line}
              <p class="lyrics-line" class:lyrics-empty={!line.trim()}>{line}</p>
            {/each}
          {/if}
        </div>
      {:else}
        <div class="lyrics-placeholder">
          <span class="lyrics-icon">&#9835;</span>
          <p class="lyrics-text">暂无歌词</p>
          <p class="lyrics-sub">歌词由插件系统提供</p>
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
            <div class="popup">
              {#each rates as r}
                <button class="pop-item" class:active={rate === r} onclick={() => { rate = r; rateOpen = false; }}>
                  {r}x
                </button>
              {/each}
            </div>
          {/if}
        </div>
        <VolumeControl />
        <PlayModeControl />
        <button class="d-btn" title="编辑标签" aria-label="编辑标签" onclick={openTagEditor}>
          <Tag size={16} />
        </button>
      </div>

      <!-- progress bar fills left width -->
      <div class="progress-track" role="slider" tabindex="0" aria-label="播放进度" onclick={seekTrack}>
        <div class="progress-fill" style="width: {progressPct}%"></div>
      </div>

      <!-- time, right-aligned -->
      <div class="time-row">
        <span class="lyric-status truncate">{lyricStatusText}</span>
        <span class="detail-time">{elapsedFormatted} / {durationFormatted}</span>
      </div>
    </div>

    <!-- Right block: transport buttons -->
    <div class="transport-group">
      <button class="d-btn big" title="上一首" disabled><SkipBack size={22} /></button>
      <button class="d-btn play-big" onclick={togglePlay} title="播放/暂停">
        {#if player.isPlaying}
          <Pause size={26} fill="currentColor" />
        {:else}
          <Play size={26} fill="currentColor" />
        {/if}
      </button>
      <button class="d-btn big" title="下一首" disabled><SkipForward size={22} /></button>
    </div>
  </div>
</div>

{#if tagSong}
  <TagEditor song={tagSong} onclose={closeTagEditor} />
{/if}

{#if lyrCtx}
  <div class="lyr-pop-backdrop" onclick={closeLyrMenu}></div>
  <div class="lyr-pop" style="left: {lyrCtx.x}px; top: {lyrCtx.y}px">
    <button class="lyr-btn" onclick={lyrDec} aria-label="缩小字体"><Minus size={15} /></button>
    <span class="lyr-size">{settingsStore.settings.lyrics_font_size ?? 14}px</span>
    <button class="lyr-btn" onclick={lyrInc} aria-label="放大字体"><Plus size={15} /></button>
  </div>
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
    background: linear-gradient(160deg, var(--color-base-300) / 0.4, var(--color-base-200));
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
    color: var(--color-base-content) / 0.75; background: none; border: none;
  }
  .hd-btn:hover { background: var(--color-base-300); color: var(--color-base-content); }
  .hd-close:hover { background: var(--color-error); color: var(--color-error-content); }

  .detail-main {
    flex: 1;
    display: flex;
    align-items: flex-start;
    justify-content: flex-start;
    gap: 80px;
    padding: 20px 60px 0 130px;
    min-height: 0;
    overflow-y: auto;
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
    color: var(--color-base-content) / 0.35;
    font-family: Consolas, "Courier New", monospace;
  }

  .meta {
    width: 100%; display: flex; flex-direction: column; gap: 4px;
    font-size: 15px; color: var(--color-base-content);
  }
  .meta-label { color: var(--color-base-content) / 0.55; margin-right: 4px; }

  .detail-lyrics {
    flex: 1; max-width: 420px; height: 100%; max-height: 72%;
    overflow-y: auto; display: flex; flex-direction: column; justify-content: center;
  }
  .lyrics-content { padding: 8px 0; line-height: 1.9; }
  .lyrics-line { font-size: 14px; color: var(--color-base-content) / 0.6; white-space: pre-wrap; word-break: break-word; transition: color 0.2s, transform 0.2s; }
  .lyrics-line.active { color: var(--color-base-content); transform: scale(1.18); font-weight: 600; }
  .lyrics-empty { height: 0.5em; }
  .lyrics-placeholder {
    display: flex; flex-direction: column; align-items: center; gap: 8px;
    color: var(--color-base-content) / 0.55; text-align: center;
  }
  .lyrics-icon { font-size: 40px; opacity: 0.4; }
  .lyrics-text { font-size: 14px; font-weight: 500; }
  .lyrics-sub { font-size: 11px; }
  .lyrics-hint { text-align: center; color: var(--color-base-content) / 0.55; }

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

  .progress-track {
    width: 100%; height: 6px; background: var(--color-base-300);
    border-radius: 3px; overflow: hidden; cursor: pointer; position: relative;
  }
  .progress-track::before { content: ""; position: absolute; inset: -8px 0; }
  .progress-fill { height: 100%; background: var(--color-primary); border-radius: 3px; pointer-events: none; transition: width 0.4s linear; }

  .time-row {
    display: flex; align-items: center; justify-content: space-between;
    gap: 12px;
  }
  .detail-time { font-size: 12px; color: var(--color-base-content) / 0.7; font-variant-numeric: tabular-nums; white-space: nowrap; }
  .lyric-status { font-size: 12px; color: var(--color-base-content) / 0.5; max-width: 60%; }

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
    background: none; border: none; color: var(--color-base-content) / 0.85;
    text-align: left;
  }
  .pop-item:hover { background: var(--color-base-200); color: var(--color-base-content); }
  .pop-item.active { color: var(--color-primary); }

  .d-btn {
    display: flex; align-items: center; justify-content: center;
    width: 38px; height: 38px; border-radius: 50%;
    color: var(--color-base-content) / 0.75;
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

  .lyr-pop-backdrop {
    position: fixed; inset: 0; z-index: 500; background: transparent;
  }
  .lyr-pop {
    position: fixed; z-index: 501;
    transform: translate(-50%, -100%);
    margin-top: -8px;
    display: flex; align-items: center; gap: 10px;
    background: var(--color-base-300);
    border: 1px solid var(--panel-border-strong);
    border-radius: 8px;
    padding: 6px 10px;
    box-shadow: var(--shadow-md);
  }
  .lyr-btn {
    display: flex; align-items: center; justify-content: center;
    width: 28px; height: 28px; border-radius: 6px;
    color: var(--color-base-content); background: none; border: none;
  }
  .lyr-btn:hover { background: var(--color-base-200); }
  .lyr-size {
    min-width: 44px; text-align: center;
    font-size: 13px; font-weight: 600;
    color: var(--color-primary); font-variant-numeric: tabular-nums;
  }
</style>
