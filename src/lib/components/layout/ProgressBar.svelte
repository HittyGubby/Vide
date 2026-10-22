<script lang="ts">
  import { invoke } from "$lib/transport";
  import { player } from "$lib/stores/player.svelte";

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
      /* seek not supported in this mode */
    }
  }
</script>

<div class="progress-bar" class:visible={player.isPlaying}>
  <div class="progress-track" onclick={seekTrack}>
    <div class="progress-fill" style="width: {progressPct}%"></div>
  </div>
  <span class="time-display">
    {elapsedFormatted} / {durationFormatted}
  </span>
</div>

<style>
  .progress-bar {
    display: flex;
    align-items: center;
    height: 22px;
    padding: 0 12px;
    gap: 10px;
    background: var(--bg-base);
    border-top: 1px solid var(--border-subtle);
    opacity: 0.4;
    transition: opacity 0.2s;
    user-select: none;
  }

  .progress-bar.visible {
    opacity: 1;
  }

  .progress-track {
    flex: 1;
    height: 4px;
    background: var(--bg-elevated);
    border-radius: 2px;
    overflow: hidden;
    cursor: pointer;
    position: relative;
  }

  .progress-track:hover {
    height: 6px;
  }

  .progress-track::before {
    content: "";
    position: absolute;
    inset: -6px 0;
  }

  .progress-fill {
    height: 100%;
    background: var(--accent-primary);
    border-radius: 2px;
    transition: width 0.5s linear;
    pointer-events: none;
  }

  .time-display {
    font-size: 11px;
    color: var(--text-tertiary);
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
    min-width: 80px;
    text-align: right;
  }
</style>
