import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type { AppSettings, PluginState } from "$lib/types";
import { isTauri } from "$lib/isTauri";

const REMOTE_KEY = "vide_remote_session";
const TARGET_KEY = "vide_playback_target";

export type PlaybackTarget = "server" | "client";

export function getPlaybackTarget(): PlaybackTarget {
  if (typeof localStorage === "undefined") return remote ? "client" : isTauri ? "server" : "client";
  const stored = localStorage.getItem(TARGET_KEY);
  if (stored === "client" || stored === "server") return stored;
  return remote ? "client" : isTauri ? "server" : "client";
}

export function setPlaybackTarget(target: PlaybackTarget): void {
  if (typeof localStorage !== "undefined") localStorage.setItem(TARGET_KEY, target);
}

export function remoteMediaUrl(id: string): { url: string; headers: Record<string, string> } | null {
  if (remote) {
    return {
      url: remote.baseUrl + "/api/v1/media/" + encodeURIComponent(id),
      headers: remote.token ? { Authorization: "Bearer " + remote.token } : {},
    };
  }
  if (!isTauri) {
    return { url: "/api/v1/media/" + encodeURIComponent(id), headers: {} };
  }
  return null;
}

export function subscribePlayerStatus(
  handler: (status: {
    state: string;
    media_id?: string;
    elapsed_secs?: number;
    duration_secs?: number;
  }) => void,
): (() => void) | null {
  if (isTauri && !remote) return null;
  if (typeof EventSource === "undefined") return null;
  const source = new EventSource((remote?.baseUrl ?? "") + "/api/v1/player/stream");
  const listener = (event: MessageEvent) => {
    try {
      handler(JSON.parse(event.data));
    } catch {
      /* ignore malformed event */
    }
  };
  source.addEventListener("player_status", listener);
  return () => source.close();
}

/**
 * Proxy URL for a remote cover, when it is known synchronously.
 *
 * The web client (same origin) and the remote client both go through the
 * server's `/api/v1/image` proxy, which applies the referer and caches the
 * bytes. Local Tauri has no HTTP server, so it returns null and the caller
 * falls back to the `fetch_remote_image` command.
 */
export function remoteImageUrl(url: string, referer?: string): string | null {
  if (!url) return null;
  if (isTauri && !remote) return null;
  const base = remote?.baseUrl ?? "";
  return (
    base +
    "/api/v1/image?url=" +
    encodeURIComponent(url) +
    (referer ? "&referer=" + encodeURIComponent(referer) : "")
  );
}

export function downloadTarget(id: string): { url: string; headers: Record<string, string> } | null {
  if (remote) {
    return {
      url: remote.baseUrl + "/api/v1/download/" + encodeURIComponent(id),
      headers: remote.token ? { Authorization: "Bearer " + remote.token } : {},
    };
  }
  if (!isTauri) return { url: "/api/v1/download/" + encodeURIComponent(id), headers: {} };
  return null;
}

export function invokeLocal<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  return tauriInvoke<T>(command, args);
}

export interface RemoteSession {
  baseUrl: string;
  token: string;
  csrf: string;
}

export function isRemoteGuest(): boolean {
  return remote !== null && !remote.token;
}

export async function restoreRemoteSession(): Promise<void> {
  if (!remote) return;
  try {
    const response = await fetch(remote.baseUrl + "/api/v1/auth/me", {
      headers: remote.token ? { Authorization: "Bearer " + remote.token } : {},
    });
    if (!response.ok) return;
    const data = (await response.json()) as { authenticated?: boolean; csrf?: string };
    if (!data.authenticated && remote.token) {
      remote = { baseUrl: remote.baseUrl, token: "", csrf: "" };
      if (typeof localStorage !== "undefined") {
        localStorage.setItem(REMOTE_KEY, JSON.stringify(remote));
      }
    } else if (data.csrf) {
      remote = { ...remote, csrf: data.csrf };
      if (typeof localStorage !== "undefined") {
        localStorage.setItem(REMOTE_KEY, JSON.stringify(remote));
      }
    }
  } catch {
    /* offline: keep the stored session */
  }
}

