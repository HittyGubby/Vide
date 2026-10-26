<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { LogIn, LogOut } from "lucide-svelte";
  import Sidebar from "$lib/components/layout/Sidebar.svelte";
  import PlayerBar from "$lib/components/layout/PlayerBar.svelte";
  import QueryBar from "$lib/components/library/QueryBar.svelte";
  import OnlineSearchBar from "$lib/components/search/OnlineSearchBar.svelte";
  import PlayerDetail from "$lib/components/player/PlayerDetail.svelte";
  import LoginDialog from "$lib/shell/LoginDialog.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { player } from "$lib/stores/player.svelte";
  import { auth } from "$lib/stores/auth.svelte";
  import {
    browserLogout,
    getPlaybackTarget,
    setPlaybackTarget,
    type PlaybackTarget,
  } from "$lib/transport";

  let { children } = $props();
  let loginOpen = $state(false);
  let target = $state<PlaybackTarget>("client");

  let rawPath = $derived(String($page.url.pathname));
  let path = $derived(rawPath === "" || rawPath === "/index.html" ? "/" : rawPath);
  let onLibrary = $derived(path === "/");
  let onSearch = $derived(path === "/search");

  onMount(() => {
    target = getPlaybackTarget();
  });

  async function chooseTarget(next: PlaybackTarget) {
    await player.stop();
    target = next;
    setPlaybackTarget(next);
    player.restartStatusUpdates();
  }

  async function logout() {
    await browserLogout();
    auth.authenticated = false;
    window.location.reload();
  }
</script>

<div class="web-shell">
  <Sidebar />
  <div class="web-body">
    <header class="web-topbar">
      {#if onLibrary}
        <div class="web-query"><QueryBar /></div>
      {:else if onSearch}
        <div class="web-query"><OnlineSearchBar /></div>
      {/if}
      <div class="web-auth">
        {#if auth.authenticated}
          <div class="target-switch" title="播放位置">
            <button class:active={target === "client"} onclick={() => void chooseTarget("client")}>浏览器</button>
            <button class:active={target === "server"} onclick={() => void chooseTarget("server")}>服务端</button>
          </div>
          <span class="auth-state">已登录</span>
          <button class="auth-btn" onclick={() => void logout()}><LogOut size={14} /> 退出</button>
        {:else}
          <button class="auth-btn" onclick={() => (loginOpen = true)}><LogIn size={14} /> 登录</button>
        {/if}
      </div>
    </header>
    <main class="web-content">{@render children()}</main>
    {#if ui.showPlayerDetail}
      <PlayerDetail />
    {:else}
      <PlayerBar />
    {/if}
  </div>
</div>

<LoginDialog bind:open={loginOpen} />

<style>
  .web-shell {
    display: flex;
    height: 100vh;
    background: var(--color-base-100);
    color: var(--color-base-content);
  }
  .web-body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .web-topbar {
    height: 40px;
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 16px;
    border-bottom: 1px solid var(--panel-border);
  }
  .web-query {
    flex: 1;
    min-width: 0;
  }
  .web-auth {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .auth-state {
    font-size: 11px;
    color: var(--color-success);
  }
  .target-switch {
    display: inline-flex;
    border: 1px solid var(--panel-border);
    border-radius: 5px;
    overflow: hidden;
  }
  .target-switch button {
    border: 0;
    background: transparent;
    color: color-mix(in oklch, var(--color-base-content) 60%, transparent);
    font: inherit;
    font-size: 10px;
    padding: 3px 7px;
    cursor: pointer;
  }
  .target-switch button.active {
    background: var(--color-primary);
    color: var(--color-primary-content);
  }
  .auth-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 4px 9px;
    border: 1px solid var(--panel-border);
    border-radius: 5px;
    background: var(--color-base-200);
    color: inherit;
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }
  .auth-btn:hover {
    border-color: var(--color-primary);
    color: var(--color-primary);
  }
  .web-content {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
</style>
