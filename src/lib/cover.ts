import { invoke, remoteImageUrl } from "$lib/transport";

/** Platform-specific Referer for covers that reject hotlinking. */
export function coverReferer(url: string | null | undefined): string | null {
  if (!url) return null;
  try {
    const host = new URL(url).hostname.toLowerCase();
    if (host.includes("hdslb.com") || host.includes("bilibili.com")) {
      return "https://www.bilibili.com/";
    }
    if (host.includes("126.net") || host.includes("music.163.com")) {
      return "https://music.163.com/";
    }
    if (host.includes("y.qq.com") || host.endsWith(".qq.com")) {
      return "https://y.qq.com/";
    }
    if (host.includes("kugou.com")) return "https://www.kugou.com/";
    if (host.includes("kuwo.cn")) return "https://www.kuwo.cn/";
    return null;
  } catch {
    return null;
  }
}

/**
 * Preferred src for a cover image.
 *
 * Web and remote clients use the server image proxy right away: it applies a
 * Referer, caches the bytes and avoids hotlink failures (and the browser is
 * same-origin with the proxy when it serves the app). Local Tauri has no HTTP
 * server, so the remote URL is used and `loadCoverWithFallback` swaps in a
 * data URL only when the direct request fails.
 */
export function coverSrc(url: string | null | undefined): string | null {
  if (!url) return null;
  if (url.startsWith("data:") || url.startsWith("blob:")) return url;
  // remoteImageUrl returns a proxy URL for the web client and for Tauri
  // clients connected to a remote server; local Tauri has no HTTP server and
  // falls back to the original URL (the onerror path proxies through IPC).
  return remoteImageUrl(url, coverReferer(url) ?? undefined) ?? url;
}

/** Load an image, retrying through the backend for hotlink-protected CDNs. */
export async function loadCoverWithFallback(img: HTMLImageElement): Promise<void> {
  // A reused <img> element keeps dataset entries across song changes, so the
  // "already proxied" mark must be tied to the URL that failed. Marking the
  // element once made every later cover fail silently after the first error.
  const url = img.dataset.coverFailedUrl || img.currentSrc || img.src;
  if (!url || url.startsWith("data:") || url.startsWith("blob:")) {
    img.style.display = "none";
    return;
  }
  img.style.display = "";
  if (img.dataset.coverFailedUrl === url) {
    // This exact URL already failed once; do not retry in a loop.
    img.style.display = "none";
    return;
  }
  try {
    const proxied = await invoke<string>("fetch_remote_image", {
      url,
      referer: coverReferer(url),
    });
    if (!proxied || proxied === url) throw new Error("proxy returned the original URL");
    img.dataset.coverFailedUrl = "";
    img.src = proxied;
  } catch {
    img.dataset.coverFailedUrl = url;
    img.style.display = "none";
  }
}
