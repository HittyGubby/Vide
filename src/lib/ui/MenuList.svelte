<script lang="ts">
  import type { MenuItem } from "$lib/ui/menu";
  import { ChevronRight } from "lucide-svelte";

  let {
    items,
    onselect,
    variant = "menu",
  }: {
    items: MenuItem[];
    onselect?: () => void;
    /** `menu` for popovers, `sheet` for the taller touch rows. */
    variant?: "menu" | "sheet";
  } = $props();
</script>

<div class="menu-list {variant}" role="menu">
  {#each items as item}
    {#if item.divider}
      <div class="divider" role="separator"></div>
    {:else}
      {@const Icon = item.icon}
      <button
        class="menu-item"
        disabled={item.disabled}
        role="menuitem"
        onclick={() => {
          item.action();
          onselect?.();
        }}
      >
        {#if Icon}
          <span class="menu-icon"><Icon size={variant === "sheet" ? 17 : 15} /></span>
        {/if}
        <span class="menu-label">{item.label}</span>
        {#if variant === "sheet"}
          <ChevronRight size={15} class="chev" />
        {/if}
      </button>
    {/if}
  {/each}
</div>

<style>
  .menu-list {
    display: flex;
    flex-direction: column;
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
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .divider {
    height: 1px;
    background: var(--border-subtle);
    margin: 4px 8px;
  }

  /* Touch rows: taller, bigger text, chevron instead of a hover highlight. */
  .sheet .menu-item {
    min-height: 48px;
    padding: 0 18px;
    border-radius: 0;
    font-size: 14px;
  }
  .sheet .menu-item:hover:not(:disabled) {
    background: var(--color-base-300);
    color: var(--color-base-content);
  }
  .sheet .menu-icon {
    width: 24px;
    color: var(--color-base-content);
  }
  .sheet :global(.chev) {
    color: var(--color-base-content);
    opacity: 0.35;
  }
  .sheet .divider {
    margin: 6px 18px;
  }
</style>
