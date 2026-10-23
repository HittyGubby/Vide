export interface TableColumnDef {
  id: string;
  label: string;
  defWidth: number;
  min: number;
  fixed?: boolean;
}

export interface TableColumnPrefs {
  sizing: Record<string, number>;
  order: string[];
  visibility: Record<string, boolean>;
}

export function defaultColumnPrefs(defs: TableColumnDef[]): TableColumnPrefs {
  return {
    sizing: Object.fromEntries(defs.map((def) => [def.id, def.defWidth])),
    order: defs.map((def) => def.id),
    visibility: Object.fromEntries(defs.map((def) => [def.id, true])),
  };
}

export function normalizeColumnPrefs(
  defs: TableColumnDef[],
  data: unknown,
): TableColumnPrefs {
  const fallback = defaultColumnPrefs(defs);
  if (!data || typeof data !== "object" || Array.isArray(data)) return fallback;
  const raw = data as Record<string, unknown>;
  const rawSizing = (raw.sizing ?? {}) as Record<string, unknown>;
  const rawVisibility = (raw.visibility ?? {}) as Record<string, unknown>;
  const sizing = { ...fallback.sizing };
  for (const def of defs) {
    const width = Number(rawSizing[def.id]);
    if (Number.isFinite(width) && width > 0) sizing[def.id] = Math.max(width, def.min);
  }
  const order = Array.isArray(raw.order)
    ? raw.order.filter((id): id is string => defs.some((def) => def.id === id))
    : [];
  for (const def of defs) if (!order.includes(def.id)) order.push(def.id);
  const visibility = { ...fallback.visibility };
  for (const def of defs) {
    if (typeof rawVisibility[def.id] === "boolean") {
      visibility[def.id] = rawVisibility[def.id] as boolean;
    }
  }
  if (!defs.some((def) => visibility[def.id] !== false)) visibility[defs[0].id] = true;
  return { sizing, order, visibility };
}

export function visibleColumns(
  defs: TableColumnDef[],
  prefs: TableColumnPrefs,
): TableColumnDef[] {
  return prefs.order
    .filter((id) => prefs.visibility[id] !== false)
    .map((id) => defs.find((def) => def.id === id))
    .filter((def): def is TableColumnDef => Boolean(def));
}
