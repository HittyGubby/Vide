<script lang="ts">
  import { Volume2, VolumeX } from "lucide-svelte";

  let open = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let volume = $state(80);
  let muted = $state(false);

  function enter() {
    clearTimeout(timer);
    open = true;
  }
  function leave() {
    timer = setTimeout(() => (open = false), 150);
  }
  function toggleMute() {
    muted = !muted;
  }
</script>

<div class="pop-wrap" onmouseenter={enter} onmouseleave={leave}>
  <button class="tool-btn" title="音量" aria-label="音量" onclick={() => (open = !open)}>
    {#if muted || volume === 0}
      <VolumeX size={16} />
    {:else}
      <Volume2 size={16} />
    {/if}
  </button>

  {#if open}
    <div class="popup vol-popup">
      <div class="vol-info">
        <span class="vol-pct">{muted ? 0 : Math.round(volume)}%</span>
        <label class="mute-label">
          <input type="checkbox" checked={muted} onclick={toggleMute} class="mute-check" />
          <span>静音</span>
        </label>
      </div>
      <input
        type="range"
        min="0"
        max="100"
        bind:value={volume}
        class="vol-slider"
        aria-label="音量"
      />
    </div>
  {/if}
</div>

<style>
  .pop-wrap {
    position: relative;
    display: flex;
    align-items: center;
    -webkit-app-region: no-drag;
  }

  .tool-btn {
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
  .tool-btn:hover {
    background: var(--color-base-300);
    color: var(--color-base-content);
  }

  .popup {
    position: absolute;
    bottom: 40px;
    left: 50%;
    transform: translateX(-50%);
    background: var(--color-base-300);
    border: 1px solid var(--panel-border-strong);
    border-radius: 8px;
    padding: 10px 12px;
    box-shadow: var(--shadow-md);
    z-index: 70;
    white-space: nowrap;
  }

  .vol-popup {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 150px;
  }

  .vol-info {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 12px;
    color: var(--color-base-content);
  }

  .vol-pct {
    font-variant-numeric: tabular-nums;
  }

  .mute-label {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    color: var(--color-base-content) / 0.75;
    cursor: pointer;
  }

  .mute-check {
    accent-color: var(--color-primary);
  }

  .vol-slider {
    width: 100%;
    accent-color: var(--color-primary);
  }
</style>
