<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/stores";
  import {
    LibraryBig,
    Search,
    Settings,
    LogIn,
    LogOut,
    Monitor,
    Server,
  } from "lucide-svelte";
  import Dialog from "$lib/ui/Dialog.svelte";
  import { auth } from "$lib/stores/auth.svelte";
  import { player } from "$lib/stores/player.svelte";
  import {
    browserLogout,
    getPlaybackTarget,
    setPlaybackTarget,
    type PlaybackTarget,
  } from "$lib/transport";

  let {
    open = $bindable(false),
    onlogin,
  }: {
    open?: boolean;
    onlogin: () => void;
  } = $props();

  const navItems = [
    { id: "library", icon: LibraryBig, label: "列表", route: "/" },
    { id: "search", icon: Search, label: "搜索", route: "/search" },
    { id: "settings", icon: Settings, label: "设置", route: "/settings" },
  ] as const;

  let rawPath = $derived(String($page.url.pathname));
  let path = $derived(rawPath === "" || rawPath === "/index.html" ? "/" : rawPath);
  let target = $state<PlaybackTarget>("client");

  // Read the target whenever the drawer opens; it can be changed elsewhere.
  $effect(() => {
    if (open) target = getPlaybackTarget();
  });

  function isActive(route: string): boolean {
    if (route === "/") return path === "/";
    return path.startsWith(route);
  }

  function navigate(route: string) {
    open = false;
    if (path !== route) void goto(route);
  }

  async function chooseTarget(next: PlaybackTarget) {
    if (target === next) return;
    await player.stop();
    target = next;
    setPlaybackTarget(next);
    player.restartStatusUpdates();
  }

  async function logout() {
    await browserLogout();
    auth.authenticated = false;
    open = false;
    window.location.reload();
  }
</script>

<Dialog bind:open placement="left" label="导航">
  <div class="drawer">
    <div class="drawer-head">
      <span class="logo">Vide</span>
    </div>

    <nav class="nav">
      {#each navItems as item}
        <button
          class="nav-item"
          class:active={isActive(item.route)}
          onclick={() => navigate(item.route)}
        >
          <item.icon size={18} strokeWidth={1.8} />
          <span>{item.label}</span>
        </button>
      {/each}
    </nav>

    <div class="drawer-foot">
      {#if auth.authenticated}
        <div class="target-row">
          <span class="target-label">播放位置</span>
          <span class="seg">
            <button class:on={target === "client"} onclick={() => void chooseTarget("client")}>
              <Monitor size={13} /> 浏览器
            </button>
            <button class:on={target === "server"} onclick={() => void chooseTarget("server")}>
              <Server size={13} /> 服务端
            </button>
          </span>
        </div>
        <div class="auth-row">
          <span class="auth-state">已登录</span>
          <button class="auth-btn" onclick={() => void logout()}>
            <LogOut size={15} /> 退出
          </button>
        </div>
      {:else}
        <button
          class="auth-btn primary"
          onclick={() => {
            open = false;
            onlogin();
          }}
        >
          <LogIn size={15} /> 登录
        </button>
        <p class="hint">只读浏览无需登录。</p>
      {/if}
    </div>
  </div>
</Dialog>

<style>
  .drawer {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding-top: env(safe-area-inset-top);
    padding-bottom: env(safe-area-inset-bottom);
  }

  .drawer-head {
    flex: none;
    display: flex;
    align-items: center;
    padding: 18px 18px 14px;
  }

  .logo {
    font-weight: 700;
    font-size: 18px;
    letter-spacing: 0.5px;
    color: var(--color-primary);
  }

  .nav {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 4px 8px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 14px;
    min-height: 48px;
    padding: 0 12px;
    border: 0;
    border-radius: 10px;
    background: none;
    color: color-mix(in oklch, var(--color-base-content) 72%, transparent);
    font-size: 14px;
    text-align: left;
    cursor: pointer;
  }

  .nav-item:hover {
    background: var(--color-base-300);
    color: var(--color-base-content);
  }

  .nav-item.active {
    background: color-mix(in oklch, var(--color-primary) 14%, transparent);
    color: var(--color-primary);
  }

  .drawer-foot {
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 16px 16px;
    border-top: 1px solid var(--panel-border);
  }

  .target-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .target-label {
    font-size: 12px;
    color: color-mix(in oklch, var(--color-base-content) 60%, transparent);
  }

  .seg {
    display: inline-flex;
    gap: 2px;
    padding: 2px;
    border-radius: 8px;
    background: var(--color-base-300);
  }

  .seg button {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 5px 9px;
    border: 0;
    border-radius: 6px;
    background: none;
    color: color-mix(in oklch, var(--color-base-content) 70%, transparent);
    font-size: 12px;
    cursor: pointer;
  }

  .seg button.on {
    background: var(--color-primary);
    color: var(--color-primary-content);
  }

  .auth-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .auth-state {
    font-size: 12px;
    color: var(--color-success);
  }

  .auth-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    min-height: 40px;
    padding: 0 14px;
    border: 1px solid var(--panel-border);
    border-radius: 8px;
    background: var(--color-base-300);
    color: inherit;
    font-size: 13px;
    cursor: pointer;
  }

  .auth-btn.primary {
    border-color: var(--color-primary);
    color: var(--color-primary);
  }

  .hint {
    margin: 0;
    font-size: 11px;
    color: color-mix(in oklch, var(--color-base-content) 50%, transparent);
  }
</style>
