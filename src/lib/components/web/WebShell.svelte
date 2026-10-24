<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { LogIn, LogOut, Loader } from "lucide-svelte";
  import Sidebar from "$lib/components/layout/Sidebar.svelte";
  import PlayerBar from "$lib/components/layout/PlayerBar.svelte";
  import QueryBar from "$lib/components/library/QueryBar.svelte";
  import OnlineSearchBar from "$lib/components/search/OnlineSearchBar.svelte";
  import PlayerDetail from "$lib/components/player/PlayerDetail.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { player } from "$lib/stores/player.svelte";
  import {
    browserAuthStatus,
    browserLogin,
    browserLogout,
    getPlaybackTarget,
    setPlaybackTarget,
    type PlaybackTarget,
  } from "$lib/transport";
  import { auth } from "$lib/stores/auth.svelte";

  let { children } = $props();
  let authenticated = $state(false);
  let loginOpen = $state(false);
  let password = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);
  let target = $state<PlaybackTarget>("client");
  let path = $derived(String($page.url.pathname) === "" || String($page.url.pathname) === "/index.html" ? "/" : String($page.url.pathname));
  let onLibrary = $derived(path === "/");
  let onSearch = $derived(path === "/search");

  onMount(() => {
    target = getPlaybackTarget();
    void browserAuthStatus().then((value) => {
      authenticated = value;
      auth.authenticated = value;
    });
  });

  async function login() {
    busy = true;
    error = null;
    try {
      await browserLogin(password);
      authenticated = true;
      auth.authenticated = true;
      password = "";
      loginOpen = false;
      window.location.reload();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function chooseTarget(next: PlaybackTarget) {
    await player.stop();
    target = next;
    setPlaybackTarget(next);
    player.restartStatusUpdates();
  }

  async function logout() {
    await browserLogout();
    authenticated = false;
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
        {#if authenticated}
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

{#if loginOpen}
  <div class="login-backdrop" role="presentation" onclick={() => (loginOpen = false)}>
    <div class="login-panel" role="dialog" aria-label="登录" onclick={(e) => e.stopPropagation()}>
      <h2>登录 Vide Server</h2>
      <p>只读浏览无需登录；播放控制、标签和数据修改需要登录。</p>
      <input
        type="password"
        bind:value={password}
        placeholder="密码"
        onkeydown={(e) => { if (e.key === "Enter") void login(); }}
      />
      {#if error}<div class="login-error">{error}</div>{/if}
      <div class="login-actions">
        <button onclick={() => (loginOpen = false)}>取消</button>
        <button class="primary" onclick={() => void login()} disabled={busy || !password}>
          {#if busy}<Loader size={13} class="spin" />{:else}登录{/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .web-shell { display: flex; height: 100vh; background: var(--color-base-100); color: var(--color-base-content); }
  .web-body { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .web-topbar { height: 40px; flex: none; display: flex; align-items: center; gap: 10px; padding: 0 16px; border-bottom: 1px solid var(--panel-border); }
  .web-query { flex: 1; min-width: 0; }
  .web-auth { margin-left: auto; display: flex; align-items: center; gap: 8px; }
  .auth-state { font-size: 11px; color: var(--color-success); }
  .target-switch { display: inline-flex; border: 1px solid var(--panel-border); border-radius: 5px; overflow: hidden; }
  .target-switch button { border: 0; background: transparent; color: var(--color-base-content) / 0.6; font: inherit; font-size: 10px; padding: 3px 7px; cursor: pointer; }
  .target-switch button.active { background: var(--color-primary); color: var(--color-primary-content); }
  .auth-btn { display: inline-flex; align-items: center; gap: 4px; padding: 4px 9px; border: 1px solid var(--panel-border); border-radius: 5px; background: var(--color-base-200); color: inherit; font: inherit; font-size: 11px; cursor: pointer; }
  .auth-btn:hover { border-color: var(--color-primary); color: var(--color-primary); }
  .web-content { flex: 1; min-height: 0; overflow: auto; }
  .login-backdrop { position: fixed; inset: 0; z-index: 900; background: rgba(0,0,0,.55); display: flex; align-items: center; justify-content: center; }
  .login-panel { width: min(360px, calc(100vw - 32px)); display: flex; flex-direction: column; gap: 9px; padding: 18px; border-radius: 10px; background: var(--color-base-200); border: 1px solid var(--panel-border); }
  .login-panel h2 { margin: 0; font-size: 15px; }
  .login-panel p { margin: 0; font-size: 11px; line-height: 1.5; color: var(--color-base-content) / 0.55; }
  .login-panel input { padding: 7px 9px; border: 1px solid var(--panel-border); border-radius: 5px; background: var(--color-base-100); color: inherit; font: inherit; }
  .login-panel input:focus { outline: none; border-color: var(--color-primary); }
  .login-error { color: var(--color-error); font-size: 11px; }
  .login-actions { display: flex; justify-content: flex-end; gap: 6px; }
  .login-actions button { padding: 4px 9px; border: 1px solid var(--panel-border); border-radius: 5px; background: var(--color-base-200); color: inherit; cursor: pointer; }
  .login-actions .primary { border-color: var(--color-primary); color: var(--color-primary); }
  .spin { animation: spin .8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
