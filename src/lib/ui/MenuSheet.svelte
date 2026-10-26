<script lang="ts">
  import Dialog from "$lib/ui/Dialog.svelte";
  import MenuList from "$lib/ui/MenuList.svelte";
  import type { MenuItem } from "$lib/ui/menu";

  /**
   * Touch counterpart of the desktop context menu. It takes exactly the same
   * `MenuItem[]`, so permissions and labels cannot diverge between layouts.
   */
  let {
    open = $bindable(false),
    title = "",
    items,
  }: {
    open?: boolean;
    title?: string;
    items: MenuItem[];
  } = $props();
</script>

<Dialog bind:open placement="bottom" label={title || "操作"}>
  {#if title}
    <div class="sheet-title">{title}</div>
  {/if}
  <MenuList {items} variant="sheet" onselect={() => (open = false)} />
  <button class="sheet-cancel" onclick={() => (open = false)}>取消</button>
</Dialog>

<style>
  .sheet-title {
    padding: 14px 18px 10px;
    font-size: 12px;
    color: color-mix(in oklch, var(--color-base-content) 60%, transparent);
    border-bottom: 1px solid var(--panel-border);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sheet-cancel {
    width: 100%;
    min-height: 52px;
    margin-top: 6px;
    border: 0;
    border-top: 8px solid var(--color-base-100);
    background: var(--color-base-200);
    color: var(--color-base-content);
    font-size: 14px;
    cursor: pointer;
  }
</style>
