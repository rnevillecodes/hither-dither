// The level meter beside the waveform: its dBFS scale, and how its bars move.

/** Bottom of the scale; quieter levels show as empty. */
export const FLOOR_DB = -60;
/** Labelled marks on the scale, top to bottom. */
export const SCALE_MARKS = [0, -6, -12, -18, -24, -36, -60];
/** Peak bar segments from here up use the warning colour. */
export const WARN_DB = -6;
/** How fast falling bars and hold ticks drop. */
export const RELEASE_DB_PER_S = 20;
/** How long a hold tick stays before falling. */
export const HOLD_S = 1.5;

/** One channel's meter state. Levels are in dBFS; -Infinity is silence. */
export interface ChannelMeter {
  /** Peak bar as drawn. */
  peak: number;
  /** RMS bar as drawn. */
  rms: number;
  /** Hold tick, and how long it has stayed since it was last raised. */
  hold: number;
  holdAge: number;
  /** Highest peak since the readouts were cleared. */
  max: number;
  /** A full-scale sample was measured since the readouts were cleared. */
  clip: boolean;
}

export function dbfs(linear: number): number {
  return linear > 0 ? 20 * Math.log10(linear) : -Infinity;
}

/** Position of `db` on the scale: 0 at the floor, 1 at 0 dBFS. */
export function levelToFraction(db: number): number {
  if (!(db > FLOOR_DB)) return 0;
  return Math.min(1, (db - FLOOR_DB) / -FLOOR_DB);
}

export function createMeter(channels: number): ChannelMeter[] {
  return Array.from({ length: channels }, () => ({
    peak: -Infinity,
    rms: -Infinity,
    hold: -Infinity,
    holdAge: 0,
    max: -Infinity,
    clip: false,
  }));
}

function fall(db: number, dt: number): number {
  const v = db - RELEASE_DB_PER_S * dt;
  return v > FLOOR_DB ? v : -Infinity;
}

/**
 * Advance `meter` by `dt` seconds. `input` is the newly heard `[peak, rms]` per channel
 * (linear), or null if none arrived. While `playing` without new input the RMS bar keeps
 * its value; once stopped, it falls like the peak.
 */
export function stepBallistics(
  meter: ChannelMeter[],
  input: readonly (readonly [number, number])[] | null,
  dt: number,
  playing: boolean,
): void {
  meter.forEach((m, c) => {
    const level = input?.[c];
    const peak = level ? dbfs(level[0]) : -Infinity;
    m.peak = Math.max(peak, fall(m.peak, dt));
    if (level) m.rms = dbfs(level[1]);
    else if (!playing) m.rms = fall(m.rms, dt);
    if (peak >= m.hold) {
      m.hold = peak;
      m.holdAge = 0;
    } else {
      m.holdAge += dt;
      if (m.holdAge > HOLD_S) m.hold = fall(m.hold, Math.min(dt, m.holdAge - HOLD_S));
    }
    m.max = Math.max(m.max, peak);
    if (level && level[0] >= 1) m.clip = true;
  });
}

/** Nothing left to animate: every bar and tick is empty. */
export function atRest(meter: readonly ChannelMeter[]): boolean {
  return meter.every((m) => m.peak === -Infinity && m.rms === -Infinity && m.hold === -Infinity);
}

/** Clear the maximum peak readouts and clip lights. */
export function clearReadouts(meter: ChannelMeter[]): void {
  for (const m of meter) {
    m.max = -Infinity;
    m.clip = false;
  }
}

/** A maximum peak readout: dBFS to one decimal place, or "–∞" for silence. */
export function formatReadout(db: number): string {
  if (db === -Infinity) return "–∞";
  const v = Math.abs(db) < 0.05 ? 0 : db;
  return v.toFixed(1);
}
