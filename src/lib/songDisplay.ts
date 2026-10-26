import type { Song, Tag } from "$lib/types";

/* ── Time ─────────────────────────────────────────────── */

/** Hours are only shown when needed: 3:07 / 1:05:23. */
function clock(total: number): string {
  const secs = Math.floor(total);
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  return h > 0
    ? h + ":" + String(m).padStart(2, "0") + ":" + String(s).padStart(2, "0")
    : m + ":" + String(s).padStart(2, "0");
}

/** Duration for list cells: empty when unknown. */
export function formatDuration(seconds?: number): string {
  if (!seconds || seconds <= 0) return "";
  return clock(seconds);
}

/** Playback clock: "0:00" when unknown. */
export function formatTime(seconds?: number): string {
  if (!seconds || seconds <= 0) return "0:00";
  return clock(seconds);
}

/* ── Display overrides ────────────────────────────────── */

/** Display overrides win over the raw file tags. */
export function displayTitle(song: Song): string {
  return song.metadata.display_title ?? song.title;
}

export function displayArtist(song: Song): string {
  return song.metadata.artist_override ?? song.artist;
}

export function displayAlbum(song: Song): string {
  return song.metadata.album_override ?? song.album;
}

/** Artist/album for a cell: "Unknown" renders as empty. */
export function displayText(song: Song, field: "artist" | "album"): string {
  const value = field === "artist" ? displayArtist(song) : displayAlbum(song);
  return value && value !== "Unknown" ? value : "";
}

/* ── Rows ─────────────────────────────────────────────── */

/** "local" badge for file-backed songs, otherwise the first source platform. */
export function platformTag(
  song: Song,
): { label: string; kind: "local" | "online" } | null {
  if (song.paths.length > 0 || song.local) return { label: "local", kind: "local" };
  const src = song.sources[0];
  if (src) return { label: src.platform, kind: "online" };
  return null;
}

/** Searchable text for the tag column and tag-based sorting. */
export function tagText(song: Song): string {
  return song.tags.map((t) => t.namespace + ":" + t.value).join(", ");
}

/** Stable per-tag hue so a tag keeps its colour across views. */
export function tagHue(tag: Tag): number {
  let hash = 0;
  const text = tag.namespace + ":" + tag.value;
  for (let i = 0; i < text.length; i++) {
    hash = (hash * 31 + text.charCodeAt(i)) >>> 0;
  }
  return hash % 360;
}
