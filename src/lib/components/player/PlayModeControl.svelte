<script lang="ts">
  import { Repeat, ListMusic, Shuffle } from "lucide-svelte";
  import { player } from "$lib/stores/player.svelte";

  let open = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  // Bind to the shared player store; the Rust backend owns sequencing.
  let single = $derived(player.playMode.single);
  let reverse = $derived(player.playMode.reverse);
  let shuffle = $derived(player.playMode.shuffle);
  let repeat = $derived(player.playMode.repeat);

  function setMode(patch: Partial<{ single: boolean; reverse: boolean; shuffle: boolean; repeat: boolean }>) {
    player.playMode = { ...player.playMode, ...patch };
    // Push the new mode to the Rust backend immediately.
    player.syncPlayMode();
  }

  function enter() {
    clearTimeout(timer);
    open = true;
  }
  function leave() {
    timer = setTimeout(() => (open = false), 150);
  }

  let modeName = $derived.by(() => {
    const scope = single ? "单曲" : "列表";
    const order = shuffle ? "随机" : reverse ? "逆序" : "按序";
    const tail = repeat ? "循环" : "不循环";
    return scope + " / " + order + " / " + tail;
  });
</script>

<div class="pop-wrap" onmouseenter={enter} onmouseleave={leave}>
  <button class="tool-btn" title="播放顺序" aria-label="播放顺序" onclick={() => (open = !open)}>
    <Repeat size={16} />
  </button>

  {#if open}
    <div class="popup mode-popup">
      <div class="mode-title">{modeName}</div>

      <!-- Toggler 1: 单曲 / 列表 -->
      <div class="toggle-row">
        <span class="toggle-label">单曲 / 列表</span>
        <span class="seg">
          <button class="seg-btn" class:on={single} onclick={() => setMode({ single: true })}>单曲</button>
          <button class="seg-btn" class:on={!single} onclick={() => setMode({ single: false })}>列表</button>
        </span>
      </div>

      <!-- Toggler 2: 按序 / 随机 -->
      <div class="toggle-row">
        <span class="toggle-label">按序 / 随机</span>
        <span class="seg">
          <button class="seg-btn" class:on={!shuffle} onclick={() => setMode({ shuffle: false, reverse: false })}>按序</button>
          <button class="seg-btn" class:on={shuffle} onclick={() => setMode({ shuffle: true })}>随机</button>
        </span>
      </div>

      <!-- Toggler 3: 顺序 / 逆序 (only relevant in order mode) -->
      {#if !shuffle}
      <div class="toggle-row">
        <span class="toggle-label">顺序 / 逆序</span>
        <span class="seg">
          <button class="seg-btn" class:on={!reverse} onclick={() => setMode({ reverse: false })}>顺序</button>
          <button class="seg-btn" class:on={reverse} onclick={() => setMode({ reverse: true })}>逆序</button>
        </span>
      </div>
      {/if}

      <!-- Toggler 4: 播完暂停 / 循环 -->
      <div class="toggle-row">
        <span class="toggle-label">播完暂停 / 循环</span>
        <span class="seg">
          <button class="seg-btn" class:on={!repeat} onclick={() => setMode({ repeat: false })}>暂停</button>
          <button class="seg-btn" class:on={repeat} onclick={() => setMode({ repeat: true })}>循环</button>
        </span>
      </div>
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
    color: color-mix(in oklch, var(--color-base-content) 70%, transparent);
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
    padding: 8px;
    box-shadow: var(--shadow-md);
    z-index: 70;
    white-space: nowrap;
  }

  .mode-popup {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 210px;
  }

  .mode-title {
    font-size: 12px;
    font-weight: 600;
    color: var(--color-primary);
    padding: 2px 4px 4px;
    border-bottom: 1px solid var(--panel-border);
    margin-bottom: 2px;
  }

  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 3px 4px;
  }

  .toggle-label {
    font-size: 11px;
    color: color-mix(in oklch, var(--color-base-content) 70%, transparent);
  }

  .seg {
    display: inline-flex;
    gap: 2px;
    background: var(--color-base-200);
    border-radius: 6px;
    padding: 2px;
  }

  .seg-btn {
    padding: 4px 10px;
    border-radius: 4px;
    font-size: 12px;
    color: color-mix(in oklch, var(--color-base-content) 70%, transparent);
    background: none;
    border: none;
  }
  .seg-btn:hover { color: var(--color-base-content); }
  .seg-btn.on {
    background: var(--color-base-100);
    color: var(--color-primary);
    font-weight: 600;
  }
</style>
