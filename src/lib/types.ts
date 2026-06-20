/* ── Song ID ─────────────────────────────────────────── */
export interface SongId {
  id: string;
}

/* ── Source ──────────────────────────────────────────── */
export interface Source {
  platform: string;
  id: string;
}

/* ── Tag ─────────────────────────────────────────────── */
export interface Tag {
  namespace: string;
  value: string;
}

/* ── Meta Tag ────────────────────────────────────────── */
export interface MetaTag {
  namespace: string;
  display_name: string;
  children: string[];
}

/* ── Song Metadata ───────────────────────────────────── */
export interface SongMetadata {
  title?: string;
  artist?: string;
  album?: string;
  source?: string;
  path?: string;
  added_time?: string;
  duration?: number;
  import_order?: number;
  import_source?: string;
  file_name?: string;
  display_title?: string;
  artist_override?: string;
  lyrics?: string;
  notes?: string;
}

/* ── Song ────────────────────────────────────────────── */
export interface Song {
  id: string;
  title: string;
  artist: string;
  album: string;
  tags: Tag[];
  sources: Source[];
  paths: string[];
  metadata: SongMetadata;
}

/* ── Playback Status ─────────────────────────────────── */
export type PlaybackState = 'Stopped' | 'Playing';

export interface PlaybackStatus {
  state: PlaybackState;
  media_id?: string;
  elapsed_secs?: number;
  duration_secs?: number;
}

/* ── Scan Result ─────────────────────────────────────── */
export interface ScanResult {
  scanned: number;
  added: number;
  updated: number;
  skipped: number;
  errors: string[];
}

/* ── Fuzzy Match ─────────────────────────────────────── */
export interface FuzzyMatch {
  song: Song;
  score: number;
}

/* ── Playlist ────────────────────────────────────────── */
export interface PlaylistEntry {
  song_id: string;
  import_order?: number;
}

export interface Playlist {
  id: string;
  name: string;
  entries: PlaylistEntry[];
  source: { type: string; file_path?: string };
}
