// Open files, one tab each, and which one is active.
import type { ChainInfo, ClipInfo, ClipRuns, Metadata, PeakTile, Stats } from "./api";
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
  // Editing.
  /** Unsaved changes: the audio differs from the file on disk. */
  dirty: boolean;
  canUndo: boolean;
  canRedo: boolean;
  /** Backend revision of the audio shown; increases whenever the audio changes. */
  revision: number;
  /** Increases whenever fetched peaks go stale (an edit, or its analysis finishing). */
  peaksVersion: number;
  /** The whole-file stats and clip markers are being brought up to date. */
  analysing: boolean;
  /** A save is in progress: the tab can't be edited until it finishes. */
  editing: boolean;
  /** The user has agreed to Save overwriting this file. */
  overwriteConfirmed: boolean;
  /** Trim window, end fades and joins, as the clip handles show them. */
  clip: ClipInfo | null;
  /** The effects the audio is heard and saved through. */
  chain: ChainInfo | null;
}

export const tabs = $state({
  list: [] as Tab[],
  activeId: null as number | null,
});

/** Overview tiles per tab, kept outside reactive state (they hold large typed arrays). */
export const overviewCache = new Map<number, { columns: number; version: number; tile: PeakTile }>();

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
    dirty: false,
    canUndo: false,
    canRedo: false,
    revision: 0,
    peaksVersion: 0,
    analysing: false,
    editing: false,
    overwriteConfirmed: false,
    clip: null,
    chain: null,
  };
}

export function findTab(id: number | null): Tab | null {
  return id === null ? null : (tabs.list.find((t) => t.id === id) ?? null);
}

export function activeTab(): Tab | null {
  return findTab(tabs.activeId);
}
