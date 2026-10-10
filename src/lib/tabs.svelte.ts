// Open files, one tab each, and which one is active.
import type { ClipRuns, Metadata, PeakTile, Stats } from "./api";
import type { Selection } from "./selection";
import type { Viewport } from "./viewport";

export interface Tab {
  /** Also the backend `fileId`. */
  id: number;
  path: string;
  name: string;
  status: "loading" | "ready";
  /** Load progress; `visible` once the load has lasted PROGRESS_DELAY_MS. */
  progress: { fraction: number; visible: boolean };
  meta: Metadata | null;
  clipRuns: ClipRuns | null;
  wholeStats: Stats | null;
  selection: Selection | null;
  /** Stats currently displayed (whole file or selection). */
  stats: Stats | null;
  statsPending: boolean;
  // View state, saved here while the tab is inactive (live in `view`/`transport` while active).
  vp: Viewport | null;
  userScrolled: boolean;
  playhead: number;
  playStart: number;
}

export const tabs = $state({
  list: [] as Tab[],
  activeId: null as number | null,
});

/** Overview tiles per tab, kept outside reactive state (they hold large typed arrays). */
export const overviewCache = new Map<number, { columns: number; tile: PeakTile }>();

let nextId = 1;

export function newTab(path: string): Tab {
  return {
    id: nextId++,
    path,
    name: path.split(/[\\/]/).pop() || path,
    status: "loading",
    progress: { fraction: 0, visible: false },
    meta: null,
    clipRuns: null,
    wholeStats: null,
    selection: null,
    stats: null,
    statsPending: false,
    vp: null,
    userScrolled: false,
    playhead: 0,
    playStart: 0,
  };
}

export function findTab(id: number | null): Tab | null {
  return id === null ? null : (tabs.list.find((t) => t.id === id) ?? null);
}

export function activeTab(): Tab | null {
  return findTab(tabs.activeId);
}
