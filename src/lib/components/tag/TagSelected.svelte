<script lang="ts">
  import type { Tag } from "$lib/types";
  import { X } from "lucide-svelte";

  let {
    groups,
    onremove,
  }: {
    groups: [string, Tag[]][];
    onremove: (namespace: string, value: string) => void;
  } = $props();
</script>

{#if groups.length === 0}
  <div class="no-tags">暂无标签</div>
{:else}
  {#each groups as [namespace, nsTags] (namespace)}
    <div class="ns-group">
      <span class="ns-label">{namespace}</span>
      <div class="ns-values">
        {#each nsTags as tag (tag.value)}
          <span class="tag-badge">
            <span>{tag.value}</span>
            <button
              class="tag-remove"
              onclick={() => onremove(tag.namespace, tag.value)}
              aria-label="移除 {tag.namespace}:{tag.value}"
            >
              <X size={12} />
            </button>
          </span>
        {/each}
      </div>
    </div>
  {/each}
{/if}

<style>
  .no-tags {
    font-size: 12px;
    color: var(--text-tertiary);
    text-align: center;
    padding: 8px 0;
  }
  .ns-group {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .ns-label {
    font-size: 10px;
    font-weight: 600;
    color: var(--text-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  .ns-values {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .tag-badge {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 2px 6px;
    border-radius: 4px;
    background: var(--accent-surface);
    color: var(--accent-primary);
    font-size: 11px;
  }
  .tag-remove {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    border-radius: 2px;
    color: var(--accent-primary);
    background: none;
    border: none;
    opacity: 0.6;
    transition: opacity 0.1s;
  }
  .tag-remove:hover {
    opacity: 1;
    background: rgba(255, 255, 255, 0.1);
  }
</style>
