<script lang="ts">
  import { ArrowDown, ArrowUp, Plus, X } from "lucide-svelte";
  import type { PluginInfo, RouteInfo } from "$lib/types";
  import {
    availableProviders,
    capabilityLabel,
    inheritedRouteArgv,
    openProviderFor,
    routeArgvKey,
    routeKey,
  } from "$lib/components/settings/pluginHelpers";

  /** Per-platform provider order + per-route argv overrides. */
  let {
    routeGroups,
    plugins,
    routeArgvOpen,
    routeArgvDrafts,
    addSelections,
    busy,
    onselect,
    onadd,
    onremove,
    onmove,
    ontoggleargv,
    onsaverouteargv,
    onresetrouteargv,
  }: {
    routeGroups: [string, RouteInfo[]][];
    plugins: PluginInfo[] | undefined;
    routeArgvOpen: string | null;
    routeArgvDrafts: Record<string, string>;
    addSelections: Record<string, string>;
    busy: string | null;
    onselect: (route: RouteInfo, value: string) => void;
    onadd: (route: RouteInfo) => void;
    onremove: (route: RouteInfo, index: number) => void;
    onmove: (route: RouteInfo, index: number, delta: number) => void;
    ontoggleargv: (route: RouteInfo, provider: { plugin: string }) => void;
    onsaverouteargv: (route: RouteInfo, provider: { plugin: string }) => void;
    onresetrouteargv: (route: RouteInfo, provider: { plugin: string }) => void;
  } = $props();
</script>

