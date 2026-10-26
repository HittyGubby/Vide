<script lang="ts">
  import type { Tag } from "$lib/types";
  import { X } from "lucide-svelte";

  /** Recently used tags; delete mode is local to this block. */
  let {
    history,
    onadd,
    onremove,
  }: {
    history: Tag[];
    onadd: (tag: Tag) => void;
    onremove: (tag: Tag) => void;
  } = $props();

  let deleteMode = $state(false);

  $effect(() => {
    if (history.length === 0) deleteMode = false;
  });
</script>

{#if history.length > 0}
  <div class="section-label history-head">
    <span>最近添加</span>
    <button
      class="history-mode"
      onclick={() => (deleteMode = !deleteMode)}
      aria-pressed={deleteMode}
    >{deleteMode ? "完成" : "删除历史"}</button>
  </div>
  <div class="history-list" class:deleting={deleteMode}>
    {#each history as tag}
      <span class="history-pill" class:deleting={deleteMode}>
        <button
          class="pill-add"
          disabled={deleteMode}
          onclick={() => onadd(tag)}
          title="添加 {tag.namespace}:{tag.value}"
        >{tag.namespace}:{tag.value}</button>
        {#if deleteMode}
          <button
            class="pill-del"
            aria-label="删除历史 {tag.namespace}:{tag.value}"
            title="从历史中删除"
            onclick={() => onremove(tag)}
          ><X size={11} /></button>
        {/if}
      </span>
    {/each}
  </div>
{/if}

<style>
  .section-label {
    margin-top: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--border-subtle);
    font-size: 10px;
    font-weight: 600;
    color: var(--text-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  .history-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .history-mode {
    border: 0;
    background: none;
    padding: 2px 0;
    font: inherit;
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0;
    text-transform: none;
    color: var(--text-tertiary);
    cursor: pointer;
  }
  .history-mode:hover {
    color: var(--accent-primary);
  }
  .history-list {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .history-pill {
    display: inline-flex;
    align-items: stretch;
    border-radius: 999px;
    overflow: hidden;
  }
  .pill-add {
    border: 1px solid var(--border-subtle);
    background: var(--bg-input);
    color: var(--text-secondary);
    border-radius: 999px;
    padding: 2px 8px;
    font-size: 11px;
    cursor: pointer;
  }
  .pill-add:hover:not(:disabled) {
    border-color: var(--accent-primary);
    color: var(--accent-primary);
    background: var(--accent-surface);
  }
  .pill-add:disabled {
    cursor: default;
    opacity: 0.55;
  }
  .history-pill.deleting .pill-add {
    border-top-right-radius: 0;
    border-bottom-right-radius: 0;
  }
  .pill-del {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 26px;
    padding: 0 6px;
    border: 1px solid var(--color-error);
    border-left: 0;
    background: color-mix(in oklch, var(--color-error) 12%, transparent);
    color: var(--color-error);
    cursor: pointer;
  }
  .pill-del:hover {
    background: color-mix(in oklch, var(--color-error) 22%, transparent);
  }
  @media (pointer: coarse) {
    .pill-del {
      min-width: 32px;
    }
    .pill-add {
      padding: 4px 10px;
    }
  }
</style>
