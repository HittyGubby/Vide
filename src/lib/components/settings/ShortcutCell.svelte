<script lang="ts">
  import { acceleratorFromEvent, formatAccelerator } from "$lib/shortcuts";

  let {
    value,
    onchange,
  }: { value: string; onchange: (next: string) => void } = $props();

  let capturing = $state(false);
  let btnEl = $state<HTMLButtonElement | null>(null);

  function startCapture() {
    capturing = true;
    // Keep the keyboard focus here so the captured keys land on this cell.
    btnEl?.focus();
  }

  function onKeyDown(e: KeyboardEvent) {
    if (!capturing) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") {
      capturing = false;
      return;
    }
    if (e.key === "Backspace" || e.key === "Delete") {
      capturing = false;
      onchange("");
      return;
    }
    // A bare modifier press is the start of a combination, not the shortcut.
    const next = acceleratorFromEvent(e);
    if (next) {
      capturing = false;
      onchange(next);
    }
  }
</script>

<svelte:window onkeydown={onKeyDown} />

<button
  bind:this={btnEl}
  class="sc-cell"
  class:capturing
  data-shortcut-capture={capturing ? "" : undefined}
  onclick={startCapture}
  title="点击后按下新的组合键（Backspace 清除，Esc 取消）"
>
  {capturing ? "按下快捷键…" : formatAccelerator(value)}
</button>

<style>
  .sc-cell {
    width: 100%;
    padding: 5px 10px;
    border: 1px solid var(--panel-border, rgba(128, 128, 128, 0.3));
    border-radius: 6px;
    background: var(--color-base-100);
    color: var(--color-base-content);
    font: inherit;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    text-align: center;
    cursor: pointer;
    white-space: nowrap;
    transition: border-color 0.12s, background 0.12s;
  }
  .sc-cell:hover {
    border-color: var(--color-primary);
  }
  .sc-cell.capturing {
    border-color: var(--color-primary);
    background: color-mix(in oklch, var(--color-primary) 12%, transparent);
    color: var(--color-primary);
  }
</style>