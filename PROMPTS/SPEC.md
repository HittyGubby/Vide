# Vide — Music Platform Specification

> This document is the authoritative design spec for building Vide.
> It is written for coding agents. Follow it precisely.

---

## 1. Vision

A lightweight, cross-platform music platform for power users. Unlike traditional music players, the core abstraction is a **metadata-rich media library organized through tags, filters, and dynamic views** — not playlists.

### Capabilities

- Local music playback
- Online music platform integration (via plugins)
- Video platform integration (via plugins, e.g. Bilibili, YouTube)
- Cross-device sync
- Remote media sharing between nodes
- Extensible plugin ecosystem
- Low resource consumption
- User-controlled provider routing per platform/capability

### Target Stack

| Layer | Technology |
|-------|-----------|
| Core | Rust |
| Desktop UI | Tauri + Svelte |
| Mobile | TBD (Tauri Mobile when mature, otherwise Flutter) |
| Metadata DB | SQLite (WAL mode) |
| Media cache | Filesystem (no DB for media blobs) |
| Plugin runtime | Node.js (bundled with app) |
| IPC | JSON lines over stdin/stdout |

---

## 2. Design Principles

### 2.1 Core Owns Infrastructure

Core is authoritative for:

- Storage (file paths, cache locations)
- Caching (creation, eviction, integrity)
- Streaming (network, pipes, file reads)
- Playback pipeline (decode, buffer, output)
- Network communication (HTTP server, remote nodes)
- Sync (metadata replication)
- Plugin routing (capability → provider mapping)
- Metadata database (SQLite)

Plugins **never** directly:

- Write cache files
- Manage downloads
- Manage sync
- Serve network requests
- Access other nodes
- Choose file paths

Plugins only provide **platform-specific logic** (search, metadata resolution, stream descriptor generation).

### 2.2 Plugins Are Resolvers

Plugins answer:

- How to search for a song on a platform?
- How to fetch metadata for a song?
- How to obtain a media stream (URL + headers, or pipe command)?

Plugins do **not** answer:

- How to cache?
- How to play?
- How to stream bytes?
- How to sync?
- Where to write files?

**Golden rule: Plugins decide WHAT to fetch. Core decides HOW to fetch it.**

### 2.3 User Control Over Automation

Routing decisions are **visible and configurable**. The user can choose which provider handles each capability per platform. Fallback order is user-configurable via a ranked list. Automatic fallback follows user-defined order.

No hidden automation. No opaque routing. No recommendation spam. Power users expect explicit control over every decision — never assume, never merge, never hide.

---

## 3. Data Model

### 3.1 Song Entity

Song is the fundamental immutable object.

```json
{
  "id": "song_id",
  "title": "Song Name",
  "artist": "...",
  "album": "...",
  "tags": [],
  "sources": [],
  "paths": [],
  "metadata": {}
}
```

### 3.2 Song Identity

Use **deterministic, reproducible IDs**. Never random UUIDs.

For online sources:

```
song_id = hash(platform + source_id)
```

Examples:

```
youtube:abc123   → hash("youtube:abc123")
bilibili:BVxxxx  → hash("bilibili:BVxxxx")
```

For local files:

```
song_id = hash(size + mtime + first_chunk + last_chunk)
```

Compute lazily during scanning to avoid hashing entire large libraries at startup. Full content hash can be computed later for dedup confirmation.

IDs must be reproducible across devices (critical for sync).

### 3.3 Paths (Local Files)

Local file paths are stored as an **array**, not as identity.

```json
{
  "song_id": "...",
  "paths": [
    "/Music/Russia/song.mp3",
    "/Backup/song.mp3"
  ]
}
```

**Rules:**

- Path is an attribute, never the identity
- Multiple paths allowed (duplicates in different locations)
- If path A disappears but path B exists, song remains valid
- When a file is moved, hash matching detects it and updates the path array
- If a new path appears with a matching hash, it is added to the existing song's paths

