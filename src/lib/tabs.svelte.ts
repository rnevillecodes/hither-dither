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
  /** Unsaved changes: something changed since the session was last saved or opened. */
  dirty: boolean;
  /** The audio is edited (differs from the WAV on disk), even with nothing to undo. */
  edited: boolean;
  canUndo: boolean;
  canRedo: boolean;
  /** Backend revision of the audio shown; increases whenever the audio changes. */
  revision: number;
  /** Increases whenever fetched peaks go stale (an edit, or its analysis finishing). */
  peaksVersion: number;
  /** The whole-file stats and clip markers are being brought up to date. */
  analysing: boolean;
  /** An export is in progress: the tab can't be edited until it finishes. */
  editing: boolean;
  /** Trim window, end fades and joins, as the clip handles show them. */
  clip: ClipInfo | null;
  /** The effects the audio is heard and exported through. */
  chain: ChainInfo | null;
  /** Effects selected in the Effects panel (slot ids), and the one last clicked. */
  fxSelected: number[];
  fxAnchor: number | null;
  /** Groups collapsed in the Effects panel. Not saved: groups start expanded. */
  collapsedGroups: number[];
  /** The session file this tab was opened from or last saved to; Save writes it. */
  sessionPath: string | null;
  /** The WAV was chosen by the user for `sessionPath`, instead of the session's own. */
  relink: boolean;
}

export const tabs = $state({
  list: [] as Tab[],
  activeId: null as number | null,
});

/** Overview tiles per tab, kept outside reactive state (they hold large typed arrays). */
export const overviewCache = new Map<number, { columns: number; version: number; tile: PeakTile }>();

let nextId = 1;

export function newTab(path: string, sessionPath: string | null = null, relink = false): Tab {
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
    edited: false,
    canUndo: false,
    canRedo: false,
    revision: 0,
    peaksVersion: 0,
    analysing: false,
    editing: false,
    clip: null,
    chain: null,
    fxSelected: [],
    fxAnchor: null,
    collapsedGroups: [],
    sessionPath,
    relink,
  };
}

export function findTab(id: number | null): Tab | null {
  return id === null ? null : (tabs.list.find((t) => t.id === id) ?? null);
}

export function activeTab(): Tab | null {
  return findTab(tabs.activeId);
}
