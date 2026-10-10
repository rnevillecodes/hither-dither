// Time-selection math. Selections are half-open frame ranges [start, end).

export interface Selection {
  start: number;
  end: number;
}

/** Normalised selection between two frames, or null if empty. */
export function between(a: number, b: number): Selection | null {
  const start = Math.min(a, b);
  const end = Math.max(a, b);
  return end > start ? { start, end } : null;
}

/** Shift+click: move whichever edge is nearer to `frame`; from `anchor` when nothing is selected. */
export function extend(sel: Selection | null, frame: number, anchor: number): Selection | null {
  if (!sel) return between(anchor, frame);
  const nearStart = Math.abs(frame - sel.start) <= Math.abs(frame - sel.end);
  return nearStart ? between(frame, sel.end) : between(sel.start, frame);
}

export interface SelectionTimes {
  start: number;
  end: number;
  duration: number;
}

export function times(sel: Selection, sampleRate: number): SelectionTimes {
  return {
    start: sel.start / sampleRate,
    end: sel.end / sampleRate,
    duration: (sel.end - sel.start) / sampleRate,
  };
}

/** Pointer movement (CSS px) below which a press counts as a click, not a drag. */
export const DRAG_THRESHOLD_PX = 3;
