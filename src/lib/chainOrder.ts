// Pure rules for reordering the effects in a chain, by dragging or with the keyboard.

/**
 * The gap a dragged row would drop into (0..count): the number of rows whose middle is
 * above `y`. `mids` are the rows' vertical middles, in order.
 */
export function dropGap(mids: readonly number[], y: number): number {
  return mids.filter((m) => m < y).length;
}

/**
 * The `move_effect` index for dropping the effect at `from` into gap `gap` (0..length, in
 * the chain as it is), or null if that leaves it where it is. The backend takes the
 * effect out before inserting it, so gaps after it shift one to the left.
 */
export function dropIndex(from: number, gap: number, length: number): number | null {
  if (from < 0 || from >= length) return null;
  const g = Math.max(0, Math.min(length, gap));
  if (g === from || g === from + 1) return null;
  return g > from ? g - 1 : g;
}

/** The index for moving the effect at `from` one place earlier (-1) or later (1), or null at an end. */
export function stepIndex(from: number, step: -1 | 1, length: number): number | null {
  const to = from + step;
  return from < 0 || from >= length || to < 0 || to >= length ? null : to;
}
