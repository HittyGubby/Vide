<script lang="ts">
  import { isTauri } from "$lib/isTauri";
  import { platform } from "$lib/platform.svelte";
  import DesktopShell from "$lib/shell/DesktopShell.svelte";
  import WebShell from "$lib/shell/WebShell.svelte";
  import MobileShell from "$lib/shell/MobileShell.svelte";

  let { children } = $props();
</script>

{#if isTauri}
  <DesktopShell>{@render children()}</DesktopShell>
{:else if platform.mobileUI}
  <MobileShell>{@render children()}</MobileShell>
{:else}
  <WebShell>{@render children()}</WebShell>
{/if}
