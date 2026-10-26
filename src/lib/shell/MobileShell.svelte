<script lang="ts">
  import { page } from "$app/stores";
  import { Menu } from "lucide-svelte";
  import QueryBar from "$lib/components/library/QueryBar.svelte";
  import OnlineSearchBar from "$lib/components/search/OnlineSearchBar.svelte";
  import MiniPlayer from "$lib/components/layout/MiniPlayer.svelte";
  import NavDrawer from "$lib/shell/NavDrawer.svelte";
  import LoginDialog from "$lib/shell/LoginDialog.svelte";

  let { children } = $props();

  let drawerOpen = $state(false);
  let loginOpen = $state(false);

  let rawPath = $derived(String($page.url.pathname));
  let path = $derived(rawPath === "" || rawPath === "/index.html" ? "/" : rawPath);
  let title = $derived(
    path === "/search" ? "搜索" : path === "/settings" ? "设置" : "列表",
  );
  let showQuery = $derived(path === "/" || path === "/search");
</script>

<div class="mobile-shell">
  <header class="m-header">
    <button class="m-icon" aria-label="菜单" onclick={() => (drawerOpen = true)}>
      <Menu size={20} />
    </button>
    {#if showQuery}
      <div class="m-search">
        {#if path === "/search"}<OnlineSearchBar />{:else}<QueryBar />{/if}
      </div>
    {:else}
      <h1 class="m-title">{title}</h1>
    {/if}
  </header>

  <main class="m-content">
    {@render children()}
  </main>

  <MiniPlayer />
</div>

<NavDrawer bind:open={drawerOpen} onlogin={() => (loginOpen = true)} />
<LoginDialog bind:open={loginOpen} />

<style>
  .mobile-shell {
    display: flex;
    flex-direction: column;
    height: 100vh;
    height: 100dvh;
    background: var(--color-base-100);
    color: var(--color-base-content);
    overflow: hidden;
  }

  .m-header {
    flex: none;
    display: flex;
    align-items: center;
    gap: 6px;
    height: calc(48px + env(safe-area-inset-top));
    padding: env(safe-area-inset-top) 8px 0;
    background: var(--color-base-200);
    border-bottom: 1px solid var(--panel-border);
  }

  .m-icon {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: color-mix(in oklch, var(--color-base-content) 80%, transparent);
    cursor: pointer;
  }

  .m-icon:active {
    background: var(--color-base-300);
  }

  .m-search {
    flex: 1;
    min-width: 0;
    height: 36px;
    display: flex;
    align-items: center;
  }

  .m-title {
    flex: 1;
    min-width: 0;
    margin: 0;
    padding-left: 4px;
    font-size: 15px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .m-content {
    flex: 1;
    min-height: 0;
    overflow-x: hidden;
    overflow-y: auto;
  }
</style>
