<script lang="ts">
  import '../app.css';
  import TitleBar from '$lib/components/layout/TitleBar.svelte';
  import Sidebar from '$lib/components/layout/Sidebar.svelte';
  import PlayerBar from '$lib/components/layout/PlayerBar.svelte';

  let { children } = $props();
</script>

<div class="app-shell">
  <TitleBar />

  <div class="main-area">
    <Sidebar />
    <main class="content">
      {@render children()}
    </main>
  </div>

  <PlayerBar />
</div>

<style>
  .app-shell {
    display: grid;
    grid-template-rows: var(--titlebar-height) 1fr var(--playerbar-height);
    height: 100vh;
    background: var(--bg-base);
    color: var(--text-primary);
  }

  .main-area {
    display: grid;
    grid-template-columns: var(--sidebar-width) 1fr;
    overflow: hidden;
    min-height: 0;
  }

  .content {
    overflow-y: auto;
    overflow-x: hidden;
    background: var(--bg-base);
  }
</style>
