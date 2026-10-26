<script lang="ts">
  import { onMount } from "svelte";
  import { AlertTriangle, Check, Loader, RefreshCw } from "lucide-svelte";
  import { invoke } from "$lib/transport";
  import type { PluginState, PluginInfo, RouteInfo } from "$lib/types";
  import PluginCard from "$lib/components/settings/PluginCard.svelte";
  import RouteSettings from "$lib/components/settings/RouteSettings.svelte";
  import {
    findPlugin,
    parseArgs,
    pluginKey,
    routeArgvKey,
    routeKey,
  } from "$lib/components/settings/pluginHelpers";

  let pluginState = $state<PluginState | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let note = $state<string | null>(null);
  let expanded = $state<string | null>(null);
  let busy = $state<string | null>(null);

  let argvDrafts = $state<Record<string, string>>({});
  let platformDrafts = $state<Record<string, string>>({});
  let concurrencyDrafts = $state<Record<string, string>>({});

  let addSelections = $state<Record<string, string>>({});
  let routeArgvOpen = $state<string | null>(null);
  let routeArgvDrafts = $state<Record<string, string>>({});

  function ensureDrafts(plugin: PluginInfo) {
    if (!(plugin.name in argvDrafts)) {
      argvDrafts[plugin.name] = plugin.argv.join("\n");
    }
    if (!(plugin.name in concurrencyDrafts)) {
      concurrencyDrafts[plugin.name] =
        plugin.concurrency == null ? "" : String(plugin.concurrency);
    }
    for (const capability of plugin.capabilities) {
      const key = pluginKey(plugin.name, capability.platform);
      if (!(key in platformDrafts)) {
        platformDrafts[key] = (plugin.platform_argv[capability.platform] ?? []).join("\n");
      }
    }
  }

  function applyState(next: PluginState) {
    pluginState = next;
    for (const plugin of next.plugins) ensureDrafts(plugin);
  }

  async function loadPlugins(reload = false) {
    loading = true;
    error = null;
    note = null;
    try {
      const next = reload
        ? await invoke<PluginState>("reload_plugins")
        : await invoke<PluginState>("get_plugin_state");
      applyState(next);
      if (reload) note = "已重新扫描插件目录";
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function mutate(key: string, action: () => Promise<PluginState>): Promise<boolean> {
    busy = key;
    error = null;
    note = null;
    try {
      const next = await action();
      applyState(next);
      note = "已保存";
      return true;
    } catch (e) {
      error = String(e);
      return false;
    } finally {
      busy = null;
    }
  }

  function toggleExpanded(plugin: PluginInfo) {
    expanded = expanded === plugin.name ? null : plugin.name;
    ensureDrafts(plugin);
  }

  async function toggleEnabled(plugin: PluginInfo, enabled: boolean) {
    await mutate("enable:" + plugin.name, () =>
      invoke<PluginState>("set_plugin_enabled", { name: plugin.name, enabled }),
    );
  }

  function updatePluginDraft(next: PluginState, name: string) {
    const plugin = next.plugins.find((item) => item.name === name);
    if (!plugin) return;
    argvDrafts[name] = plugin.argv.join("\n");
    concurrencyDrafts[name] = plugin.concurrency == null ? "" : String(plugin.concurrency);
  }

  async function saveDefaultArgv(plugin: PluginInfo) {
    const argv = parseArgs(argvDrafts[plugin.name] ?? "");
    const ok = await mutate("argv:" + plugin.name, () =>
      invoke<PluginState>("set_plugin_argv", { name: plugin.name, argv }),
    );
    if (ok && pluginState) updatePluginDraft(pluginState, plugin.name);
  }

  async function resetDefaultArgv(plugin: PluginInfo) {
    argvDrafts[plugin.name] = "";
    await saveDefaultArgv(plugin);
  }

  async function savePlatformArgv(plugin: PluginInfo, platform: string) {
    const key = pluginKey(plugin.name, platform);
    const argv = parseArgs(platformDrafts[key] ?? "");
    const ok = await mutate("platform-argv:" + key, () =>
      invoke<PluginState>("set_plugin_platform_argv", {
        name: plugin.name,
        platform,
        argv,
      }),
    );
    if (ok && pluginState) {
      const next = pluginState.plugins.find((item) => item.name === plugin.name);
      platformDrafts[key] = (next?.platform_argv[platform] ?? []).join("\n");
    }
  }

  async function saveConcurrency(plugin: PluginInfo) {
    const raw = (concurrencyDrafts[plugin.name] ?? "").trim();
    const value = raw ? Number(raw) : null;
    if (raw && (!Number.isFinite(value) || (value as number) < 1)) {
      error = "并发上限必须是正整数";
      return;
    }
    const ok = await mutate("concurrency:" + plugin.name, () =>
      invoke<PluginState>("set_plugin_concurrency", {
        name: plugin.name,
        concurrency: value,
      }),
    );
    if (ok && pluginState) updatePluginDraft(pluginState, plugin.name);
  }

  let routeGroups = $derived.by(() => {
    const groups = new Map<string, RouteInfo[]>();
    for (const route of pluginState?.routes ?? []) {
      const list = groups.get(route.platform) ?? [];
      list.push(route);
      groups.set(route.platform, list);
    }
    return Array.from(groups.entries()).sort((a, b) => a[0].localeCompare(b[0]));
  });

  function setAddSelection(route: RouteInfo, value: string) {
    addSelections[routeKey(route)] = value;
  }

  function providerNames(route: RouteInfo): string[] {
    return route.providers.map((provider) => provider.plugin);
  }

  async function applyRoute(route: RouteInfo, names: string[], key: string) {
    const ok = await mutate(key, () =>
      invoke<PluginState>("set_plugin_route", {
        platform: route.platform,
        capability: route.capability,
        providers: names,
      }),
    );
    if (ok) addSelections[routeKey(route)] = "";
  }

  async function addProvider(route: RouteInfo) {
    const key = routeKey(route);
    const plugin = addSelections[key];
    if (!plugin) return;
    const names = providerNames(route);
    if (names.includes(plugin)) return;
    names.push(plugin);
    await applyRoute(route, names, "route:" + key);
  }

  async function removeProvider(route: RouteInfo, index: number) {
    const names = providerNames(route);
    names.splice(index, 1);
    await applyRoute(route, names, "route:" + routeKey(route));
  }

  async function moveProvider(route: RouteInfo, index: number, delta: number) {
    const names = providerNames(route);
    const target = index + delta;
    if (target < 0 || target >= names.length) return;
    const current = names[index];
    names[index] = names[target];
    names[target] = current;
    await applyRoute(route, names, "route:" + routeKey(route));
  }

  function toggleRouteArgv(route: RouteInfo, provider: { plugin: string }) {
    const key = routeArgvKey(route, provider.plugin);
    if (routeArgvOpen === key) {
      routeArgvOpen = null;
      return;
    }
    const plugin = findPlugin(pluginState?.plugins, provider.plugin);
    const current = plugin?.route_argv[routeKey(route)] ?? [];
    routeArgvDrafts[key] = current.join("\n");
    routeArgvOpen = key;
  }

  async function saveRouteArgv(route: RouteInfo, provider: { plugin: string }) {
    const key = routeArgvKey(route, provider.plugin);
    const argv = parseArgs(routeArgvDrafts[key] ?? "");
    const ok = await mutate("route-argv:" + key, () =>
      invoke<PluginState>("set_plugin_route_argv", {
        name: provider.plugin,
        platform: route.platform,
        capability: route.capability,
        argv,
      }),
    );
    if (ok && pluginState) {
      const plugin = pluginState.plugins.find((item) => item.name === provider.plugin);
      routeArgvDrafts[key] = (plugin?.route_argv[routeKey(route)] ?? []).join("\n");
      routeArgvOpen = null;
    }
  }

  async function resetRouteArgv(route: RouteInfo, provider: { plugin: string }) {
    const key = routeArgvKey(route, provider.plugin);
    routeArgvDrafts[key] = "";
    await saveRouteArgv(route, provider);
  }

  onMount(() => {
    void loadPlugins(false);
  });
</script>

<div class="plugin-body">
  <div class="plugin-toolbar">
    <button class="btn-mini" onclick={() => void loadPlugins(true)} disabled={loading || busy !== null}>
      <RefreshCw size={13} class={loading ? "spin" : ""} />
      <span>{loading ? "扫描中" : "重新扫描"}</span>
    </button>
  </div>

  {#if error}
    <div class="notice error">
      <AlertTriangle size={14} />
      <span>{error}</span>
    </div>
  {/if}
  {#if note}
    <div class="notice ok"><Check size={14} /><span>{note}</span></div>
  {/if}

  <div class="plugin-card">
    <div class="plugin-card-head">
      <div>
        <h3>已安装插件</h3>
        <p class="hint">把插件文件夹放入插件目录后点“重新扫描”，无需重启应用。</p>
      </div>
      <span class="count">{pluginState?.plugins.length ?? 0}</span>
    </div>
    {#if pluginState}
      <p class="dir-line">插件目录：{pluginState.plugin_dir}</p>
    {/if}

    {#if loading && !pluginState}
      <div class="empty"><Loader size={15} class="spin" /> 正在读取插件…</div>
    {:else if !pluginState}
      <div class="empty">插件系统不可用。请检查插件目录后重启。</div>
    {:else if pluginState.plugins.length === 0}
      <div class="empty">还没有安装插件。</div>
    {:else}
      <div class="plugin-list">
        {#each pluginState.plugins as plugin (plugin.name)}
          <PluginCard
            {plugin}
            expanded={expanded === plugin.name}
            {argvDrafts}
            {platformDrafts}
            {concurrencyDrafts}
            {busy}
            ontoggleenabled={toggleEnabled}
            ontoggleexpand={toggleExpanded}
            onsaveargv={saveDefaultArgv}
            onresetargv={resetDefaultArgv}
            onsaveplatform={savePlatformArgv}
            onsaveconcurrency={saveConcurrency}
          />
        {/each}
      </div>
    {/if}
  </div>

  <div class="plugin-card">
    <div class="plugin-card-head">
      <div>
        <h3>平台路由</h3>
        <p class="hint">每个平台每个能力只选一个 provider；列表顺序就是 fallback 顺序。</p>
      </div>
    </div>

    {#if !pluginState}
      <div class="empty">插件系统不可用。</div>
    {:else}
      <RouteSettings
        {routeGroups}
        plugins={pluginState.plugins}
        {routeArgvOpen}
        {routeArgvDrafts}
        {addSelections}
        {busy}
        onselect={setAddSelection}
        onadd={addProvider}
        onremove={removeProvider}
        onmove={moveProvider}
        ontoggleargv={toggleRouteArgv}
        onsaverouteargv={saveRouteArgv}
        onresetrouteargv={resetRouteArgv}
      />
    {/if}
  </div>
</div>

<style>
  .plugin-body {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .plugin-toolbar {
    display: flex;
    justify-content: flex-end;
  }
  .plugin-card {
    background: var(--color-base-200);
    border: 1px solid var(--panel-border);
    border-radius: 8px;
    padding: 10px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .plugin-card-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }
  .plugin-card-head h3 {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
  }
  .hint {
    margin: 3px 0 0;
    font-size: 11px;
    line-height: 1.5;
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent);
  }
  .count {
    font-size: 11px;
    color: color-mix(in oklch, var(--color-base-content) 45%, transparent);
  }
  .dir-line {
    margin: 0;
    font: 10px ui-monospace, "SFMono-Regular", Menlo, Consolas, monospace;
    color: color-mix(in oklch, var(--color-base-content) 50%, transparent);
    overflow-wrap: anywhere;
  }
  .plugin-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
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
  .notice {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    padding: 6px 9px;
    border-radius: 5px;
    font-size: 11px;
    line-height: 1.5;
    overflow-wrap: anywhere;
  }
  .notice.error {
    color: var(--color-error);
    background: color-mix(in oklch, var(--color-error) 10%, transparent);
    border: 1px solid color-mix(in oklch, var(--color-error) 25%, transparent);
  }
  .notice.ok {
    color: var(--color-success);
    background: color-mix(in oklch, var(--color-success) 10%, transparent);
    border: 1px solid color-mix(in oklch, var(--color-success) 25%, transparent);
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
  .spin {
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
