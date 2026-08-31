import type { LyricsPosition } from "$lib/types";

/** One translated variant of a lyric line. */
export interface LyricPart {
  /** Language key from the plugin payload, used as the CSS class name. */
  lang: string;
  text: string;
}

export interface LyricLine {
  t: number;
  parts: LyricPart[];
}

/**
 * Default stylesheet for the floating lyrics window: it sits on top of
 * whatever is on screen, so the defaults are light text with a shadow.
 */
export const DEFAULT_DESKTOP_LYRICS_CSS = `/* 歌词区域 DOM 结构
<div class="lyrics-root" data-position="center">   根节点：铺满歌词区域，背景透明
  <div class="lyrics-viewport">                可视区：负责裁剪与自动滚动
    <div class="lyrics-lines">                 行容器
      <div class="line">                       一行歌词（可含多种语言）
        <p class="english">english lyric</p>    语言 class 来自插件 JSON 的键名
        <p class="chinese">中文歌词</p>
      </div>
      <div class="line playing">               正在播放的歌词行
        <p class="russian">ruskiy tekst 2</p>
        <p class="english">english lyric 2</p>
      </div>
    </div>
  </div>
</div>

下面所有规则都可以随意修改或删除。
*/

.lyrics-root {
  width: 100%;
  height: 100%;
  overflow: hidden;
  font-family: inherit;
}

.lyrics-viewport {
  width: 100%;
  height: 100%;
  overflow: hidden;
}

.lyrics-lines {
  text-align: center;
  padding: 8px 16px;
}

.line {
  margin: 6px 0;
  transition: 0.2s;
}

.line p {
  margin: 0;
  font-size: 14px;
  line-height: 1.5;
  color: rgba(255, 255, 255, 0.55);
  text-shadow: 0 1px 3px rgba(0, 0, 0, 0.8);
  transition: 0.2s;
}

.playing p {
  font-size: 16px;
  color: green;
}
`;

/**
 * Default stylesheet for the lyrics block on the player detail page, which
 * sits on the app background and follows the theme.
 */
export const DEFAULT_DETAIL_LYRICS_CSS = `/* 歌词区域 DOM 结构（与悬浮歌词一致）
<div class="lyrics-root" data-position="center">
  <div class="lyrics-viewport">
    <div class="lyrics-lines">
      <div class="line"><p class="english">english lyric</p></div>
      <div class="line playing"><p class="english">english lyric 2</p></div>
    </div>
  </div>
</div>

下面所有规则都可以随意修改或删除。
*/

.lyrics-root {
  width: 100%;
  height: 100%;
  overflow: hidden;
  font-family: inherit;
}

.lyrics-viewport {
  width: 100%;
  height: 100%;
  overflow: hidden;
}

.lyrics-lines {
  text-align: center;
  padding: 8px 16px;
}

.line {
  margin: 10px 0;
  transition: 0.2s;
}

.line p {
  margin: 0;
  font-size: 14px;
  line-height: 1.6;
  color: var(--color-base-content) / 0.6;
  transition: 0.2s;
}

.playing p {
  font-size: 16px;
  color: var(--color-base-content);
  font-weight: 600;
}
`;

const LRC_LINE = /\[(\d+):(\d+(?:\.\d+)?)\]\s*(.*)/;

/** Keep a plugin-provided language key usable as a CSS class name. */
function sanitizeClass(key: string): string {
  return key.replace(/[^A-Za-z0-9_-]/g, "") || "lyric";
}

/** Parse an LRC string into timed, single-language lines. */
export function parseLrc(raw: string): LyricLine[] {
  const out: LyricLine[] = [];
  for (const line of raw.split("\n")) {
    const m = line.match(LRC_LINE);
    if (m) {
      out.push({
        t: parseInt(m[1], 10) * 60 + parseFloat(m[2]),
        parts: [{ lang: "lyric", text: m[3] || "" }],
      });
    }
  }
  out.sort((a, b) => a.t - b.t);
  return out;
}

/**
 * Parse the plugin multi-language payload:
 * `[{ time: 10.3, english: "…", chinese: "…" }, …]`.
 * Every key except the timestamp becomes one `<p class="<key>">`.
 */
function parseMultilang(data: unknown[]): LyricLine[] {
  const out: LyricLine[] = [];
  for (const entry of data) {
    if (!entry || typeof entry !== "object") continue;
    const record = entry as Record<string, unknown>;
    const rawTime = record.time ?? record.t ?? 0;
    const time = typeof rawTime === "number" ? rawTime : Number(rawTime);
    const parts: LyricPart[] = [];
    for (const [key, value] of Object.entries(record)) {
      if (key === "time" || key === "t") continue;
      if (typeof value !== "string") continue;
      const text = value.trim();
      if (text) parts.push({ lang: sanitizeClass(key), text });
    }
    if (parts.length) out.push({ t: Number.isFinite(time) ? time : 0, parts });
  }
  out.sort((a, b) => a.t - b.t);
  return out;
}

/** Parse lyrics in either supported shape (LRC text or the JSON array). */
export function parseLyrics(raw: string): LyricLine[] {
  const trimmed = raw.trim();
  if (!trimmed) return [];
  if (trimmed.startsWith("[")) {
    try {
      const data: unknown = JSON.parse(trimmed);
      // LRC timestamps also start with "[", so only a JSON array wins.
      if (Array.isArray(data)) {
        const lines = parseMultilang(data);
        if (lines.length) return lines;
      }
    } catch {
      /* not JSON — treat it as LRC below */
    }
  }
  const timed = parseLrc(trimmed);
  if (timed.length) return timed;
  // Untimed lyrics: still show them, just without a highlighted line.
  return trimmed
    .split("\n")
    .map((text) => ({ t: 0, parts: [{ lang: "lyric", text }] }));
}

/** Index of the line that should be highlighted at `elapsed` seconds. */
export function activeLyricIndex(lines: LyricLine[], elapsed: number): number {
  let index = -1;
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].t <= elapsed) index = i;
    else break;
  }
  return index;
}

/** How far down the viewport the active line rests, as a 0..1 fraction. */
export function positionFactor(position: LyricsPosition): number {
  if (position === "top") return 0.2;
  if (position === "bottom") return 0.8;
  return 0.5;
}

/**
 * Scroll `viewport` so the active line sits at the configured position.
 * This is the one part of the lyrics layout that needs real geometry, which
 * is why it lives here instead of in the user stylesheet.
 */
export function scrollActiveLineIntoView(
  viewport: HTMLElement | null,
  active: HTMLElement | undefined,
  position: LyricsPosition,
  smooth = true,
) {
  if (!viewport || !active) return;
  const target =
    active.offsetTop - viewport.clientHeight * positionFactor(position) + active.clientHeight / 2;
  const top = Math.max(0, target);
  if (typeof viewport.scrollTo === "function") {
    viewport.scrollTo({ top, behavior: smooth ? "smooth" : "auto" });
  } else {
    viewport.scrollTop = top;
  }
}