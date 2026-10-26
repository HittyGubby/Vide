import type { Song } from "$lib/types";
import { displayAlbum, displayArtist, displayTitle, tagText } from "$lib/songDisplay";

export type LibraryColumn = "num" | "name" | "tag" | "artist" | "album" | "time";

export interface QuerySort {
  field: string;
  asc: boolean;
}

/** Column headers can express any sort field in the query language. */
export const SORT_FIELD_BY_COLUMN: Record<LibraryColumn, string | null> = {
  num: null,
  name: "title",
  tag: "tags",
  artist: "artist",
  album: "album",
  time: "duration",
};

/** Query-language sort fields that map back to a visible column. */
export const COLUMN_BY_SORT_FIELD: Record<string, LibraryColumn> = {
  title: "name",
  song_metadata: "name",
  tags: "tag",
  artist: "artist",
  album: "album",
  duration: "time",
};

export function sortValue(song: Song, field: string): string | number {
  switch (field) {
    case "title":
    case "song_metadata":
      return displayTitle(song);
    case "artist":
      return displayArtist(song);
    case "album":
      return displayAlbum(song);
    case "tags":
      return tagText(song);
    case "duration":
      return song.metadata.duration ?? 0;
    case "added_time":
      return song.metadata.added_time ?? "";
    case "file_name":
      return song.metadata.file_name ?? "";
    case "notes":
      return song.metadata.notes ?? "";
    case "lyrics":
      return song.metadata.lyrics ?? "";
    case "source":
      return song.sources.map((source) => source.platform).join(" ");
    case "path":
      return song.paths.map((path) => path.toString()).join(" ");
    default:
      return "";
  }
}

export function compareByField(a: Song, b: Song, field: string): number {
  const left = sortValue(a, field);
  const right = sortValue(b, field);
  if (typeof left === "number" && typeof right === "number") {
    return left - right;
  }
  return String(left).localeCompare(String(right));
}

export function compareByColumn(a: Song, b: Song, field: LibraryColumn): number {
  switch (field) {
    case "name":
      return displayTitle(a).localeCompare(displayTitle(b));
    case "artist":
      return displayArtist(a).localeCompare(displayArtist(b));
    case "album":
      return displayAlbum(a).localeCompare(displayAlbum(b));
    case "tag":
      return tagText(a).localeCompare(tagText(b));
    case "time":
      return (a.metadata.duration ?? 0) - (b.metadata.duration ?? 0);
    default:
      return 0;
  }
}

/**
 * Sort a copy of the list. A SORT BY directive carried in the query string
 * wins over the column-header sort, matching the previous behaviour.
 */
export function sortSongs(
  songs: Song[],
  directive: QuerySort | null,
  field: LibraryColumn,
  asc: boolean,
): Song[] {
  const list = [...songs];
  list.sort((a, b) => {
    const cmp = directive
      ? compareByField(a, b, directive.field)
      : compareByColumn(a, b, field);
    const up = directive ? directive.asc : asc;
    return up ? cmp : -cmp;
  });
  return list;
}
