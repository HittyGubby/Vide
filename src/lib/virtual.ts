export interface VirtualSlice {
  start: number;
  end: number;
  padTop: number;
  padBottom: number;
}

/**
 * The rendered window of a fixed-height list.
 *
 * `scrollTop` and `viewportHeight` are clamped against the current content so
 * a shorter result set never keeps padding computed from the previous one.
 */
export function virtualSlice(
  count: number,
  rowHeight: number,
  scrollTop: number,
  viewportHeight: number,
  overscan = 8,
): VirtualSlice {
  if (count === 0 || rowHeight <= 0) {
    return { start: 0, end: 0, padTop: 0, padBottom: 0 };
  }
  const height = viewportHeight || 600;
  const maxTop = Math.max(0, count * rowHeight - height);
  const top = Math.min(Math.max(0, scrollTop), maxTop);
  const first = Math.max(0, Math.floor(top / rowHeight) - overscan);
  const last = Math.min(count, Math.ceil((top + height) / rowHeight) + overscan);
  return {
    start: first,
    end: last,
    padTop: first * rowHeight,
    padBottom: Math.max(0, (count - last) * rowHeight),
  };
}