### 3.4 Sources

One song may have multiple sources across platforms.

```json
{
  "sources": [
    { "platform": "youtube", "id": "abc" },
    { "platform": "bilibili", "id": "BVxxx" }
  ]
}
```

Source replacement does not change playlists, tags, or song identity.

### 3.5 Local Files: No Caching

For local files, playback reads directly from disk. **No copy, no cache, no duplication.**

Exception: files fetched from a **remote node** are treated like online sources and may be optionally cached locally.

---

## 4. Metadata System

Metadata is independent from files. User overrides are supported.

### 4.1 Built-in Attributes (Searchable)

Every song has these indexed fields:

```json
{
  "title": "...",
  "artist": "...",
  "album": "...",
  "source": "...",
  "path": "...",
  "added_time": "...",
  "duration": "..."
}
```

Optional import attributes (present only on imported tracks):

```json
{
  "import_order": 3,
  "import_source": "my_playlist.m3u"
}
```

- `import_order` — position in the imported list (preserves original sequence)
- `import_source` — which file this track was imported from

These are not M3U-specific. Any future import format (cue sheets, text files, etc.) uses the same attributes. Users can query `import_source:"my_playlist.m3u" sort by import_order` to reconstruct any imported list.

`path` is searchable. Users can query `path contains "Russia"` to filter by filesystem location. This replaces folder-based organization without introducing hidden automation.

### 4.2 User Overrides

```json
{
  "file_name": "track01.mp3",
  "display_title": "Custom Title",
  "artist_override": "Artist"
}
```

Filesystem is source-of-truth for file **existence**, NOT for metadata display.

### 4.3 Supported Metadata

- Title, Artist, Album
- Cover art (file reference, not blob in SQLite)
- Lyrics
- Duration
- Tags
- User notes

### 4.4 Cover Storage

```text
covers/song_id.jpg
```

SQLite stores only the file path reference. Never store large image blobs inside SQLite.

---

## 5. Tag System

### 5.1 Philosophy

Replace rigid playlist hierarchies with **multidimensional tagging**. Songs can have unlimited tags.

```text
country:russia
country:ireland
genre:pop
genre:folk
mood:dark
language:arabic
```

Tags are structured with namespace:

```
namespace:value
```

### 5.2 Meta Tags

Tags themselves can be categorized for UI organization:

```text
Country
 ├ Russia
 ├ Ireland

Genre
 ├ Pop
 ├ Folk
```

Meta tags are for UI display only, not for querying.

### 5.3 No Folder-to-Tag Inheritance

Folders are **storage locations only**, not organizational units. Tags are the single source of truth for organization.

**Rejected: automatic folder→tag inheritance.** Reasons:

