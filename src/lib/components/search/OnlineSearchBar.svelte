<script lang="ts">
  import { Search, Loader } from "lucide-svelte";
  import { searchPage } from "$lib/stores/searchPage.svelte";

  function submit() {
    if (!searchPage.query.trim()) return;
    window.dispatchEvent(new CustomEvent("vide:online-search"));
  }
</script>

<div class="online-search">
  <Search size={14} class="icon" />
  <input
    type="text"
    placeholder="搜索歌曲或粘贴链接..."
    bind:value={searchPage.query}
    onkeydown={(e) => { if (e.key === "Enter") submit(); }}
  />
  {#if searchPage.loading}<Loader size={14} class="spin" />{/if}
  <button onclick={submit} disabled={!searchPage.query.trim()}>搜索</button>
</div>

<style>
  .online-search {
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
  }
  .icon { flex: none; color: color-mix(in oklch, var(--color-base-content) 50%, transparent); }
  input { flex: 1; min-width: 0; border: 0; background: transparent; color: inherit; font: inherit; font-size: 12px; outline: none; }
  button { flex: none; border: 0; background: transparent; color: var(--color-primary); font: inherit; font-size: 11px; cursor: pointer; }
  button:disabled { opacity: .4; }
  .spin { animation: spin .8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
