<script lang="ts">
  interface TagGroup {
    namespace: string;
    children: string[];
  }

  /** All known tag values grouped by namespace; collapsed state is local. */
  let {
    groups,
    onadd,
  }: {
    groups: TagGroup[];
    onadd: (namespace: string, value: string) => void;
  } = $props();

  let expanded = $state<Set<string>>(new Set());
  let initialized = false;

  $effect(() => {
    if (initialized || groups.length === 0) return;
    initialized = true;
    expanded = new Set(groups.map((group) => group.namespace));
  });

  function toggle(namespace: string) {
    const next = new Set(expanded);
    if (next.has(namespace)) next.delete(namespace);
    else next.add(namespace);
    expanded = next;
  }
</script>

{#if groups.length > 0}
  <div class="section-label">全部标签</div>
  <div class="catalog-list">
    {#each groups as group (group.namespace)}
      <div class="catalog-group">
        <button class="catalog-ns" onclick={() => toggle(group.namespace)}>
          <span class="chevron">{expanded.has(group.namespace) ? "▾" : "▸"}</span>
          {group.namespace}
        </button>
        {#if expanded.has(group.namespace)}
          <div class="catalog-values">
            {#each group.children as value}
              <button
                class="catalog-value"
                onclick={() => onadd(group.namespace, value)}
                title="添加 {group.namespace}:{value}"
              >{value}</button>
            {/each}
          </div>
        {/if}
      </div>
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
  .catalog-list {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .catalog-group {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .catalog-ns {
    display: flex;
    align-items: center;
    gap: 4px;
    align-self: flex-start;
    border: none;
    background: none;
    color: var(--text-primary);
    font-size: 11px;
    font-weight: 600;
    padding: 1px 0;
    cursor: pointer;
  }
  .chevron {
    width: 10px;
    color: var(--text-tertiary);
    font-size: 10px;
  }
  .catalog-values {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .catalog-value {
    border: 1px solid var(--border-subtle);
    background: var(--bg-input);
    color: var(--text-secondary);
    border-radius: 999px;
    padding: 2px 8px;
    font-size: 11px;
    cursor: pointer;
  }
  .catalog-value:hover {
    border-color: var(--accent-primary);
    color: var(--accent-primary);
    background: var(--accent-surface);
  }
</style>
