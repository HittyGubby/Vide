<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { Pause, Play, SkipForward } from "lucide-svelte";
  import { player } from "$lib/stores/player.svelte";
  import { openPlayer } from "$lib/player/navigation";
  import SeekBar from "$lib/components/player/SeekBar.svelte";
  import { loadCoverWithFallback } from "$lib/cover";

  /**
   * Phone-sized player bar: cover, title/subtitle and the two controls most
   * used on the go. Everything else lives in the full player (P4).
   *
   * Tapping opens the player; long-pressing the title locates the song in the
   * library, the touch equivalent of right-clicking the desktop player bar.
   */
  const LONG_PRESS_MS = 500;

  onMount(() => {
    player.startStatusUpdates();
  });

  let subtitle = $derived(
    player.artist && player.artist !== "Unknown" ? player.artist : "",
  );

  function openDetail() {
    if (player.mediaId) openPlayer();
  }

  let pressTimer: ReturnType<typeof setTimeout> | undefined;
  let pressX = 0;
  let pressY = 0;
  let suppressClick = false;

  function cancelPress() {
    if (pressTimer) {
      clearTimeout(pressTimer);
      pressTimer = undefined;
    }
  }

  function startPress(e: PointerEvent) {
    if (e.pointerType === "mouse" || !player.mediaId) return;
    pressX = e.clientX;
    pressY = e.clientY;
    cancelPress();
    pressTimer = setTimeout(() => {
      pressTimer = undefined;
      suppressClick = true;
      void goto("/");
      player.requestLocate(player.mediaId!);
      setTimeout(() => (suppressClick = false), 700);
    }, LONG_PRESS_MS);
  }

  function movePress(e: PointerEvent) {
    if (!pressTimer) return;
    if (Math.abs(e.clientX - pressX) > 10 || Math.abs(e.clientY - pressY) > 10) {
      cancelPress();
    }
  }

  function handleOpen() {
    if (suppressClick) {
      suppressClick = false;
      return;
    }
    openDetail();
  }

  async function seek(secs: number) {
    await player.seekTo(secs);
  }
</script>

<footer class="mini-player">
  {#if player.mediaId && player.duration > 0}
    <div class="progress">
      <SeekBar
        elapsed={player.elapsed}
        duration={player.duration}
        playing={player.isPlaying}
        speed={player.speed}
        heightPx={2}
        knob={false}
        onseek={seek}
      />
    </div>
  {/if}

  <button class="cover" onclick={handleOpen} aria-label="播放详情">
    {#if player.cover}
      <img
        src={player.cover}
        alt=""
        onerror={(e) => loadCoverWithFallback(e.currentTarget as HTMLImageElement)}
      />
    {:else}
      <span class="monogram">V</span>
    {/if}
  </button>

  <div
    class="info"
    role="button"
    tabindex="0"
    onclick={handleOpen}
    onkeydown={(e) => {
      if (e.key === "Enter") handleOpen();
    }}
    onpointerdown={startPress}
    onpointermove={movePress}
    onpointerup={cancelPress}
    onpointercancel={cancelPress}
    onpointerleave={cancelPress}
  >
    <span class="title">{player.mediaId ? player.title : "未在播放"}</span>
    <span class="sub">{player.mediaId ? subtitle : "选择一首歌曲开始播放"}</span>
  </div>

  <button class="ctrl" aria-label="播放/暂停" onclick={() => void player.togglePause()}>
    {#if player.isPlaying}
      <Pause size={20} fill="currentColor" />
    {:else}
      <Play size={20} fill="currentColor" />
    {/if}
  </button>

  <button class="ctrl" aria-label="下一首" onclick={() => player.playNext()}>
    <SkipForward size={20} />
  </button>
</footer>

<style>
  .mini-player {
    position: relative;
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    height: calc(56px + env(safe-area-inset-bottom));
    padding: 0 6px 0 8px;
    padding-bottom: env(safe-area-inset-bottom);
    background: var(--color-base-200);
    border-top: 1px solid var(--panel-border-strong);
    user-select: none;
  }

  .progress {
    position: absolute;
    inset: 0 0 auto;
    z-index: 1;
  }
  .progress :global(.seekbar) {
    border-radius: 0;
    background: transparent;
  }
  .progress :global(.seek-track) {
    border-radius: 0;
  }

  .cover {
    flex: none;
    width: 42px;
    height: 42px;
    padding: 0;
    border: 0;
    border-radius: 6px;
    overflow: hidden;
    background: var(--color-base-300);
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .monogram {
    font-weight: 700;
    font-size: 16px;
    font-family: Consolas, "Courier New", monospace;
  }

  .info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 2px 0;
  }

  .title,
  .sub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .title {
    font-size: 13px;
    font-weight: 500;
  }

  .sub {
    font-size: 11px;
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent);
  }

  .ctrl {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 40px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: color-mix(in oklch, var(--color-base-content) 80%, transparent);
  }

  .ctrl:active {
    background: var(--color-base-300);
    color: var(--color-primary);
  }
</style>
