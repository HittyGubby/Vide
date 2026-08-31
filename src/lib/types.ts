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

/* ── Search Preset ───────────────────── */
export interface SearchPreset {
  id: string;
  name: string;
  query: string;
}

/* ── Online Search Result ───────────────────── */
export interface SearchResultItem {
  platform: string;
  id: string;
  title: string;
  artist?: string;
  album?: string;
  duration?: number;
}

/* ── Shortcuts ─────────────────────────── */
export type ShortcutAction =
  | "toggle_window"
  | "toggle_lyrics"
  | "quit"
  | "toggle_play"
  | "prev_track"
  | "next_track"
  | "volume_down"
  | "volume_up"
  | "seek_back"
  | "seek_forward";

/** An empty string means the action has no binding. */
export interface ShortcutBinding {
  in_app: string;
  global: string;
}

export type ShortcutSettings = Record<ShortcutAction, ShortcutBinding>;

/* ── Desktop lyrics ────────────────────── */
export type LyricsPosition = "top" | "center" | "bottom";

/* ── Cache ─────────────────────────────── */
export interface CacheStat {
  kind: string;
  label: string;
  count: number;
  /** Bytes on disk; absent for caches stored as text in SQLite. */
  size?: number | null;
}

/* ── App Settings ──────────────────────── */
export interface AppSettings {
  close_to_tray: boolean;
  auto_play_on_start: boolean;
  pause_on_device_change: boolean;
  window_scale: number;
  font_family: string;
  desktop_lyrics_enabled: boolean;
  desktop_lyrics_locked: boolean;
  desktop_lyrics_always_on_top: boolean;
  desktop_lyrics_position: LyricsPosition;
  theme_primary: string;
  desktop_lyrics_css: string;
  detail_lyrics_css: string;
  shortcuts: ShortcutSettings;
  seek_step_secs: number;
  volume_step: number;
}
