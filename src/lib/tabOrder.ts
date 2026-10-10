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
