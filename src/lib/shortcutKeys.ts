/**
 * Accelerator strings use the `global-hotkey` / `Code` spelling so one string
 * works for both in-app matching and OS-level global shortcuts:
 * `Ctrl`/`Alt`/`Shift`/`Super` followed by a key such as `Space`, `PageUp`,
 * `ArrowUp`, `F4` or `KeyA`.
 */

/**
 * Normalise a key name to the `Code` spelling, so "a"/"KeyA", "up"/"ArrowUp"
 * and "esc"/"Escape" all compare equal.
 */
export function canonicalKeyName(name: string): string {
  const k = name.trim().toUpperCase();
  if (!k) return "";
  if (/^[A-Z]$/.test(k)) return "KEY" + k;
  if (/^[0-9]$/.test(k)) return "DIGIT" + k;
  if (k === "UP") return "ARROWUP";
  if (k === "DOWN") return "ARROWDOWN";
  if (k === "LEFT") return "ARROWLEFT";
  if (k === "RIGHT") return "ARROWRIGHT";
  if (k === "ESC") return "ESCAPE";
  if (k === "COMMAND" || k === "CMD") return "SUPER";
  if (k === "CONTROL") return "CTRL";
  if (k === "OPTION") return "ALT";
  return k;
}

/**
 * Bare modifier presses (`e.key`) — pressing one of these starts a
 * combination, it is not the shortcut itself. Matching on `key` rather than
 * `code` matters: `code` would be `ControlLeft`/`ShiftRight`/… and recording
 * it would produce bindings like "Ctrl+CONTROLLEFT".
 */
const MODIFIER_KEYS = new Set(["Shift", "Control", "Alt", "Meta", "OS", "CapsLock"]);

/** Split "Shift+Alt+F4" into its modifier set and its main key. */
export function parseAccelerator(accelerator: string): {
  mods: Set<string>;
  key: string;
} | null {
  const tokens = accelerator
    .split("+")
    .map((t) => t.trim())
    .filter((t) => t.length > 0);
  if (tokens.length === 0) return null;
  const key = canonicalKeyName(tokens[tokens.length - 1]);
  if (!key) return null;
  const mods = new Set(
    tokens
      .slice(0, -1)
      .map((t) => canonicalKeyName(t))
      .map((t) => (t === "CTRL" ? "CONTROL" : t === "SUPER" ? "META" : t)),
  );
  return { mods, key };
}

/** Does this keyboard event satisfy `accelerator`? */
export function eventMatchesAccelerator(e: KeyboardEvent, accelerator: string): boolean {
  const parsed = parseAccelerator(accelerator);
  if (!parsed) return false;
  if (canonicalKeyName(e.code) !== parsed.key) return false;
  return (
    e.ctrlKey === parsed.mods.has("CONTROL") &&
    e.altKey === parsed.mods.has("ALT") &&
    e.shiftKey === parsed.mods.has("SHIFT") &&
    e.metaKey === parsed.mods.has("META")
  );
}

/**
 * Build an accelerator string from a key event while the user is recording a
 * shortcut. Returns null for a bare modifier press, which means "keep waiting
 * for the real key".
 */
export function acceleratorFromEvent(e: KeyboardEvent): string | null {
  if (MODIFIER_KEYS.has(e.key)) return null;
  const key = canonicalKeyName(e.code);
  if (!key) return null;
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Super");
  parts.push(key);
  return parts.join("+");
}

/** Human readable form of an accelerator, e.g. "Shift + ↑". */
const PRETTY_KEYS: Record<string, string> = {
  ARROWUP: "↑",
  ARROWDOWN: "↓",
  ARROWLEFT: "←",
  ARROWRIGHT: "→",
  SPACE: "Space",
  PAGEUP: "PageUp",
  PAGEDOWN: "PageDown",
};

export function formatAccelerator(accelerator: string): string {
  const parsed = parseAccelerator(accelerator);
  if (!parsed) return "无";
  const order = ["CONTROL", "ALT", "SHIFT", "META"];
  const modLabels: Record<string, string> = {
    CONTROL: "Ctrl",
    ALT: "Alt",
    SHIFT: "Shift",
    META: "Super",
  };
  const mods = order
    .filter((m) => parsed.mods.has(m))
    .map((m) => modLabels[m]);
  const key = PRETTY_KEYS[parsed.key] ?? parsed.key.replace(/^KEY/, "").replace(/^DIGIT/, "");
  return [...mods, key].join(" + ");
}