<script lang="ts">
  import "../app.css";
  import { restoreRemoteSession } from "$lib/transport";
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { LYRICS_WINDOW_LABEL } from "$lib/desktopLyrics";
  import { isTauri } from "$lib/isTauri";
  import { platform } from "$lib/platform.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { refreshAuth } from "$lib/stores/auth.svelte";
  import { serverStatus } from "$lib/stores/serverStatus.svelte";
  import AppShell from "$lib/shell/AppShell.svelte";
  import { goto } from "$app/navigation";

  let { children } = $props();

  // This layout is shared with the desktop lyrics window, which must not run
  // the main window's shell/runtime; it renders bare.
  const isLyricsWindow =
    isTauri && getCurrentWindow().label === LYRICS_WINDOW_LABEL;

  onMount(() => {
    const stopPlatform = platform.init();

    // Restore the remote session before asking the server about its
    // capabilities (public media gates guest playback and downloads).
    void restoreRemoteSession().then(() => serverStatus.load());

    // The web client has no Tauri side to load these: settings hold the theme
    // and the lyrics stylesheet, auth holds the cookie session.
    if (!isTauri) {
      void settingsStore.load();
      void refreshAuth();
    }

    // Tauri/WebView can restore an /index.html or empty path after a restart.
    // Normalize it so the library route and its top-bar search mount.
    const path = window.location.pathname;
    if (path === "" || path === "/index.html") {
      void goto("/");
    }

    return stopPlatform;
  });
</script>

{#if isLyricsWindow}
  {@render children()}
{:else}
  <AppShell>{@render children()}</AppShell>
{/if}