- Creates two competing sources of truth (folders vs tags)
- Breaks when files are deduplicated across folders
- Introduces hidden automation (anti-philosophy)
- Only allows one rule per folder (can't tag `country:russia` AND `genre:pop` from folder structure)

Users who want folder-based organization can query `path contains "Russia"` directly.

---

## 6. Search System

### 6.1 Primary Search: Structured Query Bar

The main search bar accepts **explicit query syntax** with tag autocompletion. Power users type structured queries directly:

```text
country:russia AND genre:pop AND year>2020
```

```text
(genre:pop OR genre:rock) AND NOT language:instrumental
```

```text
artist:"Miyavi" AND path contains "Japan"
```

### 6.2 Query Syntax

**Operators:** AND, OR, NOT, parentheses for grouping.

**Tag filter:** `namespace:value` — exact match on structured tags.

**Attribute filter:** `field:value` or `field>value`, `field<value` — searches built-in attributes (title, artist, album, path, added_time, duration).

**Text contains:** `path contains "Russia"` — substring match.

**Negation:** `NOT tag:value` or `!tag:value`.

### 6.2.1 Parser Implementation

Use the `pest` crate (PEG parser generator) for query parsing. Define a grammar file covering the syntax above. This is a mature, widely-used Rust library — no need to build a parser from scratch.

### 6.3 Autocompletion

As the user types, suggest:

- Tag namespaces (country, genre, mood, language...)
- Tag values within namespace (russia, ireland...)
- Built-in attributes (title, artist, album, path...)
- Recent search history

Autocomplete uses fuzzy matching for similar characters: `ë` matches `e`, `ü` matches `u`, etc. Use `unicode-normalization` crate (NFD decomposition) for accent/case-insensitive matching. Use `fuzzy-matcher` crate for typo-tolerant matching.

### 6.4 Secondary Search: Fuzzy Text Search

A **retractable** secondary search bar for free-text fuzzy matching across all fields.

- Collapsed by default (expandable via toggle or keyboard shortcut)
- Matches across title, artist, album, tags, path
- Supports fuzzy/typo-tolerant matching (similar-looking characters, partial matches)
- Results ranked by relevance score

Use case: "I don't remember the exact tag, but I know part of the title."

### 6.5 Sorting

Sorting is separate from filtering. Supported sort fields:

- Relevance
- Added date
- Play count
- Artist
- Album
- Duration
- User-defined

UI:

```text
Search: "country:russia AND genre:pop"
Sort: Recently added ▼
```

### 6.6 Search History

Recent searches are stored as history. Users can re-run any previous query with one click.

History is ordered by recency. No separate "saved query" concept — just reuse history.

---

## 7. Playlist System

### 7.1 Library View

The primary interface is a **single library view** driven by the current query. There is no concept of multiple named playlists in v1. The user types a query, sees matching songs, plays them.

Search history persists previous queries. Users revisit any past query with one click.

For explicit ordered collections, users can create static playlists (ordered reference lists). But the main workflow is query-driven.

### 7.2 Import

**M3U vs M3U8 distinction:** Do not rely on file extension. Content-sniff the file: if it contains `#EXT-X-` directives, it is an HLS manifest (M3U8) — hand it directly to mpv, do not parse. Otherwise, treat it as a simple M3U playlist catalog.

**M3U import (core feature, not a plugin):**

1. Parse M3U file (use a mature Rust M3U parsing crate)
2. For each entry:
   - Resolve relative paths to absolute (base = M3U file's directory)
   - External URLs (http/https) are imported as-is (online source entries, no local path)
   - Extract `#EXTINF` metadata (title, duration) to pre-fill song info
3. Map entries → internal song IDs by path match or metadata match
4. If no match found → create "unresolved entry" (placeholder)
5. Set `import_order` (position in list) and `import_source` (M3U filename) on each imported track
6. Imported playlists are static ordered lists (order preserved via `import_order`)

**Spotify import:** Not supported (DRM-protected, yt-dlp cannot handle it). Leave to future plugins.

---

## 8. Storage Architecture

### 8.1 SQLite (Metadata Only)

Stores:

- Song metadata, tags, attributes
- Playlists and saved queries
- User settings
- Plugin configuration and routing tables

Does **NOT** store media files or large blobs.

### 8.2 Media Cache (Filesystem)

```text
cache/
  abc123.media
  def456.media
  index.json
```

`index.json`:

```json
{
  "abc123": {
    "path": "cache/abc123.media",
    "size": 123456,
    "source": "youtube",
    "ttl": 999999
  }
}
```

**Why not SQLite for media?** Write amplification, fsync overhead, unnecessary disk churn. File-first is correct for media apps.

### 8.3 Cache Policy

**Default: keep cache indefinitely.**

Evict only when:

1. User explicitly refreshes (invalidates entry)
2. File becomes invalid (missing, hash mismatch)
3. Disk limit exceeded (e.g., 20GB cap)

Support:

- Pin media (never evict)
- Manual refresh per song
- Global cache size limit

### 8.4 Eviction Details

Per-entry cache record:

```json
{
  "id": "abc",
  "path": "...",
  "size": 123,
  "last_used": 123456,
  "source": "youtube",
  "valid": true
}
```

Eviction order:

1. Disk pressure: total cache > limit
2. Corruption: file missing or hash mismatch
3. Optional LRU fallback: least recently used + not pinned

---

## 9. Plugin System

### 9.1 Architecture

Plugins are **external processes** communicating via JSON lines over stdin/stdout. No ABI. No WASM. No embedded runtime.

```
Core (Rust)  ←→  stdin/stdout JSON  ←→  Plugin process (Node.js)
```

### 9.2 Plugin Runtime

- **Language:** Arbitrary. Plugins can be written in any language that can read stdin and write stdout.
- **No bundled runtime.** Core does not ship Node.js, Python, or any specific runtime. Power users are expected to have their preferred runtime installed.
- **Execution:** Core spawns the process specified in the plugin manifest. The manifest declares the full exec command.
- **No dependency management.** Plugin authors bundle their own dependencies. Core does not manage installs.
- **Plugin directory:** Single configurable path in settings (default: `~/.config/vide/plugins/` or platform equivalent). Users who want multiple sources can symlink.

**LX Music sideload approach rejected.** LX Music embeds JS in Electron with custom `lx.fetch` to bypass CSP/CORS. This is a browser-context hack. The subprocess model is strictly better: full system access, no sandbox limitations, no custom API wrappers, any language.

### 9.3 Plugin Manifest

Each plugin has a manifest declaring how to run it and what it provides:

```json
{
  "name": "yt-dlp",
  "version": "1.0",
  "exec": ["node", "yt-dlp-plugin.js"],
  "methods": ["resolve", "stream_info"],
  "capabilities": [
    { "type": "search", "platform": "youtube" },
    { "type": "resolve", "platform": "youtube" },
    { "type": "stream", "platform": "youtube" }
  ]
}
```

`exec` is an array: the command and arguments to spawn. Examples:

- `["node", "plugin.js"]` — Node.js plugin
- `["python3", "plugin.py"]` — Python plugin
- `["./plugin-binary"]` — compiled binary
- `["bash", "plugin.sh"]` — shell script

Core spawns whatever the manifest says. It does not care about the language.

### 9.4 Capability-Based Registration

On startup, each plugin registers capabilities. Core builds a routing table:

```text
youtube/search   → yt-dlp, youtube-native
youtube/stream   → yt-dlp
youtube/metadata → youtube-native
bilibili/search  → yt-dlp
bilibili/stream  → yt-dlp
```

Core queries its routing table — no runtime guessing.

### 9.5 Capability Types

Initial set (extensible):

- `search`
- `metadata`
- `playback` (stream resolution)
- `lyrics`
- `recommendation`

System must allow future capability types without redesign.

### 9.6 Routing & Orchestration

**Per-platform, per-capability provider selection.** NOT global plugin enable/disable.

The routing UI looks like:

```text
Platform: Bilibili
──────────────────
Search     [Native ▼]
Playback   [yt-dlp ▼]
Metadata   [Native ▼]
Lyrics     [Native ▼]

Fallback order:
1. Native
2. yt-dlp
3. Mirror Plugin

☑ Auto fallback
```

- Dropdown per capability per platform (draggable for reordering)
- "Auto fallback" toggle follows dropdown order
- Runtime status indicator shows active provider: `Playing via: yt-dlp`
- "Default provider" option for inheritance (avoids configuring 20 platforms individually)

**Routing table is persisted in SQLite.** On startup, core loads manifests from plugin directory, merges with saved routing config, and builds the runtime routing table.

### 9.7 All Capabilities Are Exclusive

**Every capability is exclusive.** One provider per platform per capability. No merging. No parallel queries. No result deduplication.

Rationale: power users want explicit control. Merging results from multiple providers introduces unpredictable behavior, duplicate entries, and inconsistent metadata. Pick one provider, get one result set.

| Capability | Behavior |
|-----------|----------|
| Search | **Exclusive** — one provider per platform |
| Metadata | **Exclusive** — one authoritative source per platform |
| Playback/Stream | **Exclusive** — one provider handles download |
| Lyrics | **Exclusive** — one source |

Fallback order is used only when the primary provider fails (error, timeout, unavailable).

### 9.9 Song Identity Decoupling

Search results identify items by **platform + source_id**, never by plugin name:

```json
{
  "platform": "youtube",
  "id": "abc123"
}
```

NOT:

```json
{
  "plugin": "yt-dlp",
  "id": "abc123"
}
```

This ensures plugin replacement doesn't break playlists or references.

---

## 10. Plugin Interface

### 10.1 Message Protocol

All communication uses **JSON lines over stdin/stdout**. No gRPC, no custom serialization.

**Request (core → plugin):**

```json
{"id": "t123", "action": "search", "params": {"q": "song name", "platform": "youtube"}}
```

**Response (plugin → core):**

```json
{"id": "t123", "result": [...]}
```

**Error:**

```json
{"id": "t123", "error": "not_supported"}
```

### 10.2 Plugin Actions

Each plugin declares which actions it supports:

- `search` — search for songs on a platform
- `resolve` — resolve metadata for a known song
- `stream` — return a stream descriptor for playback

### 10.3 Stream Descriptor

Plugins return **descriptors**, never perform downloads. Only two valid output types:

**Type A — Direct URL stream:**

```json
{
  "id": "t123",
  "type": "url_stream",
  "url": "https://cdn.example.com/media.mp3",
  "headers": {
    "Referer": "https://example.com",
    "User-Agent": "..."
  }
}
```

**Type B — Subprocess pipe (e.g., yt-dlp):**

```json
{
  "id": "t123",
  "type": "stream",
  "exec": {
    "cmd": ["yt-dlp", "-o", "-", "https://..."],
    "mode": "stdout"
  }
}
```

Core executes the descriptor: for `url_stream`, it fetches via HTTP; for `stream`, it spawns the process and reads stdout.

### 10.4 Rules

- Plugins NEVER choose file paths
- Plugins NEVER write to disk
- Plugins NEVER manage cache
- Plugins return structured data only
- Plugins use **stdout** for JSON communication, **stderr** for logs/debug output
- Core ignores stderr entirely
- Core ignores any stdout line that isn't valid JSON
- If a plugin pollutes stdout with non-JSON output, that is the plugin author's bug

### 10.5 Error Handling

- Plugin exits non-zero → treated as failure, core triggers fallback
- Plugin hangs → core enforces per-action timeout (search: 10s, stream: 30s, resolve: 5s) then kills process
- Plugin crashes mid-request → core cancels task, triggers fallback
- Fallback logic is entirely core-side: retry next provider, skip to next item, etc.

---

## 11. Process Management

### 11.1 Task Table

Core maintains a task table for every in-flight operation:

```text
task_table[t123] = {
  plugin: yt-dlp,
  process: PID,
  status: running
}
```

Supports: progress reporting, cancellation, timeout, retry.

### 11.2 Cancellation

Cancellation is **process-based**, not message-based.

When user skips a song:

1. Mark task as cancelled
2. Send SIGTERM to subprocess
3. Fallback to SIGKKill after timeout
4. Drop pending output
5. Notify UI immediately

Prevent background task accumulation.

### 11.3 Concurrency Limits

**Per-plugin concurrency limits** prevent request storms (the LX Music freeze problem):

```json
{
  "yt-dlp": { "max_concurrent": 2 }
}
```

Core enforces queueing. This alone prevents the "15 HTTP requests freeze" scenario.

---

## 12. yt-dlp Integration

### 12.1 Role

yt-dlp is treated as a provider plugin (or built-in core capability). It handles all download complexity: DASH/HLS, format selection, segment merging, signature refresh.

### 12.2 Format Support

The app should support **as many media formats as possible** (mp3, flac, ogg, opus, aac, m4a, wav, mp4, mkv, webm, etc.). For video files, the **audio track is played directly** without extraction. No FFmpeg dependency. The player simply opens the file and reads the audio stream — video tracks are ignored by the playback pipeline.

### 12.3 Preferred Mode: Subprocess Pipe

```bash
yt-dlp -o - URL
```

Media streams through stdout. Core:

1. Spawns yt-dlp process
2. Reads stdout as byte stream
3. Writes to cache file AND/OR feeds directly to player
4. No intermediate files required

### 12.4 Metadata Mode

For metadata-only resolution:

```bash
yt-dlp -j URL
```

Returns structured JSON with `url`, `http_headers`, `formats[]`, etc. Core can then fetch directly using those headers.

### 12.5 Tiered Strategy

**Tier 1 (preferred):** Plugin returns URL + headers → Core fetches directly.

**Tier 2 (fallback for hard platforms):** Plugin returns pipe command → Core spawns process and reads stdout.

**Never:** Plugin downloads file → writes to disk → Core reads. (This is the LX-style failure mode.)

---

## 13. Networking

> **Important:** All media delivery in this system uses **HTTP octet-stream** (chunked transfer of bytes), like serving a static file. This is NOT real-time streaming. Do not implement RTSP, RTMP, or any streaming protocol. HLS/DASH complexity is handled by yt-dlp/mpv, not by the core HTTP server. The core server simply opens a file (or pipe) and writes bytes to the HTTP response.

### 13.1 Remote Sharing is Core

Remote sharing is a **core feature**, not a plugin. It requires access to identity, storage, and network layers — all core-owned.

### 13.2 HTTP API

```http
GET /song/{id}          → media stream (octet-stream)
GET /meta/{id}          → JSON metadata
GET /cover/{id}         → cover image
GET /search?q=...       → search via plugin system
GET /status             → current playback state (one-shot)
GET /status/stream      → SSE stream of playback state changes
GET /now-playing        → streams current track audio (dynamic)
```

`{id}` is the song ID (deterministic hash, e.g. `a1b2c3d4e5f6...`).

### 13.3 Player Status (SSE)

Realtime playback status via Server-Sent Events.

`GET /status/stream` pushes events on state changes:

```json
{"type":"track_change","data":{"id":"...","title":"...","artist":"...","duration":300}}
{"type":"state_change","data":{"state":"playing"}}
{"type":"progress","data":{"position":45.2,"duration":300}}
{"type":"track_switching"}
```

The `track_switching` event tells clients the track is about to change, so they can prepare to reconnect or re-fetch.

`GET /status` returns the current state as a one-shot JSON response (for initial load or polling fallback).

`GET /now-playing` streams the current track's audio dynamically. When the track switches, the server sends a `track_switching` SSE event and the client reconnects to the new track.

### 13.4 Access Modes

Server returns available access modes. **No priority field** — the client decides the order.

```json
{
  "id": "a1b2c3d4e5f6...",
  "sources": [
    { "type": "url", "url": "https://cdn..." },
    { "type": "stream", "endpoint": "/song/a1b2c3d4e5f6..." }
  ]
}
```

**Server is authoritative** — decides which modes are available. Client tries modes in its preferred order (typically URL first, then stream fallback).

**URL mode:** low server load, direct CDN access.

**Stream mode:** server streams bytes from local cache. Used for local files, private content, auth-required content, cross-device sharing.

### 13.5 Remote Song Identity

Remote content appears as ordinary sources:

```json
{
  "platform": "remote",
  "node": "deviceA",
  "id": "a1b2c3d4e5f6..."
}
```

`id` is the song ID (deterministic hash). Remote nodes use this directly — no additional hashing.

Core handles transport transparently. Plugins are NOT involved in remote streaming.

---

## 14. Distributed Nodes

### 14.1 Node Roles

Every instance can act as:

- Client (consumes from others)
- Server (shares its library)
- Hybrid (both)

### 14.2 What Gets Shared

- Metadata, tags, playlists, settings (always)
- Cached media (optional)
- No plugin state is shared

### 14.3 Remote Sources

When node B requests from node A:

```http
GET /song/abc123
```

Node A opens local cache file and streams bytes. No plugin involvement.

### 14.4 Synchronization

Sync is **core-managed** replication:

- Push diffs, not full state dumps
- Replicate: metadata, tags, playlists, settings
- Media replication is optional
- Plugins never participate in sync

---

## 15. UI & Application Architecture

### 15.1 UI ↔ Core Communication

Tauri UI communicates with Rust core via **Tauri commands** (built-in IPC mechanism). JS frontend calls `invoke("command_name", {args})`, Rust backend handles it. No HTTP, no WebSocket for local UI communication.

### 15.2 Configuration

All user configuration stored in **SQLite settings table** (same database as metadata). No separate config file. Single source of truth.

### 15.3 Logging

Core uses structured logging via the `tracing` crate (Rust). Plugins use stderr for their own logs. Core ignores plugin stderr.

### 15.4 UI Principles

- **Discoverability** over hidden automation
- **Visibility** over opaque routing
- **Power-user control** over defaults

### 15.5 Provider Status

Display active provider during playback:

```text
Playing via: yt-dlp
Metadata via: Native
Lyrics via: Native
```

Clickable status badges for quick switching.

### 15.6 No Recommendations

No song recommendations. No "discover" feed. No distraction. User controls everything.

---

## 16. Library Management

### 16.1 Library Roots

User defines root directories to scan:

```json
{
  "library_roots": [
    "/Music",
    "/Downloads/Music"
  ]
}
```

Core recursively scans them for all supported media formats (audio and video — video files have audio track extracted).

### 16.2 File Scanning

During scan, for each file:

1. Compute quick hash: `hash(size + mtime + first_chunk + last_chunk)`
2. Check if hash matches existing song → update paths array
3. If no match → create new song entity
4. Full content hash computed lazily later for dedup confirmation

### 16.3 Deduplication

When same file exists in multiple locations:

- Hash match detected during scan
- Paths array updated: `["/Music/Russia/song.mp3", "/Backup/song.mp3"]`
- Song entity remains singular, storage locations are plural
- Free integrity feature: if path A disappears, path B keeps song alive

### 16.4 Playback Fallback for Local Files

```text
1. local path (first available in paths array)
2. local duplicate path (other paths)
3. remote cache (from connected nodes)
4. remote stream (from connected nodes)
```

User-configurable order.

### 16.5 Folder Handling

Folders are **storage roots and discovery sources**, not organizational units.

- User sets library root paths
- Core scans recursively for all supported formats
- Filesystem path is a searchable attribute (`path contains "Russia"`)
- No automatic folder-to-tag inheritance
- No folder-as-playlist concept
- Users organize via tags, not folders

---

## 17. Build Order (Prototype)

For a first prototype, **postpone**:

1. Cross-device sync
2. Plugin marketplace
3. Remote sharing
4. Mobile support

**Build first:**

1. Rust core with async runtime (Tokio)
2. SQLite metadata layer (WAL mode)
3. Tag system (multi-dimensional, with meta tags)
4. Local media support (library roots, scanning, audio+video format support)
5. Filesystem cache with index
6. Plugin system (process spawning, JSON IPC, manifest loading)
7. yt-dlp provider plugin (stream pipe mode)
8. Provider routing UI (per-platform capability selection, draggable fallback)
9. Search system (structured query bar with autocompletion + fuzzy text search)

This alone surpasses LX Music architecturally and validates the model in daily use.

---

## 18. Future Extensions

### Potential Plugins

- SoundCloud, YouTube, Bilibili, Spotify import
- Jellyfin, Subsonic integration
- Podcast providers
- Generic yt-dlp provider (all platforms yt-dlp supports)

### Potential Media Types

- Music, Video, Podcasts, Audiobooks

Core architecture should remain **media-agnostic** wherever possible.

---

*End of specification.*
