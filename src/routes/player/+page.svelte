<script lang="ts">
  import { platform } from "$lib/platform.svelte";
  import PlayerDetail from "$lib/components/player/PlayerDetail.svelte";
  import MobilePlayerDetail from "$lib/components/player/MobilePlayerDetail.svelte";
</script>

{#if platform.mobileUI}
  <MobilePlayerDetail />
{:else}
  <PlayerDetail />
{/if}
