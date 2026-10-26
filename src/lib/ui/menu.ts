import type { ComponentType } from "svelte";

/**
 * One entry in a context menu / action sheet. The same data drives the
 * desktop context menu, the column menu and the mobile bottom sheet, so the
 * entries (and their `canManage` / `canDownload` gating) can never drift
 * between layouts.
 */
export interface MenuItem {
  label: string;
  icon?: ComponentType;
  action: () => void;
  disabled?: boolean;
  divider?: boolean;
}
