#!/usr/bin/env node
/*
 * Vide Meting adapter.
 *
 * Provides search / resolve / stream / playlist / lyrics for:
 *   netease, tencent, kugou, kuwo
 *
 * The Meting library is vendored under lib/ (MIT, see METING_LICENSE).
 * Kuwo search/stream use the older public endpoints because the Meting
 * provider currently gets "request is illegal" from the web API.
 *
 * Optional: pass --cookie "<cookie>" in the plugin argv to supply a platform
 * login cookie (mainly useful for NetEase/Tencent/Kugou stream URLs).
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import Meting from './lib/meting.js';

const HERE = dirname(fileURLToPath(import.meta.url));
const COOKIE = readCookieArg();
const KW_HEADERS = {
  'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120 Safari/537.36',
  Referer: 'http://www.kuwo.cn/',
};

function readCookieArg() {
  const index = process.argv.indexOf('--cookie');
  return index >= 0 ? process.argv[index + 1] || '' : '';
}

function meting(platform) {
  const instance = new Meting(platform);
  instance.format(true);
  if (COOKIE) instance.cookie(COOKIE);
  return instance;
}

function parseJson(raw) {
  if (typeof raw !== 'string') return raw;
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

function first(value) {
  const data = parseJson(value);
  if (Array.isArray(data)) return data[0] || null;
  return data || null;
}

function asArray(value) {
  const data = parseJson(value);
  return Array.isArray(data) ? data : [];
}

function cleanText(value) {
  return typeof value === 'string'
    ? value.replaceAll('&amp;', '&').replaceAll('&#39;', "'").replaceAll('&quot;', '"')
    : '';
}

function toTrack(platform, song) {
  if (!song) return null;
  const artist = Array.isArray(song.artist)
    ? song.artist.filter(Boolean).join(', ')
    : cleanText(song.artist || '');
  const sourceId = song.id ?? song.url_id;
  if (sourceId == null) return null;
  return {
    platform,
    source_id: String(sourceId),
    title: cleanText(song.name || song.title || 'Unknown'),
    artist: artist || undefined,
    album: song.album ? cleanText(song.album) : undefined,
    cover_url: song.cover_url || undefined,
    duration: song.duration || undefined,
  };
}

function rawMeting(platform) {
  const instance = new Meting(platform);
  instance.format(false);
  if (COOKIE) instance.cookie(COOKIE);
  return instance;
}

function neteaseTrack(song) {
  if (!song || song.id == null) return null;
  return {
    platform: 'netease',
    source_id: String(song.id),
    title: cleanText(song.name || 'Unknown'),
    artist: (song.ar || []).map((item) => item.name).filter(Boolean).join(', ') || undefined,
    album: song.al ? cleanText(song.al.name) : undefined,
    duration: song.dt ? song.dt / 1000 : undefined,
    cover_url: song.al?.picUrl || undefined,
  };
}

function tencentTrack(song) {
  if (!song || !song.songmid) return null;
  return {
    platform: 'tencent',
    source_id: String(song.songmid),
    title: cleanText(song.songname || song.name || 'Unknown'),
    artist: (song.singer || []).map((item) => item.name).filter(Boolean).join(', ') || undefined,
    album: song.albumname ? cleanText(song.albumname) : undefined,
    duration: song.interval ? Number(song.interval) : undefined,
    cover_url: song.albummid
      ? 'https://y.gtimg.cn/music/photo_new/T002R300x300M000' + song.albummid + '.jpg'
      : undefined,
  };
}

function kugouTrack(song) {
  if (!song || !song.hash) return null;
  const unionCover = song.trans_param?.union_cover;
  return {
    platform: 'kugou',
    source_id: String(song.hash),
    title: cleanText(song.songname || 'Unknown'),
    artist: cleanText(song.singername || '') || undefined,
    album: song.album_name ? cleanText(song.album_name) : undefined,
    duration: song.duration ? Number(song.duration) : undefined,
    cover_url: unionCover ? unionCover.replace('{size}', '240') : undefined,
  };
}

function streamCommand(url, headers) {
  const payload = Buffer.from(JSON.stringify({ url, headers })).toString('base64');
  return [process.execPath, join(HERE, 'stream.mjs'), payload];
}

async function fetchText(url, headers = {}) {
  const response = await fetch(url, {
    headers,
    signal: AbortSignal.timeout(15000),
  });
  const text = await response.text();
  if (!response.ok) throw new Error('HTTP ' + response.status + ' from ' + url);
  return text;
}

// ── Kuwo fallbacks ────────────────────────────────────────────────

async function kuwoSearch(query, limit) {
  const url =
    'http://search.kuwo.cn/r.s?client=kt&all=' +
    encodeURIComponent(query) +
    '&pn=0&rn=' +
    limit +
    '&uid=794762570&ver=kwplayer_ar_9.2.2.1&vipver=1&show_copyright_off=1&newver=1&ft=music&cluster=0&strategy=2012&encoding=utf8&rformat=json&vermerge=1&mobi=1&issubtitle=1';
  const body = parseJson(await fetchText(url));
  const list = body && Array.isArray(body.abslist) ? body.abslist : [];
  return list
    .map((item) => {
      if (!item || !item.MUSICRID) return null;
      const id = String(item.MUSICRID).replace('MUSIC_', '');
      return {
        platform: 'kuwo',
        source_id: id,
        title: cleanText(item.SONGNAME || 'Unknown'),
        artist: cleanText(item.ARTIST || '') || undefined,
        album: item.ALBUM ? cleanText(item.ALBUM) : undefined,
        duration: item.DURATION ? Number(item.DURATION) : undefined,
        cover_url: item.web_albumpic_short
          ? 'https://img2.kuwo.cn/star/albumcover/' + item.web_albumpic_short
          : undefined,
      };
    })
    .filter(Boolean);
}

function htmlField(text, key) {
  const marker = key + ':"';
  const start = text.indexOf(marker);
  if (start < 0) return null;
  let index = start + marker.length;
  let value = '';
  while (index < text.length) {
    const char = text[index++];
    if (char === '"') break;
    value += char;
  }
  return value;
}

function htmlNumber(text, key) {
  const marker = key + ':';
  const start = text.indexOf(marker);
  if (start < 0) return null;
  let index = start + marker.length;
  let value = '';
  while (index < text.length) {
    const char = text[index];
    if ((char >= '0' && char <= '9') || char === '.') {
      value += char;
      index++;
    } else {
      break;
    }
  }
  return value ? Number(value) : null;
}

async function kuwoSong(id) {
  const html = await fetchText('http://www.kuwo.cn/play_detail/' + encodeURIComponent(id), KW_HEADERS);
  const titleMatch = html.indexOf('<title>');
  const titleEnd = html.indexOf('</title>', titleMatch);
  const titleText = titleMatch >= 0 ? html.slice(titleMatch + 7, titleEnd) : '';
  const title = titleText.split('_')[0] || 'Unknown';
  const songInfoStart = html.indexOf('songinfo:{');
  const region = songInfoStart >= 0 ? html.slice(songInfoStart, songInfoStart + 8000) : html;
  return {
    platform: 'kuwo',
    source_id: String(id),
    title,
    artist: htmlField(region, 'artist') || undefined,
    album: htmlField(region, 'album') || undefined,
    duration: htmlNumber(region, 'duration') || undefined,
  };
}

async function kuwoPlaylist(id) {
  const url =
    'http://nplserver.kuwo.cn/pl.svc?op=getlistinfo&pid=' +
    encodeURIComponent(id) +
    '&pn=0&rn=1000&encode=utf8&keyset=pl2012&identity=kuwo&pcmp4=1&vipver=MUSIC_9.0.5.0_W1&newver=1';
  const body = parseJson(await fetchText(url));
  const list = body && Array.isArray(body.musiclist) ? body.musiclist : [];
  return list
    .map((item) => {
      if (!item) return null;
      const rawId = item.MUSICRID ?? item.id;
      if (!rawId) return null;
      const itemId = String(rawId).replace('MUSIC_', '');
      return {
        platform: 'kuwo',
        source_id: itemId,
        title: cleanText(item.SONGNAME || item.name || 'Unknown'),
        artist: cleanText(item.ARTIST || item.artist || '') || undefined,
        album: item.ALBUM || item.album ? cleanText(item.ALBUM || item.album) : undefined,
        duration: item.DURATION || item.duration ? Number(item.DURATION || item.duration) : undefined,
      };
    })
    .filter(Boolean);
}

async function kuwoStream(id) {
  const url =
    'http://antiserver.kuwo.cn/anti.s?type=convert_url&rid=MUSIC_' +
    encodeURIComponent(id) +
    '&format=mp3&response=url';
  const text = (await fetchText(url, KW_HEADERS)).trim();
  if (!text.startsWith('http')) throw new Error('Kuwo returned no stream URL');
  return text;
}

// ── URL parsing ───────────────────────────────────────────────────

function parseUrl(raw) {
  let url;
  try {
    url = new URL(raw);
  } catch {
    return null;
  }
  const host = url.hostname.toLowerCase();
  const path = url.pathname;
  const query = url.searchParams;

  if (host.endsWith('music.163.com') || host.endsWith('163.com')) {
    let id = query.get('id');
    if (!id && url.hash.includes('id=')) id = url.hash.split('id=')[1].split('&')[0];
    if (!id) {
      const parts = path.split('/').filter(Boolean);
      if (parts[0] === 'song' || parts[0] === 'playlist') id = parts[1];
    }
    if (id) return { platform: 'netease', type: path.includes('playlist') ? 'playlist' : 'song', id };
  }

  if (host.endsWith('y.qq.com') || host.endsWith('qq.com')) {
    let id = query.get('songmid') || query.get('id');
    let type = path.includes('playlist') || path.includes('taoge') ? 'playlist' : 'song';
    if (!id) {
      const parts = path.split('/').filter(Boolean);
      const marker = parts.indexOf('songDetail');
      if (marker >= 0) id = parts[marker + 1];
      const playlistMarker = parts.indexOf('playlist');
      if (playlistMarker >= 0) {
        id = parts[playlistMarker + 1];
        type = 'playlist';
      }
    }
    if (id) return { platform: 'tencent', type, id };
  }

  if (host.endsWith('kugou.com')) {
    let id = query.get('hash');
    if (!id && url.hash.includes('hash=')) id = url.hash.split('hash=')[1].split('&')[0];
    let type = 'song';
    if (!id) {
      const parts = path.split('/').filter(Boolean);
      const marker = parts.indexOf('single');
      if (marker >= 0) {
        id = parts[marker + 1].replace('.html', '');
        type = 'playlist';
      }
    }
    if (id) return { platform: 'kugou', type, id };
  }

  if (host.endsWith('kuwo.cn')) {
    const parts = path.split('/').filter(Boolean);
    if (parts[0] === 'play_detail' && parts[1]) {
      return { platform: 'kuwo', type: 'song', id: parts[1] };
    }
    if (parts[0] === 'playlist_detail' && parts[1]) {
      return { platform: 'kuwo', type: 'playlist', id: parts[1] };
    }
  }

  // bilibili/youtube are handled by the yt-dlp adapter via fallback.
  return null;
}

// ── Actions ───────────────────────────────────────────────────────

function mapSearchTracks(platform, data) {
  if (platform === 'netease') {
    return ((data?.result?.songs) || []).map(neteaseTrack).filter(Boolean);
  }
  if (platform === 'tencent') {
    return ((data?.data?.song?.list) || []).map(tencentTrack).filter(Boolean);
  }
  if (platform === 'kugou') {
    return ((data?.data?.info) || []).map(kugouTrack).filter(Boolean);
  }
  return [];
}

async function search(params) {
  const platform = params.platform;
  const query = params.query || params.q || '';
  // The caller decides how many results it wants; never cap below that.
  const limit = Math.max(Number(params.limit) || 20, 1);
  if (!query) return [];

  if (platform === 'kuwo') {
    return kuwoSearch(query, limit);
  }
  if (!['netease', 'tencent', 'kugou'].includes(platform)) {
    throw new Error('unsupported platform: ' + platform);
  }
  const instance = rawMeting(platform);
  // Tencent/Kugou return at most 30 rows per page; walk pages so the caller's
  // limit is respected. Netease accepts the full limit in one request.
  const paged = platform === 'tencent' || platform === 'kugou';
  const pageSize = paged ? Math.min(limit, 30) : limit;
  const pageCount = paged ? Math.max(1, Math.ceil(limit / pageSize)) : 1;
  const tracks = [];
  const seen = new Set();

  for (let page = 1; page <= pageCount && tracks.length < limit; page += 1) {
    const raw = await instance.search(query, { page, limit: pageSize });
    const pageTracks = mapSearchTracks(platform, parseJson(raw));
    for (const track of pageTracks) {
      const key = track.source_id || track.id || JSON.stringify(track);
      if (seen.has(key)) continue;
      seen.add(key);
      tracks.push(track);
      if (tracks.length >= limit) break;
    }
    if (pageTracks.length < pageSize) break;
  }
  return tracks.slice(0, limit);
}

async function resolve(params) {
  const platform = params.platform;
  const sourceId = params.source_id;
  if (!sourceId) throw new Error('source_id is required');

  if (platform === 'kuwo') return kuwoSong(sourceId);
  if (!['netease', 'tencent', 'kugou'].includes(platform)) {
    throw new Error('unsupported platform: ' + platform);
  }
  const instance = meting(platform);
  const track = toTrack(platform, first(await instance.song(sourceId)));
  if (!track) throw new Error('song not found');
  return track;
}

async function stream(params) {
  const platform = params.platform;
  const sourceId = params.source_id;
  if (!sourceId) throw new Error('source_id is required');

  if (platform === 'kuwo') {
    const url = await kuwoStream(sourceId);
    return { type: 'pipe', cmd: streamCommand(url, KW_HEADERS) };
  }
  if (!['netease', 'tencent', 'kugou'].includes(platform)) {
    throw new Error('unsupported platform: ' + platform);
  }

  const instance = meting(platform);
  const info = parseJson(await instance.url(sourceId, 320));
  if (!info || !info.url) {
    throw new Error(
      'no stream URL for ' +
        platform +
        ' (a login cookie may be required; put --cookie <cookie> in the plugin argv)',
    );
  }
  return {
    type: 'pipe',
    cmd: streamCommand(info.url, instance.header || {}),
  };
}

async function lyrics(params) {
  const platform = params.platform;
  const sourceId = params.source_id;
  if (platform === 'kuwo') return null;
  if (!['netease', 'tencent', 'kugou'].includes(platform)) return null;
  const instance = meting(platform);
  const data = parseJson(await instance.lyric(sourceId));
  if (!data || !data.lyric) return null;
  const text = data.tlyric ? data.lyric + '\n' + data.tlyric : data.lyric;
  return { text, format: 'lrc', language: 'zh' };
}

async function playlist(params) {
  const rawUrl = params.url || '';
  const parsed = parseUrl(rawUrl);
  if (!parsed) {
    throw new Error('unsupported URL (fallback provider may handle it)');
  }
  if (parsed.type === 'song') {
    const track = await resolve({ platform: parsed.platform, source_id: parsed.id });
    return [track];
  }

  if (parsed.platform === 'kuwo') {
    return kuwoPlaylist(parsed.id);
  }
  if (!['netease', 'tencent', 'kugou'].includes(parsed.platform)) {
    throw new Error('unsupported platform: ' + parsed.platform);
  }
  const instance = meting(parsed.platform);
  const raw = await instance.playlist(parsed.id);
  return asArray(raw).map((item) => toTrack(parsed.platform, item)).filter(Boolean);
}

async function handle(action, params) {
  switch (action) {
    case 'search':
      return search(params);
    case 'resolve':
      return resolve(params);
    case 'stream':
      return stream(params);
    case 'playlist':
      return playlist(params);
    case 'lyrics':
      return lyrics(params);
    default:
      throw new Error('not_supported');
  }
}

async function main() {
  let buffer = '';
  for await (const chunk of process.stdin) {
    buffer += chunk;
    if (buffer.includes('\n')) break;
  }
  const line = buffer.split('\n')[0].trim();
  if (!line) return;

  let request;
  try {
    request = JSON.parse(line);
  } catch {
    return;
  }
  const id = request.id;
  try {
    const result = await handle(request.action, request.params || {});
    process.stdout.write(JSON.stringify({ id, result }) + '\n');
  } catch (error) {
    process.stdout.write(
      JSON.stringify({ id, error: { code: 'failed', message: String(error.message || error) } }) + '\n',
    );
  }
}

main().catch((error) => {
  process.stderr.write(String(error.stack || error) + '\n');
  process.exitCode = 1;
});
