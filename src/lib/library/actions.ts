import { invoke, downloadTarget as remoteDownloadTarget, isRemoteMode } from "$lib/transport";
import { isTauri } from "$lib/isTauri";
import { open, save } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { downloadManyWeb } from "$lib/download";
import { displayArtist, displayTitle } from "$lib/songDisplay";
import type { Song } from "$lib/types";

/**
 * Environment-aware row actions shared by the desktop table, the mobile list
 * and the context menu.
 *
 * They return a user-facing message (or an empty string / null for "nothing to
 * say") instead of touching the UI: the caller owns the toast.
 */

/** The online page for a song, when the platform maps to one. */
export function buildDetailUrl(song: Song): string | null {
  const src = song.sources[0];
  if (!src) return null;
  const id = src.id;
  switch (src.platform) {
    case "youtube":
      return "https://www.youtube.com/watch?v=" + id;
    case "soundcloud":
      return id;
    case "bilibili":
    case "bili":
      return "https://www.bilibili.com/video/" + id;
    case "netease":
    case "wy":
      return "https://music.163.com/#/song?id=" + id;
    case "kugou":
    case "kg":
      return "https://www.kugou.com/song/#hash=" + id;
    case "kuwo":
    case "kw":
      return "http://www.kuwo.cn/play_detail/" + id;
    case "tencent":
    case "qq":
    case "tx":
      return "https://y.qq.com/n/ryqq/songDetail/" + id;
    default:
      return null;
  }
}

/**
 * "Details" for a row:
 * - local file  → reveal it in the file manager;
 * - online song → open its page in the browser.
 * Returns "" on success, otherwise the message to show.
 */
export async function revealOrOpenDetails(song: Song): Promise<string> {
  if (song.paths.length > 0) {
    let lastError = "";
    for (const path of song.paths) {
      try {
        // The backend opens the file manager and selects the file, falling
        // back to the containing folder on desktops without FileManager1.
        await invoke("reveal_local_file", { path });
        return "";
      } catch (e) {
        // Try the next duplicate location; report only if none worked.
        lastError = String(e);
        console.error("reveal item error:", e);
      }
    }
    return "无法在文件管理器中定位该文件" + (lastError ? "：" + lastError : "");
  }

  const url = buildDetailUrl(song);
  if (!url) return "该歌曲没有可打开的来源链接";
  try {
    if (isTauri) {
      await openUrl(url);
    } else {
      window.open(url, "_blank", "noopener,noreferrer");
    }
    return "";
  } catch (e) {
    console.error("open url error:", e);
    return "无法打开浏览器";
  }
}

/** Delete the cached media for one song, forcing a fresh fetch. */
export async function clearSongCache(song: Song): Promise<string> {
  const removed = await invoke<number>("clear_song_cache", { id: song.id });
  return "已清空缓存" + (removed > 0 ? "（" + removed + " 条）" : "（无缓存）");
}

export function suggestDownloadName(song: Song): string {
  const base =
    [displayArtist(song), displayTitle(song)].filter(Boolean).join(" - ") || "audio";
  const existing = song.paths[0] ?? song.metadata.file_name ?? "";
  const ext = existing.includes(".") ? existing.split(".").pop()! : "mp3";
  return base.replace(/[\\/:*?"<>|]/g, "_") + "." + (ext === "media" ? "mp3" : ext);
}

/**
 * Download one song.
 *
 * Returns a message to show, or null when the user cancelled the save dialog.
 * Throws on real failures so the caller can show the error.
 */
export async function downloadSong(song: Song): Promise<string | null> {
  if (!isTauri) {
    const target = remoteDownloadTarget(song.id);
    if (!target) throw new Error("download endpoint unavailable");
    const link = document.createElement("a");
    link.href = target.url;
    link.download = "";
    link.click();
    return "已开始下载";
  }

  const destination = await save({ defaultPath: suggestDownloadName(song) });
  if (!destination) return null;

  if (isRemoteMode()) {
    const target = remoteDownloadTarget(song.id);
    if (!target) throw new Error("download endpoint unavailable");
    const path = await invoke<string>("download_remote_to_path", {
      url: target.url,
      headers: target.headers,
      path: destination,
    });
    return "已保存到 " + path;
  }

  const path = await invoke<string>("download_song_to_path", {
    id: song.id,
    path: destination,
  });
  return "已保存到 " + path;
}

/** Download every selected song into one directory (browser: N downloads). */
export async function downloadSelected(songs: Song[]): Promise<string | null> {
  if (songs.length === 0) return null;

  if (!isTauri) {
    await downloadManyWeb(songs.map((song) => ({ id: song.id, title: displayTitle(song) })));
    return null;
  }

  const directory = await open({ directory: true, multiple: false });
  if (!directory || Array.isArray(directory)) return null;

  for (const song of songs) {
    const destination = directory + "/" + suggestDownloadName(song);
    if (isRemoteMode()) {
      const target = remoteDownloadTarget(song.id);
      if (!target) continue;
      await invoke("download_remote_to_path", {
        url: target.url,
        headers: target.headers,
        path: destination,
      });
    } else {
      await invoke("download_song_to_path", { id: song.id, path: destination });
    }
  }
  return "已下载 " + songs.length + " 首";
}

/** Clear the cache of every selected song, ignoring per-song failures. */
export async function clearSelectedCache(songs: Song[]): Promise<string> {
  for (const song of songs) {
    try {
      await invoke("clear_song_cache", { id: song.id });
    } catch {
      /* remote per-song cache clearing is not available */
    }
  }
  return "已清空所选缓存";
}
