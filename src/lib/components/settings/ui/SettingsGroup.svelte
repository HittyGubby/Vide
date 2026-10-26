<script lang="ts">
  import type { Snippet } from "svelte";
  import { platform } from "$lib/platform.svelte";

  /** One settings section: title, optional description and header actions. */
  let {
    id,
    title,
    description = "",
    actions,
    children,
  }: {
    id: string;
    title: string;
    description?: string;
    actions?: Snippet;
    children: Snippet;
  } = $props();
</script>

<section class="setting-group" id="section-{id}">
  <!-- On phones the page header already shows the section name; keep the
       row only when there is an action to place there. -->
  {#if !platform.mobileUI || actions}
    <div class="setting-group-head">
      {#if !platform.mobileUI}
        <h2>{title}</h2>
      {/if}
      {#if actions}
        {@render actions()}
      {/if}
    </div>
  {/if}
  {#if description}
    <p class="setting-group-desc">{description}</p>
  {/if}
  {@render children()}
</section>

<style>
  .setting-group {
    display: flex;
    flex-direction: column;
    gap: 8px;
    /* Desktop anchor navigation must not hide the title under the sticky bar. */
    scroll-margin-top: 56px;
  }

  .setting-group-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding-bottom: 6px;
    border-bottom: 1px solid var(--panel-border);
  }

  h2 {
    margin: 0;
    font-size: 14px;
    font-weight: 700;
    color: var(--color-primary);
  }

  .setting-group-desc {
    margin: 0 0 2px;
    font-size: 11px;
    line-height: 1.5;
    color: color-mix(in oklch, var(--color-base-content) 60%, transparent);
  }
</style>
