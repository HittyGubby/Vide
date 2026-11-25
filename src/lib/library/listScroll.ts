/**
 * Last scroll offset of the library list.
 *
 * The list route is destroyed while the full player route (/player) is open,
 * so component state cannot carry the position across that switch. Module
 * state survives it and is applied before the (async) SQLite client state
 * read resolves, which is what makes "back from details" land on the same row.
 */
export const listScrollMemory = { top: 0 };
