import { loadClientState, saveClientState } from "$lib/clientState";
import {
  defaultColumnPrefs,
  normalizeColumnPrefs,
  visibleColumns,
  type TableColumnDef,
  type TableColumnPrefs,
} from "$lib/table/columns";

/** Resize deltas for one column map (widths by column id). */
export type ColumnSizing = Record<string, number>;

/**
 * Column layout state for a table: widths, order and visibility, persisted in
 * the per-client state store.
 *
 * Width changes keep the table total constant by taking the delta from the next
 * resizable column, so rows always fill their container and only the dragged
 * boundary moves. Both the library table and the online-search table use this,
 * which is why the rule lives here instead of in either page.
 */
export class ColumnLayout {
  readonly defs: TableColumnDef[];
  readonly storageKey: string;
  prefs = $state<TableColumnPrefs>({ sizing: {}, order: [], visibility: {} });

  private saveTimer: ReturnType<typeof setTimeout> | undefined;

  constructor(defs: TableColumnDef[], storageKey: string) {
    this.defs = defs;
    this.storageKey = storageKey;
    this.prefs = defaultColumnPrefs(defs);
  }

  defOf(id: string): TableColumnDef {
    return this.defs.find((def) => def.id === id) ?? this.defs[0];
  }

  isFixed(id: string): boolean {
    return this.defOf(id).fixed === true;
  }

  widthOf(id: string): number {
    const def = this.defOf(id);
    return this.prefs.sizing[id] ?? def.defWidth;
  }

  /** Visible columns in display order. */
  visible(): TableColumnDef[] {
    return visibleColumns(this.defs, this.prefs);
  }

  visibleIds(): string[] {
    return this.prefs.order.filter((id) => this.prefs.visibility[id] !== false);
  }

  /** The visible, resizable column after `id` — the one that gives up width. */
  nextResizable(id: string): string | null {
    const ids = this.visibleIds();
    for (let i = ids.indexOf(id) + 1; i < ids.length; i += 1) {
      if (!this.isFixed(ids[i])) return ids[i];
    }
    return null;
  }

  /**
   * TanStack resizes a single column, which would change the table's total
   * width and break the "row fills the container" layout. Take the delta from
   * the next resizable column instead.
   */
  withAdjacentSizing(next: ColumnSizing): ColumnSizing {
    let changed: string | null = null;
    let delta = 0;
    for (const id of Object.keys(next)) {
      const prev = this.prefs.sizing[id];
      if (prev != null && prev !== next[id]) {
        changed = id;
        delta = next[id] - prev;
      }
    }
    if (changed == null || delta === 0) return next;
    const neighbour = this.nextResizable(changed);
    if (neighbour == null) return next;
    const neighbourSize = next[neighbour] ?? this.defOf(neighbour).defWidth;
    // Growing: the neighbour gives up space, never dropping below its minimum.
    const applied =
      delta > 0 ? Math.min(delta, neighbourSize - this.defOf(neighbour).min) : delta;
    return {
      ...next,
      [changed]: this.prefs.sizing[changed] + applied,
      [neighbour]: neighbourSize - applied,
    };
  }

  /** Set one column's width, keeping the table total constant. */
  setWidth(id: string, width: number): void {
    const def = this.defOf(id);
    if (def.fixed) return;
    const next = this.withAdjacentSizing({
      ...this.prefs.sizing,
      [id]: Math.max(width, def.min),
    });
    this.prefs = { ...this.prefs, sizing: next };
    this.save();
  }

  /** Double-clicking a resize handle restores the default width. */
  resetWidth(id: string): void {
    this.setWidth(id, this.defOf(id).defWidth);
  }

  /** Scale the resizable columns so the table keeps filling `width`. */
  fitToWidth(width: number): void {
    const visible = this.visibleIds();
    const resizable = visible.filter((id) => !this.isFixed(id));
    const fixedTotal = visible.reduce(
      (sum, id) => sum + (this.isFixed(id) ? this.widthOf(id) : 0),
      0,
    );
    const target = width - fixedTotal;
    const total = resizable.reduce((sum, id) => sum + this.widthOf(id), 0);
    if (resizable.length === 0 || total <= 0 || target <= 0) return;
    const scale = target / total;
    if (Math.abs(scale - 1) < 0.01) return;
    const sizing: ColumnSizing = { ...this.prefs.sizing };
    for (const id of resizable) {
      sizing[id] = Math.max(this.defOf(id).min, Math.round(this.widthOf(id) * scale));
    }
    this.prefs = { ...this.prefs, sizing };
    this.save();
  }

  setOrder(order: string[]): void {
    this.prefs = { ...this.prefs, order };
    this.save();
  }

  /** Move `sourceId` to `targetId`'s position (drag reorder). */
  move(sourceId: string, targetId: string): void {
    const from = this.prefs.order.indexOf(sourceId);
    const to = this.prefs.order.indexOf(targetId);
    if (from < 0 || to < 0) return;
    const order = [...this.prefs.order];
    order.splice(to, 0, ...order.splice(from, 1));
    this.setOrder(order);
  }

  setVisibility(visibility: Record<string, boolean>): void {
    this.prefs = { ...this.prefs, visibility };
    this.save();
  }

  /** Toggle one column; the last visible column cannot be hidden. */
  toggleVisibility(id: string): void {
    const shown = this.prefs.visibility[id] !== false;
    if (shown && this.visibleIds().length <= 1) return;
    this.setVisibility({ ...this.prefs.visibility, [id]: !shown });
  }

  /** Persist after a short idle period; drag gestures fire far too often. */
  save(): void {
    if (this.saveTimer) clearTimeout(this.saveTimer);
    this.saveTimer = setTimeout(() => {
      this.saveTimer = undefined;
      void saveClientState(this.storageKey, $state.snapshot(this.prefs));
    }, 250);
  }

  /** Write immediately (component teardown). */
  flush(): void {
    if (this.saveTimer) {
      clearTimeout(this.saveTimer);
      this.saveTimer = undefined;
    }
    void saveClientState(this.storageKey, $state.snapshot(this.prefs));
  }

  /**
   * Load the stored layout. `legacyKey` is a one-time localStorage key written
   * by older builds; it is migrated into the client state store and removed.
   */
  async load(legacyKey?: string): Promise<void> {
    const stored = await loadClientState<unknown>(this.storageKey);
    if (stored) {
      this.prefs = normalizeColumnPrefs(this.defs, stored);
      if (legacyKey) localStorage.removeItem(legacyKey);
      return;
    }
    if (!legacyKey) return;
    try {
      const legacy = localStorage.getItem(legacyKey);
      if (!legacy) return;
      this.prefs = normalizeColumnPrefs(this.defs, JSON.parse(legacy));
      this.flush();
      localStorage.removeItem(legacyKey);
    } catch {
      /* ignore corrupt legacy data */
    }
  }
}
