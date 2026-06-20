<script lang="ts">
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { Search, ListMusic, Settings } from 'lucide-svelte';

  const navItems = [
    { id: 'search', icon: Search, label: '搜索', route: '/' },
    { id: 'playlists', icon: ListMusic, label: '列表', route: '/playlists' },
    { id: 'settings', icon: Settings, label: '设置', route: '/settings' },
  ];

  let currentPath = $derived($page.url.pathname);

  function navigate(route: string) {
    goto(route);
  }
</script>

<nav class="sidebar">
  <div class="nav-items">
    {#each navItems as item}
      <button
        class="nav-btn"
        class:active={currentPath === item.route}
        onclick={() => navigate(item.route)}
        title={item.label}
      >
        <item.icon size={18} />
        <span class="nav-label">{item.label}</span>
      </button>
    {/each}
  </div>
</nav>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    width: var(--sidebar-width);
    background: var(--bg-surface);
    border-right: 1px solid var(--border-subtle);
    overflow: hidden;
  }

  .nav-items {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: 8px 0;
  }

  .nav-btn {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    width: 40px;
    height: 44px;
    border-radius: 8px;
    color: var(--text-secondary);
    background: none;
    border: none;
    transition: background 0.15s, color 0.15s;
  }

  .nav-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .nav-btn:active {
    background: var(--bg-active);
  }

  .nav-btn.active {
    color: var(--accent-primary);
    background: var(--accent-surface);
  }

  .nav-label {
    font-size: 9px;
    line-height: 1;
    letter-spacing: 0.3px;
  }
</style>
