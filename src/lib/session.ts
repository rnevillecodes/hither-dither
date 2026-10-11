// The open files and active tab, remembered across restarts.

const SESSION_KEY = "hither-dither.session";
const VERSION = 1;

export interface Session {
  /** Open file paths, in tab order. */
  paths: string[];
  /** Path of the active tab. */
  active: string | null;
}

const EMPTY: Session = { paths: [], active: null };

/** The remembered session; empty if there is none or it can't be read. */
export function readSession(): Session {
  try {
    const raw = localStorage.getItem(SESSION_KEY);
    if (raw === null) return EMPTY;
    const data = JSON.parse(raw);
    if (data?.v !== VERSION || !Array.isArray(data.paths)) return EMPTY;
    const paths = data.paths.filter((p: unknown): p is string => typeof p === "string" && p !== "");
    const active = typeof data.active === "string" && paths.includes(data.active) ? data.active : null;
    return { paths, active };
  } catch {
    return EMPTY;
  }
}

export function writeSession(paths: readonly string[], active: string | null): void {
  try {
    localStorage.setItem(SESSION_KEY, JSON.stringify({ v: VERSION, paths, active }));
  } catch {
    // Storage unavailable: the next launch starts empty.
  }
}
