<script lang="ts">
  import { onMount } from "svelte";
  import { RefreshCw } from "lucide-svelte";
  import SettingsGroup from "$lib/components/settings/ui/SettingsGroup.svelte";
  import SettingCard from "$lib/components/settings/ui/SettingCard.svelte";
  import SettingRow from "$lib/components/settings/ui/SettingRow.svelte";
  import { invoke } from "$lib/transport";
  import type { CacheStat } from "$lib/types";

  let stats = $state<CacheStat[]>([]);
  let busy = $state<string | null>(null);
  let note = $state<string | null>(null);
  let loaded = $state(false);

  async function load() {
    try {
      stats = await invoke<CacheStat[]>("get_cache_stats");
      loaded = true;
    } catch (e) {
      console.error("load cache stats error:", e);
      note = "缓存统计加载失败";
      loaded = true;
    }
  }

  async function clearKind(kind: string) {
    busy = kind;
    note = null;
    try {
      const removed = await invoke<number>("clear_cache_kind", { kind });
      note = "已清除 " + removed + " 项";
      await load();
    } catch (e) {
      console.error("clear cache error:", e);
      note = "清除失败";
    } finally {
      busy = null;
    }
  }

  function formatBytes(bytes?: number | null): string {
    if (bytes == null) return "";
    const units = ["B", "KB", "MB", "GB", "TB"];
    let value = bytes;
    let index = 0;
    while (value >= 1024 && index < units.length - 1) {
      value /= 1024;
      index += 1;
    }
    const digits = index === 0 || value >= 100 ? 0 : value >= 10 ? 1 : 2;
    return value.toFixed(digits) + units[index];
  }

  onMount(() => {
    void load();
  });
</script>

{#snippet actions()}
  <button class="btn-mini" onclick={() => void load()}>
    <RefreshCw size={12} />
    <span>刷新</span>
  </button>
{/snippet}

<SettingsGroup id="cache" title="缓存管理" actions={actions}>
  <SettingCard>
    {#each stats as stat (stat.kind)}
      <SettingRow label={stat.label}>
        <span class="amount">
          {stat.count}{stat.size != null ? "（" + formatBytes(stat.size) + "）" : ""}
        </span>
        <button
          class="clear-btn"
          disabled={busy === stat.kind}
          onclick={() => void clearKind(stat.kind)}
        >
          {busy === stat.kind ? "清除中…" : "清除"}
        </button>
      </SettingRow>
    {:else}
      <p class="hint">{loaded ? "暂无缓存" : "加载中…"}</p>
    {/each}
    {#if note}
      <p class="hint">{note}</p>
    {/if}
  </SettingCard>
</SettingsGroup>

<style>
  .btn-mini {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 3px 9px;
    border: 1px solid var(--panel-border);
    border-radius: 6px;
    background: var(--color-base-200);
    color: var(--color-base-content);
    font-size: 11px;
    cursor: pointer;
  }
  .btn-mini:hover {
    border-color: var(--color-primary);
    color: var(--color-primary);
  }

  .amount {
    font-variant-numeric: tabular-nums;
    font-size: 12px;
    color: color-mix(in oklch, var(--color-base-content) 75%, transparent);
    white-space: nowrap;
  }

  .clear-btn {
    padding: 4px 12px;
    border: 1px solid var(--panel-border-strong, var(--panel-border));
    border-radius: 6px;
    background: var(--color-base-100);
    color: var(--color-base-content);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  .clear-btn:hover:not(:disabled) {
    border-color: var(--color-primary);
    color: var(--color-primary);
  }
  .clear-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .hint {
    margin: 0;
    padding: 4px 0;
    font-size: 11px;
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent);
  }

  @media (max-width: 560px) {
    .amount {
      flex: 1;
    }
  }
</style>
