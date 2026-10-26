import { goto } from "$app/navigation";

/** Open the full player route (phones and desktop use the same route). */
export function openPlayer(): void {
  void goto("/player");
}

/**
 * Leave the player route. A deep link straight to /player has no app history
 * to return to, so it falls back to the library.
 */
export function closePlayer(): void {
  if (window.history.length > 1) history.back();
  else void goto("/", { replaceState: true });
}
