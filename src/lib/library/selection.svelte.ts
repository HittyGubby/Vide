/**
 * Multi-selection state for a list: an anchor row (Shift-range), a set of ids
 * and the Ctrl/Cmd toggle. Generic over the row type so the library and the
 * online-search list share one implementation instead of two copies.
 */
export class SelectionStore<T> {
  ids = $state<Set<string>>(new Set());
  anchor = $state<number | null>(null);

  private readonly keyOf: (item: T) => string;

  constructor(keyOf: (item: T) => string) {
    this.keyOf = keyOf;
  }

  get size(): number {
    return this.ids.size;
  }

  has(item: T): boolean {
    return this.ids.has(this.keyOf(item));
  }

  clear(): void {
    this.ids = new Set();
    this.anchor = null;
  }

  setKeys(keys: string[]): void {
    this.ids = new Set(keys);
  }

  /** Ctrl/Cmd click: add or remove one row. */
  toggle(item: T, index: number): void {
    const key = this.keyOf(item);
    const next = new Set(this.ids);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    this.ids = next;
    this.anchor = index;
  }

  setChecked(item: T, checked: boolean, index: number): void {
    const key = this.keyOf(item);
    const next = new Set(this.ids);
    if (checked) next.add(key);
    else next.delete(key);
    this.ids = next;
    this.anchor = index;
  }

  /** Shift click: select everything between the anchor and `index`. */
  selectRange(items: T[], index: number): void {
    const anchor = this.anchor ?? index;
    const start = Math.min(anchor, index);
    const end = Math.max(anchor, index);
    const next = new Set(this.ids);
    for (let i = start; i <= end; i += 1) next.add(this.keyOf(items[i]));
    this.ids = next;
    this.anchor = index;
  }

  selectAll(items: T[]): void {
    this.ids = new Set(items.map((item) => this.keyOf(item)));
  }

  /** Selected items in list order (not click order). */
  items(list: T[]): T[] {
    return list.filter((item) => this.ids.has(this.keyOf(item)));
  }
}