function loadRemote(): RemoteSession | null {
  if (typeof localStorage === "undefined") return null;
  try {
    const raw = localStorage.getItem(REMOTE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as RemoteSession;
    if (!parsed.baseUrl) return null;
    return {
      baseUrl: parsed.baseUrl,
      token: parsed.token ?? "",
      csrf: parsed.csrf ?? "",
    };
  } catch {
    return null;
  }
}

let remote: RemoteSession | null = loadRemote();
let browserCsrf = "";

export async function browserLogin(password: string): Promise<void> {
  const response = await fetch("/api/v1/auth/login", {
    method: "POST",
    credentials: "same-origin",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ password }),
  });
  if (!response.ok) throw new Error((await response.text()) || "login failed");
  const data = (await response.json()) as { csrf?: string };
  browserCsrf = data.csrf ?? "";
}

export async function browserAuthStatus(): Promise<boolean> {
  try {
    const response = await fetch("/api/v1/auth/me", { credentials: "same-origin" });
    if (!response.ok) return false;
    const data = (await response.json()) as { authenticated?: boolean; csrf?: string };
    browserCsrf = data.csrf ?? "";
    return Boolean(data.authenticated);
  } catch {
    return false;
  }
}

export async function browserLogout(): Promise<void> {
  await fetch("/api/v1/auth/logout", {
    method: "POST",
    credentials: "same-origin",
  }).catch(() => undefined);
  browserCsrf = "";
}

export function remoteSession(): RemoteSession | null {
  return remote;
}

export function isRemoteMode(): boolean {
  return remote !== null;
}

export function disconnectRemote(): void {
  remote = null;
  if (typeof localStorage !== "undefined") localStorage.removeItem(REMOTE_KEY);
}

function normalizeBaseUrl(value: string): string {
  return value.trim().replace(/\/+$/, "");
}

export async function connectRemote(baseUrl: string, password = ""): Promise<RemoteSession> {
  const base = normalizeBaseUrl(baseUrl);
  if (!base) throw new Error("server URL is required");
  if (!password.trim()) {
    const status = await fetch(base + "/api/v1/status");
    if (!status.ok) throw new Error("server is not reachable");
    remote = { baseUrl: base, token: "", csrf: "" };
  } else {
    const response = await fetch(base + "/api/v1/auth/login", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ password }),
    });
    if (!response.ok) throw new Error((await response.text()) || "login failed");
    const data = (await response.json()) as { token?: string; csrf?: string };
    if (!data.token || !data.csrf) throw new Error("server did not return a session token");
    remote = { baseUrl: base, token: data.token, csrf: data.csrf };
  }
  if (typeof localStorage !== "undefined") {
    localStorage.setItem(REMOTE_KEY, JSON.stringify(remote));
    // A freshly connected remote session starts on the client player.
    localStorage.setItem(TARGET_KEY, "client");
  }
  return remote;
}

function localKey(key: string): string {
  return "vide_client:" + key;
}

