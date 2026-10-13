// Levels from playback on their way to the level meter. Kept out of Svelte state: they
// arrive about 60 times a second, and the meter reads them once per animation frame.

type Levels = [number, number][];

let pending: Levels | null = null;
let starts = 0;

/** Newly heard `[peak, rms]` per channel (linear). Reports not yet drawn are merged. */
export function pushLevels(levels: Levels) {
  if (levels.length === 0) return;
  if (pending && pending.length === levels.length) {
    pending = levels.map(([peak, rms], c) => [Math.max(peak, pending![c][0]), rms]);
  } else {
    pending = levels;
  }
}

/** The levels heard since the last call, or null if none. */
export function takeLevels(): Levels | null {
  const levels = pending;
  pending = null;
  return levels;
}

/** Playback is starting: drop old levels; the meter clears its readouts. */
export function meterPlaybackStarted() {
  pending = null;
  starts++;
}

/** Counts playback starts, so the meter can tell when to clear its readouts. */
export function playbackStarts(): number {
  return starts;
}
