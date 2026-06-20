<script lang="ts">
  import type { ComponentType } from "svelte";

  export interface MenuItem {
    label: string;
    icon?: ComponentType;
    action: () => void;
    disabled?: boolean;
    divider?: boolean;
  }

  let {
    items,
    x,
    y,
    onclose,
  }: {
    items: MenuItem[];
    x: number;
    y: number;
    onclose: () => void;
  } = $props();

  let menuEl = $state<HTMLDivElement | null>(null);
  let styleX = $state(x);
  let styleY = $state(y);

  $effect(() => {
    if (menuEl) {
      const rect = menuEl.getBoundingClientRect();
      if (rect.right > window.innerWidth - 8) {
        styleX = window.innerWidth - rect.width - 8;
      }
      if (rect.bottom > window.innerHeight - 8) {
        styleY = window.innerHeight - rect.height - 8;
      }
    }
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="backdrop"
  onclick={onclose}
  oncontextmenu={(e) => { e.preventDefault(); onclose(); }}
  role="presentation"
></div>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="context-menu"
  bind:this={menuEl}
  style="left: {styleX}px; top: {styleY}px"
  role="menu"
  onkeydown={(e) => { if (e.key === "Escape") onclose(); }}
>
  {#each items as item}
    {#if item.divider}
      <div class="divider" role="separator"></div>
    {:else}
      <button
        class="menu-item"
        disabled={item.disabled}
        onclick={() => { item.action(); onclose(); }}
        role="menuitem"
      >
        {#if item.icon}
          <span class="menu-icon"><svelte:component this={item.icon} size={15} /></span>
        {/if}
        <span class="menu-label">{item.label}</span>
      </button>
    {/if}
  {/each}
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 600;
    background: transparent;
  }

  .context-menu {
    position: fixed;
    z-index: 601;
    min-width: 160px;
    max-width: 240px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-normal);
    border-radius: 8px;
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.5);
    padding: 4px;
    overflow: hidden;
  }

  .menu-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 10px;
    border-radius: 5px;
    background: none;
    border: none;
    color: var(--text-primary);
    font-size: 12.5px;
    text-align: left;
    cursor: pointer;
  }

  .menu-item:hover:not(:disabled) {
    background: var(--accent-surface);
    color: var(--accent-primary);
  }

  .menu-item:disabled {
    opacity: 0.35;
    cursor: default;
  }

  .menu-icon {
    display: flex;
    align-items: center;
    width: 18px;
    justify-content: center;
    color: var(--text-tertiary);
    flex-shrink: 0;
  }

  .menu-item:hover .menu-icon {
    color: var(--accent-primary);
  }

  .menu-label {
    flex: 1;
  }

  .divider {
    height: 1px;
    background: var(--border-subtle);
    margin: 4px 8px;
  }
</style>
