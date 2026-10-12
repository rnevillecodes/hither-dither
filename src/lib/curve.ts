// Fade curve shape: a power curve with one bend value. Mirrors src-tauri/src/edit/curve.rs;
// both test suites check the same table of values.

export type FadeDir = "in" | "out";

/** Clamp a bend to [-1, 1] (0 = linear, positive bows the fade line upward). */
export function clampBend(bend: number): number {
  return Number.isFinite(bend) ? Math.max(-1, Math.min(1, bend)) : 0;
}

/** Exponent of the power curve: 4^-bend. */
function power(bend: number): number {
  return Math.pow(4, -clampBend(bend));
}

/** Gain at position `t` (0 at the fade's start, 1 at its end). */
export function gain(dir: FadeDir, bend: number, t: number): number {
  const c = Math.max(0, Math.min(1, t));
  const x = dir === "in" ? c : 1 - c;
  return bend === 0 ? x : Math.pow(x, power(bend));
}

/** Height (gain) of the bend point, which sits halfway through the fade. */
export function bendPointHeight(bend: number): number {
  return Math.pow(0.5, power(bend));
}

/** The bend that puts the bend point at gain `height`. */
export function bendFromPoint(height: number): number {
  // 0.5^p = h  =>  p = ln h / ln 0.5, and p = 4^-bend  =>  bend = -log4 p.
  const h = Math.max(bendPointHeight(-1), Math.min(bendPointHeight(1), height));
  const p = Math.log(h) / Math.log(0.5);
  return clampBend(-Math.log(p) / Math.log(4));
}
