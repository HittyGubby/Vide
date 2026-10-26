<script lang="ts">
  import type { Snippet } from "svelte";
  let { children }: { children: Snippet } = $props();
</script>

<div class="setting-card">
  {@render children()}
</div>

<style>
  .setting-card {
    background: var(--color-base-200);
    border: 1px solid var(--panel-border);
    border-radius: 8px;
    padding: 8px 14px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
</style>
