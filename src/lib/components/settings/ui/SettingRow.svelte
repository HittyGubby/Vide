<script lang="ts">
  import type { Snippet } from "svelte";

  /** Label on the left, control on the right; stacks on narrow screens. */
  let {
    label,
    hint = "",
    children,
  }: {
    label: string;
    hint?: string;
    children: Snippet;
  } = $props();
</script>

<div class="setting-row">
  <div class="setting-label">
    <span>{label}</span>
    {#if hint}
      <span class="setting-hint">{hint}</span>
    {/if}
  </div>
  <div class="setting-control">
    {@render children()}
  </div>
</div>

<style>
  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 8px 0;
    min-height: 34px;
  }

  .setting-row + :global(.setting-row) {
    border-top: 1px solid var(--panel-border);
  }

  .setting-label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 13px;
    color: var(--color-base-content);
    min-width: 0;
  }

  .setting-hint {
    font-size: 11px;
    line-height: 1.5;
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent);
  }

  .setting-control {
    flex: none;
    width: 240px;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 6px;
  }

  /* Phones: the control moves under its label and uses the full width. */
  @media (max-width: 560px) {
    .setting-row {
      flex-direction: column;
      align-items: stretch;
      gap: 6px;
    }
    .setting-control {
      width: 100%;
      justify-content: flex-start;
    }
  }
</style>
