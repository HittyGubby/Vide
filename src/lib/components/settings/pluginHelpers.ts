import type { PluginInfo, RouteInfo } from "$lib/types";

const CAPABILITY_LABELS: Record<string, string> = {
  search: "搜索",
  resolve: "解析",
  playlist: "歌单",
  stream: "音频",
  lyrics: "歌词",
};

const ALL_CAPABILITIES = ["search", "resolve", "stream", "playlist", "lyrics"];

export function capabilityLabel(capability: string): string {
  return CAPABILITY_LABELS[capability] ?? capability;
}

export function capabilityGroups(plugin: PluginInfo): {
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
      const all = ALL_CAPABILITIES.every((capability) => capabilities.includes(capability));
      return {
        platform,
        all,
        labels: capabilities
          .slice()
          .sort((a, b) => ALL_CAPABILITIES.indexOf(a) - ALL_CAPABILITIES.indexOf(b))
          .map(capabilityLabel),
      };
    });
}

export function pluginKey(name: string, platform: string): string {
  return name + "::" + platform;
}

export function routeKey(route: RouteInfo): string {
  return route.platform + "/" + route.capability;
}

export function routeArgvKey(route: RouteInfo, plugin: string): string {
  return routeKey(route) + "::" + plugin;
}

export function parseArgs(text: string): string[] {
  return text
    .split("\n")
    .map((line) => (line.endsWith("\r") ? line.slice(0, -1) : line))
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
}

export function platformPlaceholder(plugin: PluginInfo): string {
  return (plugin.argv.length ? plugin.argv : plugin.default_argv).join("\n");
}

export function inheritedDefault(plugin: PluginInfo): string {
  return plugin.default_argv.join("\n");
}

export function findPlugin(plugins: PluginInfo[] | undefined, name: string): PluginInfo | undefined {
  return plugins?.find((plugin) => plugin.name === name);
}

export function availableProviders(route: RouteInfo, plugins: PluginInfo[] | undefined): PluginInfo[] {
  const used = new Set(route.providers.map((provider) => provider.plugin));
  return (plugins ?? []).filter(
    (plugin) =>
      !used.has(plugin.name) &&
      plugin.capabilities.some(
        (capability) =>
          capability.platform === route.platform && capability.capability === route.capability,
      ),
  );
}

/** The provider whose route-argv editor is open for this route, if any. */
export function openProviderFor(route: RouteInfo, openKey: string | null) {
  if (!openKey) return null;
  const prefix = routeKey(route) + "::";
  if (!openKey.startsWith(prefix)) return null;
  const plugin = openKey.slice(prefix.length);
  return route.providers.find((provider) => provider.plugin === plugin) ?? null;
}

export function inheritedRouteArgv(
  route: RouteInfo,
  pluginName: string,
  plugins: PluginInfo[] | undefined,
): string {
  const plugin = findPlugin(plugins, pluginName);
  if (!plugin) return "";
  const platformArgv = plugin.platform_argv[route.platform];
  if (platformArgv && platformArgv.length > 0) return platformArgv.join("\n");
  return (plugin.argv.length ? plugin.argv : plugin.default_argv).join("\n");
}
