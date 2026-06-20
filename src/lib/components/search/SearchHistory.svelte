<script lang="ts">
  import { X, Clock, Search } from "lucide-svelte";
  import { searchStore } from "$lib/stores/search.svelte";

  let {
    onselect,
    onclose,
  }: {
    onselect: (q: string) => void;
    onclose: () => void;
  } = $props();

  function handleSelect(q: string) {
    onselect(q);
    onclose();
  }

  function handleRemove(e: Event, q: string) {
    e.stopPropagation();
    searchStore.remove(q);
  }
</script>

{#if searchStore.history.length > 0}
  <!-- eslint-disable-next-line svelte/no-at-html-tags -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="history-dropdown" onmouseleave={onclose}>
    <div class="history-header">
      <span class="history-title">搜索历史</span>
      <button
        class="clear-btn"
        onclick={() => {
          searchStore.clear();
          onclose();
        }}>清除</button
      >
    </div>
    <div class="history-list" role="listbox">
      {#each searchStore.history as item}
        <button
          class="history-item"
          onclick={() => handleSelect(item)}
          role="option"
        >
          <Clock size={14} class="item-icon" />
          <span class="item-text truncate">{item}</span>
          <span
            class="item-remove"
            role="button"
            tabindex="0"
            onclick={(e) => handleRemove(e, item)}
            onkeydown={(e) => {
              if (e.key === "Enter" || e.key === " ")
                handleRemove(e as any, item);
            }}
            aria-label="删除 {item}"
          >
            <X size={12} />
          </span>
        </button>
      {/each}
    </div>
  </div>
{/if}

<style>
  .history-dropdown {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    background: var(--bg-elevated);
    border: 1px solid var(--border-normal);
    border-top: none;
    border-radius: 0 0 8px 8px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
    z-index: 50;
    overflow: hidden;
  }

  .history-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px 4px;
  }

  .history-title {
    font-size: 10px;
    font-weight: 600;
    color: var(--text-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .clear-btn {
    font-size: 10px;
    color: var(--text-tertiary);
    background: none;
    border: none;
    padding: 2px 6px;
    border-radius: 3px;
  }

  .clear-btn:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  .history-list {
    max-height: 240px;
    overflow-y: auto;
    padding: 2px 0;
  }

  .history-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 12px;
    background: none;
    border: none;
    color: var(--text-primary);
    font-size: 12.5px;
    text-align: left;
    cursor: pointer;
  }

  .history-item:hover {
    background: var(--bg-hover);
  }

  .item-icon {
    color: var(--text-tertiary);
    flex-shrink: 0;
  }

  .item-text {
    flex: 1;
  }

  .item-remove {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border-radius: 3px;
    color: var(--text-tertiary);
    background: none;
    border: none;
    opacity: 0;
    transition: opacity 0.1s;
    cursor: pointer;
  }

  .history-item:hover .item-remove {
    opacity: 1;
  }

  .item-remove:hover {
    background: var(--bg-active);
    color: var(--text-primary);
  }
</style>