function readLocal<T>(key: string, fallback: T): T {
  const raw = localStorage.getItem(localKey(key));
  if (!raw) return fallback;
  try {
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

function browserLocal<T>(command: string, args: Record<string, unknown>): T {
  switch (command) {
    case "get_app_settings": {
      const raw = localStorage.getItem("vide_client_settings");
      if (!raw) throw new Error("no browser client settings yet");
      return JSON.parse(raw) as AppSettings as T;
    }
    case "set_app_settings":
      localStorage.setItem("vide_client_settings", JSON.stringify(args.settings));
      return undefined as T;
    case "get_client_state": {
      const raw = localStorage.getItem(localKey(String(args.key)));
      return (raw ? JSON.parse(raw) : null) as T;
    }
    case "set_client_state":
      localStorage.setItem(localKey(String(args.key)), JSON.stringify(args.value));
      return undefined as T;
    case "get_last_played": {
      const raw = localStorage.getItem("vide_client:last_played");
      return (raw ? JSON.parse(raw) : null) as T;
    }
    case "save_last_played":
      localStorage.setItem(localKey("last_played"), JSON.stringify(args.value));
      return undefined as T;
    case "set_global_shortcuts":
      return [] as T;
    case "get_online_search_history":
      return readLocal<string[]>("online_search_history", []) as T;
    case "set_online_search_history":
      localStorage.setItem(localKey("online_search_history"), JSON.stringify(args.history ?? []));
      return undefined as T;
    case "add_online_search_history": {
      const history = readLocal<string[]>("online_search_history", []);
      const query = String(args.query ?? "");
      const next = [query, ...history.filter((item) => item !== query)].slice(0, 30);
      localStorage.setItem(localKey("online_search_history"), JSON.stringify(next));
      return undefined as T;
    }
    case "remove_online_search_history": {
      const history = readLocal<string[]>("online_search_history", []);
      localStorage.setItem(localKey("online_search_history"), JSON.stringify(history.filter((item) => item !== args.query)));
      return undefined as T;
    }
    case "clear_online_search_history":
      localStorage.removeItem(localKey("online_search_history"));
      return undefined as T;
    case "get_query_history":
      return readLocal<string[]>("query_history", []) as T;
    case "record_library_query": {
      const history = readLocal<string[]>("query_history", []);
      const query = String(args.query ?? "");
      localStorage.setItem(localKey("query_history"), JSON.stringify([query, ...history.filter((item) => item !== query)].slice(0, 30)));
      return undefined as T;
    }
    case "remove_query_history": {
      const history = readLocal<string[]>("query_history", []);
      localStorage.setItem(localKey("query_history"), JSON.stringify(history.filter((item) => item !== args.query)));
      return undefined as T;
    }
    case "clear_query_history":
      localStorage.removeItem(localKey("query_history"));
      return undefined as T;
    case "list_search_presets":
      return readLocal<unknown[]>("search_presets", []) as T;
    case "save_search_preset": {
      const presets = readLocal<unknown[]>("search_presets", []);
      presets.push(args.preset);
      localStorage.setItem(localKey("search_presets"), JSON.stringify(presets));
      return undefined as T;
    }
    case "delete_search_preset": {
      const presets = readLocal<Array<{ id?: string }>>("search_presets", []);
      localStorage.setItem(localKey("search_presets"), JSON.stringify(presets.filter((item) => item.id !== args.id)));
      return undefined as T;
    }
    case "set_last_library_query":
    case "get_library_query_state":
      return undefined as T;
    default:
      throw new Error("browser mode does not support command: " + command);
  }
}

function localInvoke<T>(command: string, args: Record<string, unknown>): Promise<T> {
  if (isTauri) return tauriInvoke<T>(command, args);
  try {
    return Promise.resolve(browserLocal<T>(command, args));
  } catch (error) {
    return Promise.reject(error);
  }
}

async function remoteRequest<T>(
  method: "GET" | "POST",
  path: string,
  body?: unknown,
): Promise<T> {
  if (!remote && isTauri) throw new Error("not connected to a remote server");
  const base = remote?.baseUrl ?? "";
  const response = await fetch(base + path, {
    method,
    credentials: remote ? "omit" : "same-origin",
    headers: {
      "Content-Type": "application/json",
      ...(remote?.token ? { Authorization: "Bearer " + remote.token } : {}),
      ...(method === "POST" && (remote?.csrf || browserCsrf)
        ? { "X-CSRF-Token": remote?.csrf || browserCsrf }
        : {}),
    },
    body: method === "POST" ? JSON.stringify(body ?? {}) : undefined,
  });
  if (!response.ok) {
    throw new Error((await response.text()) || response.statusText);
  }
  if (response.status === 204) return undefined as T;
  return (await response.json()) as T;
}

function arg<T>(args: Record<string, unknown>, ...names: string[]): T {
  for (const name of names) {
    if (name in args) return args[name] as T;
  }
  throw new Error("missing argument: " + names.join(" / "));
}

interface ServerSettings {
  library_roots: string[];
  exclude_roots: string[];
  scan_extensions?: string[];
}

async function remoteServerSettings(): Promise<ServerSettings> {
  return remoteRequest<ServerSettings>("GET", "/api/v1/server-settings");
}

async function saveRemoteServerSettings(settings: ServerSettings): Promise<void> {
  await remoteRequest("POST", "/api/v1/server-settings", {
    library_roots: settings.library_roots,
    exclude_roots: settings.exclude_roots,
  });
}

async function remoteInvoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  switch (command) {
    case "list_library_songs":
    case "list_songs":
      return remoteRequest<T>("GET", "/api/v1/songs");
    case "export_songs":
      return remoteRequest<T>("GET", "/api/v1/songs/export");
    case "search_songs":
      return remoteRequest<T>(
        "GET",
        "/api/v1/search?q=" + encodeURIComponent(arg<string>(args, "query")),
      );
    case "get_song":
      return remoteRequest<T>("GET", "/api/v1/songs/" + encodeURIComponent(arg<string>(args, "id")));
    case "get_song_details": {
      const song = await remoteRequest(
        "GET",
        "/api/v1/songs/" + encodeURIComponent(arg<string>(args, "id")),
      );
      return { song, files: [] } as T;
    }
    case "get_player_status":
      return remoteRequest<T>("GET", "/api/v1/player/status");
    case "get_lyrics":
      return remoteRequest<T>("GET", "/api/v1/lyrics/" + encodeURIComponent(arg<string>(args, "songId")));
    case "get_song_tags":
      return remoteRequest<T>(
        "GET",
        "/api/v1/songs/" + encodeURIComponent(arg<string>(args, "songId")) + "/tags",
      );
    case "get_tag_history":
      return remoteRequest<T>("GET", "/api/v1/tag-history");
    case "remove_tag_history":
      return remoteRequest<T>("POST", "/api/v1/tag-history/remove", {
        namespace: arg<string>(args, "namespace"),
        value: arg<string>(args, "value"),
      });
    case "get_namespaces":
      return remoteRequest<T>("GET", "/api/v1/namespaces");
    case "get_meta_tags":
      return remoteRequest<T>("GET", "/api/v1/meta-tags");
    case "tag_values":
      return remoteRequest<T>(
        "GET",
        "/api/v1/tag-values?namespace=" +
          encodeURIComponent(arg<string>(args, "namespace")) +
          "&prefix=" +
          encodeURIComponent((args.prefix as string) ?? "") +
          "&limit=" +
          ((args.limit as number) ?? 20),
      );
    case "add_tag":
      return remoteRequest<T>("POST", "/api/v1/tags", {
        song_id: arg<string>(args, "songId"),
        namespace: args.namespace,
        value: args.value,
      });
    case "remove_tag":
      return remoteRequest<T>("POST", "/api/v1/tags/remove", {
        song_id: arg<string>(args, "songId"),
        namespace: args.namespace,
        value: args.value,
      });
    case "update_song_metadata":
      return remoteRequest<T>(
        "POST",
        "/api/v1/songs/" + encodeURIComponent(arg<string>(args, "id")) + "/metadata",
        {
          title: args.title,
          artist: args.artist,
          album: args.album,
          notes: args.notes,
        },
      );
    case "play_song": {
      const id = arg<string>(args, "id");
      return remoteRequest<T>("POST", "/api/v1/player/play", {
        id,
        position_secs: args.positionSecs ?? null,
      });
    }
    case "set_play_queue": {
      const ids = (args.ids as string[]) ?? [];
      const index = (args.startIndex as number) ?? -1;
      return remoteRequest<T>("POST", "/api/v1/player/queue", {
        ids,
        start_index: index,
      });
    }
    case "stop_playback":
      return remoteRequest<T>("POST", "/api/v1/player/stop", {});
    case "toggle_pause":
      return remoteRequest<T>("POST", "/api/v1/player/toggle", {});
    case "play_next":
      return remoteRequest<T>("POST", "/api/v1/player/next", {});
    case "play_prev":
      return remoteRequest<T>("POST", "/api/v1/player/prev", {});
    case "seek":
      return remoteRequest<T>("POST", "/api/v1/player/seek", {
        position_secs: arg<number>(args, "positionSecs", "position_secs"),
      });
    case "set_volume":
      return remoteRequest<T>("POST", "/api/v1/player/volume", { volume: args.volume });
    case "set_mute":
      return remoteRequest<T>("POST", "/api/v1/player/mute", { muted: args.muted });
    case "set_speed":
      return remoteRequest<T>("POST", "/api/v1/player/speed", { speed: args.speed });
    case "set_play_mode":
      return remoteRequest<T>("POST", "/api/v1/player/mode", {
        mode: args.mode,
      });
    case "get_app_settings":
    case "set_app_settings":
    case "get_client_state":
    case "set_client_state":
    case "get_last_played":
    case "save_last_played":
    case "set_global_shortcuts":
    case "get_online_search_history":
    case "set_online_search_history":
    case "add_online_search_history":
    case "remove_online_search_history":
    case "clear_online_search_history":
    case "list_search_presets":
    case "save_search_preset":
    case "delete_search_preset":
    case "get_query_history":
    case "record_library_query":
    case "remove_query_history":
    case "clear_query_history":
    case "set_last_library_query":
    case "get_library_query_state":
      return localInvoke<T>(command, args);
    case "get_plugin_state":
      return remoteRequest<T>("GET", "/api/v1/plugins/state");
    case "reload_plugins":
      return remoteRequest<T>("POST", "/api/v1/plugins/reload", {});
    case "set_plugin_enabled":
      return remoteRequest<T>("POST", "/api/v1/plugins/enabled", {
        name: args.name,
        enabled: args.enabled,
      });
    case "set_plugin_argv":
      return remoteRequest<T>("POST", "/api/v1/plugins/argv", {
        name: args.name,
        argv: args.argv,
      });
    case "set_plugin_platform_argv":
      return remoteRequest<T>("POST", "/api/v1/plugins/platform-argv", {
        name: args.name,
        platform: args.platform,
        argv: args.argv,
      });
    case "set_plugin_route_argv":
      return remoteRequest<T>("POST", "/api/v1/plugins/route-argv", {
        name: args.name,
        platform: args.platform,
        capability: args.capability,
        argv: args.argv,
      });
    case "set_plugin_concurrency":
      return remoteRequest<T>("POST", "/api/v1/plugins/concurrency", {
        name: args.name,
        concurrency: args.concurrency ?? null,
      });
    case "set_plugin_route":
      return remoteRequest<T>("POST", "/api/v1/plugins/route", {
        platform: args.platform,
        capability: args.capability,
        providers: args.providers,
      });

    case "get_library_roots": {
      const settings = await remoteServerSettings();
      return settings.library_roots as T;
    }
    case "get_exclude_roots": {
      const settings = await remoteServerSettings();
      return settings.exclude_roots as T;
    }
    case "add_library_root": {
      const settings = await remoteServerSettings();
      const path = String(args.path ?? "");
      if (!settings.library_roots.includes(path)) settings.library_roots.push(path);
      await saveRemoteServerSettings(settings);
      return undefined as T;
    }
    case "remove_library_root": {
      const settings = await remoteServerSettings();
      const path = String(args.path ?? "");
      settings.library_roots = settings.library_roots.filter((item) => item !== path);
      await saveRemoteServerSettings(settings);
      return undefined as T;
    }
    case "add_exclude_root": {
      const settings = await remoteServerSettings();
      const path = String(args.path ?? "");
      if (!settings.exclude_roots.includes(path)) settings.exclude_roots.push(path);
      await saveRemoteServerSettings(settings);
      return undefined as T;
    }
    case "remove_exclude_root": {
      const settings = await remoteServerSettings();
      const path = String(args.path ?? "");
      settings.exclude_roots = settings.exclude_roots.filter((item) => item !== path);
      await saveRemoteServerSettings(settings);
      return undefined as T;
    }
    case "import_songs":
      return remoteRequest<T>("POST", "/api/v1/import/songs", args.songs);
    case "play_online_results":
      return remoteRequest<T>("POST", "/api/v1/player/play-online", {
        results: args.results,
        index: args.index ?? 0,
      });
    case "search_online":
      return remoteRequest<T>("POST", "/api/v1/search/online", {
        query: args.query,
        platform: args.platform,
        limit: args.limit ?? 20,
      });
    case "resolve_song":
      return remoteRequest<T>("POST", "/api/v1/resolve", {
        platform: args.platform,
        source_id: args.sourceId ?? args.source_id,
      });
    case "scan_library":
      return remoteRequest<T>("POST", "/api/v1/library/scan", {});
    case "get_cache_stats":
      return remoteRequest<T>("GET", "/api/v1/cache/stats");
    case "clear_cache_kind":
      return remoteRequest<T>("POST", "/api/v1/cache/clear", {
        kind: args.kind,
      });
    case "clear_song_cache":
      return remoteRequest<T>(
        "POST",
        "/api/v1/cache/song/" + encodeURIComponent(arg<string>(args, "id")),
        {},
      );
    case "fetch_remote_image": {
      const base = remote?.baseUrl ?? "";
      const url = String(args.url ?? "");
      const referer = args.referer ? "&referer=" + encodeURIComponent(String(args.referer)) : "";
      return (base + "/api/v1/image?url=" + encodeURIComponent(url) + referer) as T;
    }
    case "get_default_download_dir":
      return "Downloads" as T;
    default:
      throw new Error("remote mode does not support command: " + command);
  }
}

export async function invoke<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  if (remote || !isTauri) return remoteInvoke<T>(command, args);
  return tauriInvoke<T>(command, args);
}
