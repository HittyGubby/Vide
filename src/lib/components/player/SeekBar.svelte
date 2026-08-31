<script lang="ts">
  import { onDestroy } from "svelte";

  interface Props {
    elapsed: number;
    duration: number;
    playing: boolean;
    onseek: (secs: number) => void;
    /** Playback rate — the bar advances at this multiple of wall-clock time. */
    speed?: number;
    /** Bar height in px. Default 5. Use ~2-3 for a slim strip. */
    heightPx?: number;
    /** Hide the hover drag handle knob. */
    knob?: boolean;
    /** Extra class applied to the root. */
    class?: string;
  }

  let { elapsed, duration, playing, onseek, speed = 1, heightPx = 5, knob = true, class: cls = "" }: Props = $props();

  // Local interpolation so the bar advances smoothly between polls.
  let displayElapsed = $state(elapsed);
  let lastBase = elapsed;
  let lastBaseAt = now();
  let rafId: number | null = null;

  function now(): number {
    return typeof performance !== "undefined" ? performance.now() : Date.now();
  }

  // Following lx-music-desktop: the bar only animates when the position jumps
  // by more than a couple of seconds (seek, track change, resume at a saved
  // position). Normal playback is advanced frame-by-frame instead, because a
  // CSS transition restarted every frame would just lag behind the audio.
  const JUMP_SECS = 2;
  const TWEEN_MS = 250;
  let tweenFrom = 0;
  let tweenTo = 0;
  let tweenStart = 0;
  let tweening = false;

  function startTween(to: number) {
    tweenFrom = displayElapsed;
    tweenTo = to;
    tweenStart = now();
    tweening = true;
  }

  function stopTween(at: number) {
    tweening = false;
    displayElapsed = at;
    lastBase = at;
    lastBaseAt = now();
  }

  // Track the last value seen from the backend so per-frame updates do not
  // retrigger the effect.
  let lastProp = elapsed;
  $effect(() => {
    const target = elapsed;
    if (target === lastProp) return;
    const jumped = Math.abs(target - lastProp) > JUMP_SECS;
    lastProp = target;
    if (jumped) startTween(target);
    else stopTween(target);
  });

  function tick() {
    if (tweening) {
      const p = Math.min(1, (now() - tweenStart) / TWEEN_MS);
      const eased = 1 - Math.pow(1 - p, 3);
      if (p >= 1) {
        stopTween(tweenTo);
      } else {
        displayElapsed = tweenFrom + (tweenTo - tweenFrom) * eased;
      }
    } else if (playing && duration > 0) {
      const dt = (now() - lastBaseAt) / 1000;
      // Advance at the playback rate so the bar moves at the real (speed-scaled)
      // media time. Without this, 2x audio would make the bar lag behind.
      let next = lastBase + dt * (speed || 1);
      if (next > duration) next = duration;
      displayElapsed = next;
    }
    rafId = requestAnimationFrame(tick);
  }
  rafId = requestAnimationFrame(tick);
  onDestroy(() => { if (rafId != null) cancelAnimationFrame(rafId); });

  let pct = $derived(duration > 0 ? Math.min(displayElapsed / duration, 1) : 0);

  let trackEl = $state<HTMLDivElement | null>(null);
  let dragging = $state(false);
  let dragPct = $state(0);

  function pctFromEvent(e: PointerEvent | MouseEvent): number {
    if (!trackEl) return 0;
    const rect = trackEl.getBoundingClientRect();
    let v = (e.clientX - rect.left) / rect.width;
    if (v < 0) v = 0;
    if (v > 1) v = 1;
    return v;
  }

  function onPointerDown(e: PointerEvent) {
    if (duration <= 0) return;
    dragging = true;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    const v = pctFromEvent(e);
    dragPct = v;
    displayElapsed = v * duration;
  }

  function onPointerMove(e: PointerEvent) {
    if (!dragging) return;
    const v = pctFromEvent(e);
    dragPct = v;
    displayElapsed = v * duration;
  }

  /** Commit a user seek: snap the bar there and keep polls from re-animating. */
  function commitSeek(secs: number) {
    stopTween(secs);
    lastProp = secs;
  }

  function onPointerUp(e: PointerEvent) {
    if (!dragging) return;
    dragging = false;
    const secs = pctFromEvent(e) * duration;
    commitSeek(secs);
    onseek(secs);
  }

  function onClick(e: MouseEvent) {
    if (duration <= 0) return;
    const secs = pctFromEvent(e) * duration;
    commitSeek(secs);
    onseek(secs);
  }

  let shownPct = $derived(dragging ? dragPct : pct);
  let shownStyle = $derived("transform: scaleX(" + shownPct + ")");
  let knobLeftPct = $derived(shownPct * 100);
  let rootStyle = $derived("height: " + heightPx + "px");
</script>

<div
  class="seekbar {cls}"
  style={rootStyle}
  role="slider"
  tabindex="0"
  aria-label="播放进度"
  aria-valuemin={0}
  aria-valuemax={duration}
  aria-valuenow={Math.round(displayElapsed)}
  bind:this={trackEl}
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  onpointercancel={onPointerUp}
  onclick={onClick}
>
  <div class="seek-track" style={rootStyle}></div>
  <div class="seek-fill" style={shownStyle}></div>
  {#if knob && duration > 0}
    <div class="seek-knob" style={"left: " + knobLeftPct + "%"}></div>
  {/if}
</div>

<style>
  .seekbar {
    position: relative;
    width: 100%;
    background: var(--color-base-300);
    border-radius: 3px;
    cursor: pointer;
    touch-action: none;
    user-select: none;
  }
  .seek-track {
    position: absolute;
    inset: 0;
    overflow: hidden;
    border-radius: 3px;
    width: 100%;
  }
  .seek-fill {
    position: absolute;
    left: 0;
    top: 0;
    width: 100%;
    height: 100%;
    background: var(--color-primary);
    border-radius: 3px;
    transform-origin: 0 50%;
    pointer-events: none;
    will-change: transform;
  }
  .seek-knob {
    position: absolute;
    top: 50%;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--color-base-100);
    border: 2px solid var(--color-primary);
    box-shadow: 0 0 3px rgba(0,0,0,0.4);
    transform: translate(-50%, -50%);
    pointer-events: none;
  }
  .seek-fill, .seek-knob { transition: none; }
  .seekbar:not(:hover) .seek-knob { opacity: 0; }
  .seekbar:hover .seek-knob { opacity: 1; }
</style>