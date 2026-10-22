<script lang="ts">
  import { invoke } from "$lib/transport";
  import { onMount } from "svelte";
  import {
    AlertTriangle,
    ArrowDown,
    ArrowUp,
    Check,
    ChevronDown,
    ChevronRight,
    Loader,
    Plus,
    RefreshCw,
    X,
  } from "lucide-svelte";
  import type {
    PluginInfo,
    PluginState,
    RouteInfo,
    RouteProviderInfo,
  } from "$lib/types";

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

  const capabilityLabels: Record<string, string> = {
    search: "搜索",
    resolve: "解析",
    playlist: "歌单",
    stream: "音频",
    lyrics: "歌词",
  };

  const allCapabilities = ["search", "resolve", "stream", "playlist", "lyrics"];

  function capabilityLabel(capability: string): string {
    return capabilityLabels[capability] ?? capability;
  }

  function capabilityGroups(plugin: PluginInfo): {
    platform: string;
    all: boolean;
    labels: string[];
  }[] {
    const groups = new Map<string, string[]>();
    for (const item of plugin.capabilities) {
      const capabilities = groups.get(item.platform) ?? [];
      if (!capabilities.includes(item.capability)) capabilities.push(item.capability);
      groups.set(item.platform, capabilities);
    }
    return Array.from(groups.entries())
      .sort((a, b) => a[0].localeCompare(b[0]))
      .map(([platform, capabilities]) => {
        const all = allCapabilities.every((capability) => capabilities.includes(capability));
        return {
          platform,
          all,
          labels: capabilities
            .slice()
            .sort((a, b) => allCapabilities.indexOf(a) - allCapabilities.indexOf(b))
            .map(capabilityLabel),
        };
      });
  }

  function pluginKey(name: string, platform: string): string {
    return name + "::" + platform;
  }

  function routeKey(route: RouteInfo): string {
    return route.platform + "/" + route.capability;
  }

  function routeArgvKey(route: RouteInfo, plugin: string): string {
    return routeKey(route) + "::" + plugin;
  }

  function parseArgs(text: string): string[] {
    return text
      .split("\n")
      .map((line) => (line.endsWith("\r") ? line.slice(0, -1) : line))
      .map((line) => line.trim())
      .filter((line) => line.length > 0);
  }

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
    for (const plugin of next.plugins) {
      ensureDrafts(plugin);
    }
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

  async function mutate(
    key: string,
    action: () => Promise<PluginState>,
  ): Promise<boolean> {
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
      invoke<PluginState>("set_plugin_enabled", {
        name: plugin.name,
        enabled,
      }),
    );
  }

  function updatePluginDraft(next: PluginState, name: string) {
    const plugin = next.plugins.find((item) => item.name === name);
    if (!plugin) return;
    argvDrafts[name] = plugin.argv.join("\n");
    concurrencyDrafts[name] =
      plugin.concurrency == null ? "" : String(plugin.concurrency);
  }

  async function saveDefaultArgv(plugin: PluginInfo) {
    const argv = parseArgs(argvDrafts[plugin.name] ?? "");
    const ok = await mutate("argv:" + plugin.name, () =>
      invoke<PluginState>("set_plugin_argv", {
        name: plugin.name,
        argv,
      }),
    );
    if (ok && pluginState) updatePluginDraft(pluginState, plugin.name);
  }

  async function resetDefaultArgv(plugin: PluginInfo) {
    argvDrafts[plugin.name] = "";
    await saveDefaultArgv(plugin);
  }

  function inheritedDefault(plugin: PluginInfo): string {
    return plugin.default_argv.join("\n");
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

  function platformPlaceholder(plugin: PluginInfo): string {
    return (plugin.argv.length ? plugin.argv : plugin.default_argv).join("\n");
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
    return Array.from(groups.entries()).sort((a, b) =>
      a[0].localeCompare(b[0]),
    );
  });

  function findPlugin(name: string): PluginInfo | undefined {
    return pluginState?.plugins.find((plugin) => plugin.name === name);
  }

  function availableProviders(route: RouteInfo): PluginInfo[] {
    const used = new Set(route.providers.map((provider) => provider.plugin));
    return (pluginState?.plugins ?? []).filter(
      (plugin) =>
        !used.has(plugin.name) &&
        plugin.capabilities.some(
          (capability) =>
            capability.platform === route.platform &&
            capability.capability === route.capability,
        ),
    );
  }

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
    if (ok) {
      addSelections[routeKey(route)] = "";
    }
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

  function openProviderFor(route: RouteInfo): RouteProviderInfo | null {
    if (!routeArgvOpen) return null;
    const prefix = routeKey(route) + "::";
    if (!routeArgvOpen.startsWith(prefix)) return null;
    const plugin = routeArgvOpen.slice(prefix.length);
    return route.providers.find((provider) => provider.plugin === plugin) ?? null;
  }

  function toggleRouteArgv(route: RouteInfo, provider: RouteProviderInfo) {
    const key = routeArgvKey(route, provider.plugin);
    if (routeArgvOpen === key) {
      routeArgvOpen = null;
      return;
    }
    const plugin = findPlugin(provider.plugin);
    const current = plugin?.route_argv[routeKey(route)] ?? [];
    routeArgvDrafts[key] = current.join("\n");
    routeArgvOpen = key;
  }

  function inheritedRouteArgv(route: RouteInfo, pluginName: string): string {
    const plugin = findPlugin(pluginName);
    if (!plugin) return "";
    const platformArgv = plugin.platform_argv[route.platform];
    if (platformArgv && platformArgv.length > 0) {
      return platformArgv.join("\n");
    }
    return (plugin.argv.length ? plugin.argv : plugin.default_argv).join("\n");
  }

  async function saveRouteArgv(route: RouteInfo, provider: RouteProviderInfo) {
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

  async function resetRouteArgv(route: RouteInfo, provider: RouteProviderInfo) {
    const key = routeArgvKey(route, provider.plugin);
    routeArgvDrafts[key] = "";
    await saveRouteArgv(route, provider);
  }

  onMount(() => {
    void loadPlugins(false);
  });
</script>

<section class="group">
  <h2 class="group-title">
    <span>插件</span>
    <button class="btn-mini" onclick={() => void loadPlugins(true)} disabled={loading || busy !== null}>
      <RefreshCw size={13} class={loading ? "spin" : ""} />
      <span>{loading ? "扫描中" : "重新扫描"}</span>
    </button>
  </h2>

  {#if error}
    <div class="notice error">
      <AlertTriangle size={14} />
      <span>{error}</span>
    </div>
  {/if}
  {#if note}
    <div class="notice ok"><Check size={14} /><span>{note}</span></div>
  {/if}

  <div class="card">
    <div class="card-head">
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
          {@const isOpen = expanded === plugin.name}
          <div class="plugin">
            <div class="plugin-head">
              <label class="check" title={plugin.enabled ? "禁用插件" : "启用插件"}>
                <input
                  type="checkbox"
                  checked={plugin.enabled}
                  onchange={(e) => void toggleEnabled(plugin, e.currentTarget.checked)}
                />
              </label>
              <button class="plugin-toggle" onclick={() => toggleExpanded(plugin)}>
                {#if isOpen}<ChevronDown size={14} />{:else}<ChevronRight size={14} />{/if}
                <span class="plugin-name" class:muted={!plugin.enabled}>{plugin.name}</span>
                <span class="version">v{plugin.version}</span>
                {#if plugin.description}
                  <span class="desc">{plugin.description}</span>
                {/if}
              </button>
            </div>

            <div class="cap-row">
              {#each capabilityGroups(plugin) as group (group.platform)}
                <span class="cap-chip" title={group.platform + ": " + group.labels.join(", ")}>
                  <span class="cap-platform">{group.platform}</span>
                  <span class="cap-value">{group.all ? "全部" : group.labels.join(" / ")}</span>
                </span>
              {/each}
            </div>

            {#if isOpen}
              <div class="plugin-detail">
                <p class="dir">{plugin.dir}</p>

                <div class="field">
                  <div class="field-head">
                    <span>默认 argv</span>
                    <div class="field-actions">
                      <button
                        class="btn-mini"
                        onclick={() => resetDefaultArgv(plugin)}
                        disabled={busy !== null}
                      >恢复清单默认</button>
                      <button
                        class="btn-mini primary"
                        onclick={() => void saveDefaultArgv(plugin)}
                        disabled={busy !== null}
                      >保存</button>
                    </div>
                  </div>
                  <textarea
                    rows="3"
                    spellcheck="false"
                    placeholder={inheritedDefault(plugin)}
                    value={argvDrafts[plugin.name] ?? ""}
                    oninput={(e) => (argvDrafts[plugin.name] = e.currentTarget.value)}
                  ></textarea>
                  <p class="hint">每行一个参数；留空则继承清单 exec。</p>
                </div>

                {#each Array.from(new Set(plugin.capabilities.map((c) => c.platform))) as platform (platform)}
                  {@const key = pluginKey(plugin.name, platform)}
                  <div class="field">
                    <div class="field-head">
                      <span>{platform} argv 覆盖</span>
                      <button
                        class="btn-mini"
                        onclick={() => void savePlatformArgv(plugin, platform)}
                        disabled={busy !== null}
                      >保存</button>
                    </div>
                    <textarea
                      rows="2"
                      spellcheck="false"
                      placeholder={platformPlaceholder(plugin)}
                      value={platformDrafts[key] ?? ""}
                      oninput={(e) => (platformDrafts[key] = e.currentTarget.value)}
                    ></textarea>
                  </div>
                {/each}

                <div class="field">
                  <div class="field-head">
                    <span>并发上限</span>
                    <button
                      class="btn-mini"
                      onclick={() => void saveConcurrency(plugin)}
                      disabled={busy !== null}
                    >保存</button>
                  </div>
                  <input
                    class="text-input"
                    type="number"
                    min="1"
                    step="1"
                    placeholder="默认"
                    value={concurrencyDrafts[plugin.name] ?? ""}
                    oninput={(e) => (concurrencyDrafts[plugin.name] = e.currentTarget.value)}
                  />
                </div>

                {#if plugin.last_stderr}
                  <div class="field">
                    <span class="field-label">最近一次 stderr</span>
                    <pre class="stderr">{plugin.last_stderr}</pre>
                  </div>
                {/if}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <div class="card">
    <div class="card-head">
      <div>
        <h3>平台路由</h3>
        <p class="hint">每个平台每个能力只选一个 provider；列表顺序就是 fallback 顺序。</p>
      </div>
    </div>

    {#if !pluginState}
      <div class="empty">插件系统不可用。</div>
    {:else if routeGroups.length === 0}
      <div class="empty">还没有可配置的路由。</div>
    {:else}
      <div class="route-groups">
        {#each routeGroups as [platform, routes] (platform)}
          <div class="route-platform">
            <h4>{platform}</h4>
            {#each routes as route (route.capability)}
              {@const key = routeKey(route)}
              {@const open = openProviderFor(route)}
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
                        onclick={() => void moveProvider(route, index, -1)}
                      ><ArrowUp size={11} /></button>
                      <button
                        class="mini-btn"
                        title="下移"
                        disabled={busy !== null || index === route.providers.length - 1}
                        onclick={() => void moveProvider(route, index, 1)}
                      ><ArrowDown size={11} /></button>
                      <button
                        class="mini-btn"
                        class:active={routeArgvOpen === routeArgvKey(route, provider.plugin)}
                        title="路由 argv 覆盖"
                        disabled={busy !== null}
                        onclick={() => toggleRouteArgv(route, provider)}
                      >argv</button>
                      <button
                        class="mini-btn danger"
                        title="移除"
                        disabled={busy !== null}
                        onclick={() => void removeProvider(route, index)}
                      ><X size={11} /></button>
                    </div>
                  {/each}
                  {#if route.providers.length === 0}
                    <span class="muted small">未配置</span>
                  {/if}
                </div>
                <div class="route-add">
                  {#if availableProviders(route).length > 0}
                    <select
                      class="select select-xs select-bordered route-select"
                      value={addSelections[key] ?? ""}
                      onchange={(e) => setAddSelection(route, e.currentTarget.value)}
                    >
                      <option value="">加 provider…</option>
                      {#each availableProviders(route) as candidate (candidate.name)}
                        <option value={candidate.name}>
                          {candidate.name}{candidate.enabled ? "" : " (off)"}
                        </option>
                      {/each}
                    </select>
                    <button
                      class="btn-mini"
                      disabled={busy !== null || !addSelections[key]}
                      onclick={() => void addProvider(route)}
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
                        onclick={() => void resetRouteArgv(route, open)}
                      >恢复继承</button>
                      <button
                        class="btn-mini primary"
                        disabled={busy !== null}
                        onclick={() => void saveRouteArgv(route, open)}
                      >保存</button>
                    </div>
                  </div>
                  <textarea
                    rows="2"
                    spellcheck="false"
                    placeholder={inheritedRouteArgv(route, open.plugin)}
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
  </div>
</section>

<style>
  .group {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .group-title {
    font-size: 14px;
    font-weight: 700;
    color: var(--color-primary);
    padding-bottom: 6px;
    border-bottom: 1px solid var(--panel-border);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .card {
    background: var(--color-base-200);
    border: 1px solid var(--panel-border);
    border-radius: 8px;
    padding: 10px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .card-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }
  .card-head h3 {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
  }
  .hint {
    margin: 3px 0 0;
    font-size: 11px;
    line-height: 1.5;
    color: var(--color-base-content) / 0.55;
  }
  .count {
    font-size: 11px;
    color: var(--color-base-content) / 0.45;
  }
  .dir-line {
    margin: 0;
    font: 10px var(--font-mono, monospace);
    color: var(--color-base-content) / 0.5;
    overflow-wrap: anywhere;
  }
  .plugin-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .plugin {
    border: 1px solid var(--panel-border);
    border-radius: 7px;
    padding: 6px 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .plugin-head {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .plugin-toggle {
    display: flex;
    align-items: center;
    gap: 5px;
    flex: 1;
    min-width: 0;
    padding: 0;
    border: 0;
    background: none;
    color: var(--color-base-content);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .plugin-name {
    font-size: 13px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .version {
    flex: none;
    font-size: 10px;
    color: var(--color-base-content) / 0.45;
  }
  .cap-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    min-width: 0;
  }
  .cap-chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 4px;
    color: var(--color-base-content) / 0.7;
    background: var(--color-base-100);
    border: 1px solid var(--panel-border);
  }
  .cap-platform {
    color: var(--color-primary);
    font-weight: 600;
  }
  .cap-value {
    color: var(--color-base-content) / 0.55;
  }
  .desc {
    flex: 1;
    min-width: 0;
    font-size: 10.5px;
    color: var(--color-base-content) / 0.5;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .check {
    display: flex;
    align-items: center;
    flex: none;
    cursor: pointer;
  }
  .check input {
    width: 14px;
    height: 14px;
    accent-color: var(--color-primary);
  }
  .plugin-detail {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 4px;
    padding-top: 8px;
    border-top: 1px solid var(--panel-border);
  }
  .dir {
    margin: 0;
    font-size: 10px;
    color: var(--color-base-content) / 0.45;
    font-family: var(--font-mono, monospace);
    overflow-wrap: anywhere;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .field-label {
    font-size: 11px;
    color: var(--color-base-content) / 0.6;
  }
  .field-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    font-size: 11px;
    color: var(--color-base-content) / 0.7;
  }
  .field-actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  textarea,
  .text-input {
    width: 100%;
    box-sizing: border-box;
    font: 11px var(--font-mono, monospace);
    line-height: 1.4;
    color: var(--color-base-content);
    background: var(--color-base-100);
    border: 1px solid var(--panel-border);
    border-radius: 5px;
    padding: 5px 7px;
    resize: vertical;
  }
  textarea:focus,
  .text-input:focus {
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
    color: var(--color-base-content) / 0.8;
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
    border-color: var(--color-primary) / 0.45;
    color: var(--color-primary);
  }
  .muted {
    color: var(--color-base-content) / 0.4;
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
    color: var(--color-base-content) / 0.5;
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
    background: var(--color-error) / 0.1;
    border: 1px solid var(--color-error) / 0.25;
  }
  .notice.ok {
    color: var(--color-success);
    background: var(--color-success) / 0.1;
    border: 1px solid var(--color-success) / 0.25;
  }
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
    color: var(--color-base-content) / 0.7;
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
    background: var(--color-primary) / 0.15;
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
    color: var(--color-base-content) / 0.55;
    font-size: 9px;
    font-family: inherit;
    cursor: pointer;
  }
  .mini-btn:hover:not(:disabled) {
    background: var(--color-primary) / 0.15;
    color: var(--color-primary);
  }
  .mini-btn.active {
    color: var(--color-primary);
    background: var(--color-primary) / 0.15;
  }
  .mini-btn.danger:hover:not(:disabled) {
    color: var(--color-error);
    background: var(--color-error) / 0.12;
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
  .stderr {
    margin: 0;
    max-height: 140px;
    overflow: auto;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    font: 10px var(--font-mono, monospace);
    color: var(--color-base-content) / 0.65;
    background: var(--color-base-100);
    border: 1px solid var(--panel-border);
    border-radius: 4px;
    padding: 5px 6px;
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
