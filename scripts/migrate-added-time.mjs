#!/usr/bin/env bun
/**
 * Normalize legacy songs.added_time values to "YYYY/MM/DD HH:MM:SS".
 *
 * Date-only rows use the first existing file's mtime when the song has a local
 * path, and midnight of the recorded date otherwise. Only the
 * songs.added_time column is touched: reads prefer the column and
 * metadata_json is a compatibility copy.
 *
 * Usage: bun scripts/migrate-added-time.mjs [--db PATH] [--dry-run]
 */
import { Database } from "bun:sqlite";
import { existsSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const args = process.argv.slice(2);
const flag = (name) => args.includes(name);
const valueOf = (name, fallback) => {
  const index = args.indexOf(name);
  return index >= 0 && args[index + 1] ? args[index + 1] : fallback;
};

const dbPath = valueOf("--db", join(homedir(), ".local/share/vide/vide.db"));
const dryRun = flag("--dry-run");
const NEW_FORMAT = /^\d{4}\/\d{2}\/\d{2} \d{2}:\d{2}:\d{2}$/;

const db = new Database(dbPath);
db.run("PRAGMA busy_timeout = 5000");

const rows = db
  .query(
    "SELECT s.song_id AS id, s.added_time AS added, p.path AS path " +
      "FROM songs s LEFT JOIN song_paths p ON p.song_id = s.song_id " +
      "ORDER BY s.song_id",
  )
  .all();

const songs = new Map();
for (const row of rows) {
  let entry = songs.get(row.id);
  if (!entry) {
    entry = { id: row.id, added: row.added, paths: [] };
    songs.set(row.id, entry);
  }
  if (row.path) entry.paths.push(row.path);
}

const pad = (n) => String(n).padStart(2, "0");
const formatLocal = (date) =>
  date.getFullYear() +
  "/" +
  pad(date.getMonth() + 1) +
  "/" +
  pad(date.getDate()) +
  " " +
  pad(date.getHours()) +
  ":" +
  pad(date.getMinutes()) +
  ":" +
  pad(date.getSeconds());

function parseLegacy(value) {
  const match = /^(\d{4})-(\d{1,2})-(\d{1,2})(?:[ T](\d{1,2}):(\d{2})(?::(\d{2}))?)?$/.exec(
    (value ?? "").trim(),
  );
  if (!match) return null;
  return {
    year: Number(match[1]),
    month: Number(match[2]),
    day: Number(match[3]),
    hour: match[4] ? Number(match[4]) : 0,
    minute: match[5] ? Number(match[5]) : 0,
    second: match[6] ? Number(match[6]) : 0,
  };
}

const now = Date.now();
let fromMtime = 0;
let fallback = 0;
let skipped = 0;
const updates = [];

for (const song of songs.values()) {
  if (song.added && NEW_FORMAT.test(song.added)) continue;
  const legacy = parseLegacy(song.added);
  if (!legacy) {
    skipped += 1;
    continue;
  }

  let next = null;
  const path = song.paths.find((candidate) => existsSync(candidate));
  if (path) {
    const mtime = statSync(path).mtime;
    if (mtime.getTime() <= now + 86400000) {
      next = formatLocal(mtime);
      fromMtime += 1;
    }
  }
  if (!next) {
    next =
      legacy.year +
      "/" +
      pad(legacy.month) +
      "/" +
      pad(legacy.day) +
      " " +
      pad(legacy.hour) +
      ":" +
      pad(legacy.minute) +
      ":" +
      pad(legacy.second);
    fallback += 1;
  }
  updates.push({ id: song.id, before: song.added, after: next });
}

console.log("db: " + dbPath);
console.log(
  "legacy rows: " +
    updates.length +
    " (mtime: " +
    fromMtime +
    ", midnight: " +
    fallback +
    ", skipped: " +
    skipped +
    ")",
);

if (dryRun) {
  for (const update of updates.slice(0, 10)) {
    console.log("  " + update.id + ": " + update.before + " -> " + update.after);
  }
  db.close();
  process.exit(0);
}

if (updates.length === 0) {
  console.log("nothing to migrate");
  db.close();
  process.exit(0);
}

const stamp = new Date().toISOString().replace(/[:.]/g, "-");
const backup = dbPath + ".bak-" + stamp;
db.exec("VACUUM INTO '" + backup.replace(/'/g, "''") + "'");
console.log("backup: " + backup);

const update = db.prepare("UPDATE songs SET added_time = ? WHERE song_id = ?");
db.transaction(() => {
  for (const entry of updates) update.run(entry.after, entry.id);
})();
db.close();
console.log("updated " + updates.length + " rows");