{#if routeGroups.length === 0}
  <div class="empty">还没有可配置的路由。</div>
{:else}
  <div class="route-groups">
    {#each routeGroups as [platform, routes] (platform)}
      <div class="route-platform">
        <h4>{platform}</h4>
        {#each routes as route (route.capability)}
          {@const key = routeKey(route)}
          {@const open = openProviderFor(route, routeArgvOpen)}
          <div class="route-row">
            <div class="route-cap">{capabilityLabel(route.capability)}</div>
            <div class="route-providers">
              {#each route.providers as provider, index (provider.plugin)}
                <div class="provider" class:off={!provider.enabled}>
                  <span class="order">{index + 1}</span>
                  <span class="provider-name" class:muted={!provider.enabled}>{provider.plugin}</span>
                  <button
                    class="mini-btn"
                    title="上移"
                    disabled={busy !== null || index === 0}
                    onclick={() => onmove(route, index, -1)}
                  ><ArrowUp size={11} /></button>
                  <button
                    class="mini-btn"
                    title="下移"
                    disabled={busy !== null || index === route.providers.length - 1}
                    onclick={() => onmove(route, index, 1)}
                  ><ArrowDown size={11} /></button>
                  <button
                    class="mini-btn"
                    class:active={routeArgvOpen === routeArgvKey(route, provider.plugin)}
                    title="路由 argv 覆盖"
                    disabled={busy !== null}
                    onclick={() => ontoggleargv(route, provider)}
                  >argv</button>
                  <button
                    class="mini-btn danger"
                    title="移除"
                    disabled={busy !== null}
                    onclick={() => onremove(route, index)}
                  ><X size={11} /></button>
                </div>
              {/each}
              {#if route.providers.length === 0}
                <span class="muted small">未配置</span>
              {/if}
            </div>
            <div class="route-add">
              {#if availableProviders(route, plugins).length > 0}
                <select
                  class="select select-xs select-bordered route-select"
                  value={addSelections[key] ?? ""}
                  onchange={(e) => onselect(route, e.currentTarget.value)}
                >
                  <option value="">加 provider…</option>
                  {#each availableProviders(route, plugins) as candidate (candidate.name)}
                    <option value={candidate.name}>
                      {candidate.name}{candidate.enabled ? "" : " (off)"}
                    </option>
                  {/each}
                </select>
                <button
                  class="btn-mini"
                  disabled={busy !== null || !addSelections[key]}
                  onclick={() => onadd(route)}
                ><Plus size={12} /></button>
              {:else}
                <span class="muted small">无可用</span>
              {/if}
            </div>
          </div>

          {#if open}
            {@const arKey = routeArgvKey(route, open.plugin)}
            <div class="route-argv">
              <div class="field-head">
                <span>{open.plugin} · 路由 argv 覆盖</span>
                <div class="field-actions">
                  <button
                    class="btn-mini"
                    disabled={busy !== null}
                    onclick={() => onresetrouteargv(route, open)}
                  >恢复继承</button>
                  <button
                    class="btn-mini primary"
                    disabled={busy !== null}
                    onclick={() => onsaverouteargv(route, open)}
                  >保存</button>
                </div>
              </div>
              <textarea
                rows="2"
                spellcheck="false"
                placeholder={inheritedRouteArgv(route, open.plugin, plugins)}
                value={routeArgvDrafts[arKey] ?? ""}
                oninput={(e) => (routeArgvDrafts[arKey] = e.currentTarget.value)}
              ></textarea>
            </div>
          {/if}
        {/each}
      </div>
    {/each}
  </div>
{/if}

<style>
  .route-groups {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .route-platform {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .route-platform h4 {
    margin: 0;
    font-size: 12px;
    font-weight: 600;
    color: var(--color-primary);
  }
  .route-row {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    flex-wrap: wrap;
    padding: 6px 0;
    border-top: 1px solid var(--panel-border);
  }
  .route-platform .route-row:first-of-type {
    border-top: none;
  }
  .route-cap {
    flex: none;
    width: 125px;
    padding-top: 3px;
    font-size: 11px;
    color: color-mix(in oklch, var(--color-base-content) 70%, transparent);
  }
  .route-providers {
    flex: 1 1 220px;
    min-width: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
  }
  .provider {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 2px 4px;
    border-radius: 5px;
    background: var(--color-base-100);
    border: 1px solid var(--panel-border);
    min-width: 0;
  }
  .provider.off {
    opacity: 0.55;
  }
  .order {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: color-mix(in oklch, var(--color-primary) 15%, transparent);
    color: var(--color-primary);
    font-size: 9px;
    font-weight: 600;
  }
  .provider-name {
    font-size: 10.5px;
    max-width: 140px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .muted {
    color: color-mix(in oklch, var(--color-base-content) 40%, transparent);
  }
  .small {
    font-size: 10.5px;
  }
  .empty {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 14px;
    font-size: 11.5px;
    color: color-mix(in oklch, var(--color-base-content) 50%, transparent);
  }
  .mini-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 16px;
    height: 16px;
    padding: 0 3px;
    border: 0;
    border-radius: 3px;
    background: transparent;
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent);
    font-size: 9px;
    font-family: inherit;
    cursor: pointer;
  }
  .mini-btn:hover:not(:disabled) {
    background: color-mix(in oklch, var(--color-primary) 15%, transparent);
    color: var(--color-primary);
  }
  .mini-btn.active {
    color: var(--color-primary);
    background: color-mix(in oklch, var(--color-primary) 15%, transparent);
  }
  .mini-btn.danger:hover:not(:disabled) {
    color: var(--color-error);
    background: color-mix(in oklch, var(--color-error) 12%, transparent);
  }
  .mini-btn:disabled {
    opacity: 0.3;
    cursor: default;
  }
  .route-add {
    flex: 0 1 180px;
    display: flex;
    align-items: center;
    gap: 3px;
  }
  .route-select {
    min-width: 0;
    max-width: 160px;
    height: 1.9rem;
    min-height: 1.9rem;
    font-size: 10.5px;
    font-family: inherit;
  }
  .route-argv {
    margin: 2px 0 4px 133px;
    display: flex;
    flex-direction: column;
    gap: 3px;
    flex-basis: 100%;
  }
  .field-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    font-size: 11px;
    color: color-mix(in oklch, var(--color-base-content) 70%, transparent);
  }
  .field-actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  textarea {
    width: 100%;
    box-sizing: border-box;
    font: 11px ui-monospace, "SFMono-Regular", Menlo, Consolas, monospace;
    line-height: 1.4;
    color: var(--color-base-content);
    background: var(--color-base-100);
    border: 1px solid var(--panel-border);
    border-radius: 5px;
    padding: 5px 7px;
    resize: vertical;
  }
  textarea:focus {
    outline: none;
    border-color: var(--color-primary);
  }
  .btn-mini {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 2px 7px;
    border-radius: 4px;
    border: 1px solid var(--panel-border);
    background: var(--color-base-100);
    color: color-mix(in oklch, var(--color-base-content) 80%, transparent);
    font-size: 10.5px;
    line-height: 1.5;
    font-family: inherit;
    cursor: pointer;
  }
  .btn-mini:hover:not(:disabled) {
    border-color: var(--color-primary);
    color: var(--color-primary);
  }
  .btn-mini:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .btn-mini.primary {
    border-color: color-mix(in oklch, var(--color-primary) 45%, transparent);
    color: var(--color-primary);
  }
  @media (max-width: 560px) {
    .route-argv {
      margin-left: 0;
    }
    .route-cap {
      width: 100%;
    }
  }
</style>
