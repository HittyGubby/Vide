/**
 * One-shot guard for restoring the main window geometry.
 *
 * DesktopShell is unmounted while the full player route is open and mounted
 * again when the user returns to the list. Restoring the saved size on every
 * remount made the window grow slightly on each switch (outer size ↔ logical
 * size rounding), so geometry is restored once per app session only.
 */
export const windowGeometry = { restored: false };
