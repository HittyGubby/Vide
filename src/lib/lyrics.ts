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
        <p class="lyric-1">english lyric</p>    同一时间的第 1 行
        <p class="lyric-2">中文歌词</p>          同一时间的第 2 行
      </div>
      <div class="line playing">               正在播放的歌词行
        <p class="lyric-1">ruskiy tekst 2</p>
        <p class="lyric-2">english lyric 2</p>
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

/* 同一时间的多行：lyric-1、lyric-2、lyric-3… 按出现顺序 */
.line .lyric-1 { color: rgba(255, 255, 255, 0.8); }
.line .lyric-2 { color: rgba(255, 220, 180, 0.75); }

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
      <div class="line"><p class="lyric-1">english lyric</p></div>
      <div class="line playing"><p class="lyric-1">english lyric 2</p></div>
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
  color: color-mix(in oklch, var(--color-base-content) 60%, transparent);
  transition: 0.2s;
}

/* 同一时间的多行：lyric-1、lyric-2、lyric-3… 按出现顺序 */
.line .lyric-1 { color: color-mix(in oklch, var(--color-base-content) 85%, transparent); }
.line .lyric-2 { color: color-mix(in oklch, var(--color-base-content) 60%, transparent); }

.playing p {
  font-size: 16px;
  color: var(--color-base-content);
  font-weight: 600;
}
`;

const LRC_LINE = /\[(\d+):(\d+(?:\.\d+)?)\]\s*(.*)/;

/** Assign stable, order-based CSS classes; language is never inferred. */
function orderedParts(texts: string[]): LyricPart[] {
  return texts
    .map((text) => text.trim())
    .filter(Boolean)
    .map((text, index) => ({ lang: "lyric-" + (index + 1), text }));
}

/** Parse LRC lines, grouping equal timestamps into one ordered line. */
export function parseLrc(raw: string): LyricLine[] {
  const groups = new Map<number, string[]>();
  for (const line of raw.split("\n")) {
    const m = line.match(LRC_LINE);
    if (!m) continue;
    const time = parseInt(m[1], 10) * 60 + parseFloat(m[2]);
    const text = (m[3] || "").trim();
    if (!text) continue;
    const parts = groups.get(time) ?? [];
    // Within one timestamp the source order is the display order. Keep every
    // line: some providers append more than the usual original + translation.
    parts.push(text);
    groups.set(time, parts);
  }
  return [...groups.entries()]
    .sort((a, b) => a[0] - b[0])
    .map(([t, texts]) => ({ t, parts: orderedParts(texts) }));
}

/**
 * Parse the plugin multi-language payload in object key order. Keys are not
 * used as CSS classes; values receive lyric-1, lyric-2, ... in source order.
 */
function parseMultilang(data: unknown[]): LyricLine[] {
  const out: LyricLine[] = [];
  for (const entry of data) {
    if (!entry || typeof entry !== "object") continue;
    const record = entry as Record<string, unknown>;
    const rawTime = record.time ?? record.t ?? 0;
    const time = typeof rawTime === "number" ? rawTime : Number(rawTime);
    const texts: string[] = [];
    for (const [key, value] of Object.entries(record)) {
      if (key === "time" || key === "t") continue;
      if (typeof value === "string") texts.push(value);
    }
    const parts = orderedParts(texts);
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
    .filter((text) => text.trim())
    .map((text) => ({ t: 0, parts: orderedParts([text]) }));
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

/* ── Stylesheet compatibility ─────────────────────────── */

const LEGACY_ALPHA = /var\((--[\w-]+)\)\s*\/\s*([0-9.]+)/g;

/**
 * Older default stylesheets (and hand-written user CSS) used the invalid
 * `var(--x) / 0.6` opacity syntax, which browsers drop entirely. Rewrite it to
 * `color-mix` when injecting, without touching the text the user stored or is
 * editing.
 */
export function normalizeCssColors(css: string): string {
  return css.replace(
    LEGACY_ALPHA,
    (_match, variable: string, alpha: string) =>
      "color-mix(in oklch, var(" +
      variable +
      ") " +
      Math.round(parseFloat(alpha) * 1000) / 10 +
      "%, transparent)",
  );
}
