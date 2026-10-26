<script lang="ts">
  import MenuList from "$lib/ui/MenuList.svelte";
  import type { MenuItem } from "$lib/ui/menu";

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
  <MenuList {items} onselect={onclose} />
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
</style>
