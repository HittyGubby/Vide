<script lang="ts">
  import { History, Loader, Search, X } from "lucide-svelte";
  import { searchPage } from "$lib/stores/searchPage.svelte";
  import { searchStore } from "$lib/stores/search.svelte";

  let open = $state(false);
  let wrapEl = $state<HTMLDivElement | null>(null);

  function submit() {
    if (!searchPage.query.trim()) return;
    open = false;
    window.dispatchEvent(new CustomEvent("vide:online-search"));
  }

  function showHistory() {
    open = true;
  }

  // Close only when the pointer lands outside the whole search box. Tapping
  // the panel itself (including its blank space) keeps it visible, so the
  // keyboard/autocomplete can be dismissed without losing the history.
  $effect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as Node | null;
      if (!wrapEl || (target && wrapEl.contains(target))) return;
      open = false;
    };
    document.addEventListener("pointerdown", onPointerDown, true);
    return () => document.removeEventListener("pointerdown", onPointerDown, true);
  });

  function use(value: string) {
    searchPage.query = value;
    submit();
  }
</script>

<div class="online-search" bind:this={wrapEl}>
  <Search size={14} class="icon" />
  <input
    type="text"
    placeholder="搜索歌曲或粘贴链接..."
    bind:value={searchPage.query}
    autocomplete="off"
    autocapitalize="off"
    spellcheck="false"
    onfocus={showHistory}
    onkeydown={(e) => {
      if (e.key === "Enter") submit();
      else if (e.key === "Escape") open = false;
    }}
  />
  {#if searchPage.loading}<Loader size={14} class="spin" />{/if}
  <button onclick={submit} disabled={!searchPage.query.trim()}>搜索</button>

  {#if open && !searchPage.loading}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="panel"
      role="listbox"
      onpointerdown={(event) => {
        // A tap on the panel's blank space drops focus (hiding the native
        // keyboard/autocomplete) but is not a reason to close the history.
        const target = event.target as HTMLElement | null;
        if (target && !target.closest("button")) {
          (document.activeElement as HTMLElement | null)?.blur();
        }
      }}
    >
      <div class="panel-head">
        <span class="panel-title"><History size={12} /> 搜索历史</span>
        {#if searchStore.history.length > 0}
          <button class="panel-clear" onclick={() => void searchStore.clear()}>清除</button>
        {/if}
      </div>
      {#if searchStore.history.length === 0}
        <div class="panel-empty">暂无搜索历史</div>
      {:else}
        {#each searchStore.history as value (value)}
          <div class="row">
            <button class="row-main" onclick={() => use(value)} title={value}>{value}</button>
            <button
              class="row-del"
              aria-label="移除 {value}"
              onclick={() => void searchStore.remove(value)}
            ><X size={12} /></button>
          </div>
        {/each}
      {/if}
    </div>
  {/if}
</div>

<style>
  .online-search {
    position: relative;
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1;
    min-width: 0;
    height: 28px;
    padding: 0 8px;
    border: 1px solid var(--panel-border);
    border-radius: 5px;
    background: var(--color-base-200);
    z-index: 30;
  }
  .icon { flex: none; color: color-mix(in oklch, var(--color-base-content) 50%, transparent); }
  input { flex: 1; min-width: 0; border: 0; background: transparent; color: inherit; font: inherit; font-size: 12px; outline: none; }
  button { flex: none; border: 0; background: transparent; color: var(--color-primary); font: inherit; font-size: 11px; cursor: pointer; }
  button:disabled { opacity: .4; }
  .spin { animation: spin .8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }

  .panel {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    z-index: 80;
    max-height: 320px;
    overflow-y: auto;
    padding: 4px;
    background: var(--color-base-300);
    border: 1px solid var(--panel-border-strong);
    border-radius: 8px;
    box-shadow: var(--shadow-md);
  }
  .panel-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 3px 6px 6px;
    border-bottom: 1px solid var(--panel-border);
  }
  .panel-title {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    color: color-mix(in oklch, var(--color-base-content) 60%, transparent);
  }
  .panel-clear {
    padding: 0;
    font-size: 10px;
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent);
  }
  .panel-clear:hover { color: var(--color-primary); }
  .panel-empty {
    padding: 10px 6px;
    font-size: 11px;
    text-align: center;
    color: color-mix(in oklch, var(--color-base-content) 45%, transparent);
  }
  .row { display: flex; align-items: center; gap: 2px; }
  .row-main {
    flex: 1;
    min-width: 0;
    padding: 7px 6px;
    border-radius: 5px;
    text-align: left;
    font-size: 12px;
    color: var(--color-base-content);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .row-main:hover { background: var(--color-base-200); }
  .row-del {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 5px;
    color: color-mix(in oklch, var(--color-base-content) 45%, transparent);
  }
  .row-del:hover { color: var(--color-error); background: var(--color-base-200); }
</style>
