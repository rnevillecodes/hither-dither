// Pure tab-strip rules: which tab becomes active after a close, cycling, and path matching.

/** The tab to activate after `closing` is removed from `ids` while `active` is active. */
export function neighbourAfterClose<T>(ids: readonly T[], closing: T, active: T | null): T | null {
  if (closing !== active) return active;
  const i = ids.indexOf(closing);
  if (i < 0) return active;
  return ids[i + 1] ?? ids[i - 1] ?? null;
}

/** The tab after (`step` 1) or before (`step` -1) `active`, wrapping around at the ends. */
export function cycle<T>(ids: readonly T[], active: T | null, step: 1 | -1): T | null {
  if (ids.length === 0) return null;
  const i = active === null ? -1 : ids.indexOf(active);
  if (i < 0) return ids[0];
  return ids[(i + step + ids.length) % ids.length];
}

/**
 * `ids` with `id` moved into the gap before `ids[to]` (`to` counts gaps in the original
 * list, 0..length, and is clamped). Unknown ids leave the order unchanged.
 */
export function move<T>(ids: readonly T[], id: T, to: number): T[] {
  const out = [...ids];
  const from = out.indexOf(id);
  if (from < 0) return out;
  const gap = Math.max(0, Math.min(out.length, Math.round(to)));
  out.splice(from, 1);
  // Removing the tab shifts the gaps after it one to the left.
  out.splice(gap > from ? gap - 1 : gap, 0, id);
  return out;
}

/** Normalise a path for comparison: forward slashes, no trailing slash, optionally lower case. */
export function pathKey(path: string, caseInsensitive: boolean): string {
  const p = path.replace(/[\\/]+/g, "/").replace(/(.)\/$/, "$1");
  return caseInsensitive ? p.toLowerCase() : p;
}

/** Whether two paths name the same file (case-insensitively on Windows). */
export function samePath(a: string, b: string, caseInsensitive = isWindows()): boolean {
  return pathKey(a, caseInsensitive) === pathKey(b, caseInsensitive);
}

function isWindows(): boolean {
  return typeof navigator !== "undefined" && navigator.userAgent.includes("Windows");
}
