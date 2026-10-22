import { invoke } from "$lib/transport";

/** Load an image, retrying through the backend for hotlink-protected CDNs. */
export async function loadCoverWithFallback(
  img: HTMLImageElement,
): Promise<void> {
  if (img.dataset.proxied) {
    img.style.display = "none";
    return;
  }
  img.dataset.proxied = "1";
  const url = img.currentSrc || img.src;
  const referer = url.includes("hdslb.com") ? "https://www.bilibili.com/" : null;
  try {
    img.src = await invoke<string>("fetch_remote_image", { url, referer });
  } catch {
    img.style.display = "none";
  }
}
