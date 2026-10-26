<script lang="ts">
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { Search, LibraryBig, Settings } from "lucide-svelte";
  let navItems = $derived([
    { id: "search", icon: Search, label: "搜索", route: "/search" },
    { id: "library", icon: LibraryBig, label: "列表", route: "/" },
    { id: "settings", icon: Settings, label: "设置", route: "/settings" },
  ]);

  let currentPath = $derived($page.url.pathname);

  function isActive(route: string): boolean {
    if (route === "/") return currentPath === "/";
    return currentPath.startsWith(route);
  }

  function navigate(item: (typeof navItems)[number]) {
    goto(item.route);
  }
</script>

<nav class="sidebar">
  <div class="sidebar-top">
    <span class="app-logo">Vide</span>
  </div>

  <div class="nav-items">
    {#each navItems as item}
      <button
        class="nav-btn"
        class:active={isActive(item.route)}
        onclick={() => navigate(item)}
        title={item.label}
        aria-label={item.label}
      >
        <item.icon size={20} strokeWidth={1.8} />
        <span class="nav-label">{item.label}</span>
      </button>
    {/each}
  </div>
</nav>

<style>
  .sidebar {
    width: 60px;
    height: 100%;
    background: var(--color-base-200);
    border-right: 1px solid var(--panel-border-strong);
    user-select: none;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
  }

  .sidebar-top {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    height: var(--titlebar-height);
    border-bottom: 1px solid var(--panel-border);
    -webkit-app-region: drag;
  }

  .app-logo {
    font-weight: 700;
    font-size: 15px;
    letter-spacing: 0.5px;
    color: var(--color-primary);
  }

  .nav-items {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: 10px 0;
  }

  .nav-btn {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    width: 44px;
    height: 48px;
    border-radius: 8px;
    color: color-mix(in oklch, var(--color-base-content) 65%, transparent);
    background: none;
    border: none;
    transition: background 0.15s, color 0.15s;
  }

  .nav-btn:hover {
    background: var(--color-base-300);
    color: var(--color-base-content);
  }

  .nav-btn.active {
    color: var(--color-primary);
    background: color-mix(in oklch, var(--color-primary) 12%, transparent);
  }

  .nav-label {
    font-size: 9px;
    line-height: 1;
    letter-spacing: 0.3px;
  }
</style>
