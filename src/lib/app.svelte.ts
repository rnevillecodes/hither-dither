// The active document (facade over the active tab), loading a file into a tab, and
// selection statistics.
import {
  type EditState,
  type EffectWarning,
  type Metadata,
  type RestoreOutcome,
  closeFile,
  computeStats,
  getChain,
  getClip,
  getClipRuns,
  getMetadata,
  openFile,
  restoreSession,
} from "./api";
import { notify } from "./notices.svelte";
import { setChain, setEditState } from "./outcome";
import type { Selection } from "./selection";
import { type Tab, activeTab } from "./tabs.svelte";

/** Loads shorter than this never show the progress indicator. */
export const PROGRESS_DELAY_MS = 500;

/**
 * The active tab's document. Components read and write these as before; they forward
 * to whichever tab is active (null/false when none is, or while it is still loading).
 */
export const app = {
  get meta() {
    return activeTab()?.meta ?? null;
  },
  get clipRuns() {
    return activeTab()?.clipRuns ?? null;
  },
  get clip() {
    return activeTab()?.clip ?? null;
  },
  get wholeStats() {
    return activeTab()?.wholeStats ?? null;
  },
  get selection() {
    return activeTab()?.selection ?? null;
  },
  set selection(sel: Selection | null) {
    const tab = activeTab();
    if (tab) tab.selection = sel;
  },
  get stats() {
    return activeTab()?.stats ?? null;
  },
  get statsPending() {
    return activeTab()?.statsPending ?? false;
  },
  /** The whole-file stats and clip markers are being brought up to date after an edit. */
  get analysing() {
    return activeTab()?.analysing ?? false;
  },
};

/**
 * Load `tab.path` into `tab` (a reactive tab from `tabs.list`). Resolves when the tab is
 * ready; rejects with the backend's error message on failure (`…was cancelled` if it
 * was closed meanwhile).
 */
export async function loadIntoTab(tab: Tab): Promise<void> {
  const timer = setTimeout(() => (tab.progress.visible = true), PROGRESS_DELAY_MS);
  try {
    let meta = await openFile(tab.id, tab.path, (p) => {
      tab.progress.fraction = p.totalFrames ? p.doneFrames / p.totalFrames : 1;
    });
    // Before anything is fetched or the tab can be edited, so all of it is the session's.
    const restored = tab.sessionPath ? await restoreInto(tab, tab.sessionPath) : null;
    if (restored) meta = (await getMetadata(tab.id)) ?? meta;
    await showLoaded(tab, meta, restored?.editState ?? null);
    if (restored) showRestoreNotices(meta.name, restored);
  } finally {
    clearTimeout(timer);
  }
}

/**
 * Show `tab`'s file as just loaded (opened, restored or replaced): its metadata, whole-file
 * stats, clip runs, clip and chain, with `editState` when it isn't a fresh file's.
 */
export async function showLoaded(tab: Tab, meta: Metadata, editState: EditState | null): Promise<void> {
  const [whole, clipRuns, clip, chain] = await Promise.all([
    computeStats(tab.id, null),
    getClipRuns(tab.id),
    getClip(tab.id),
    getChain(tab.id),
  ]);
  tab.meta = meta;
  tab.clip = clip;
  setChain(tab, chain);
  tab.wholeStats = whole;
  tab.stats = whole;
  tab.clipRuns = clipRuns;
  if (editState) {
    setEditState(tab, editState);
    tab.revision = editState.revision;
    // Edits not analysed yet are analysed in the background, as after an edit (see `loadTab`).
    tab.analysing = whole === null;
  }
  tab.status = "ready";
  if (meta.truncated) {
    notify(
      "warning",
      `“${meta.name}” appears truncated`,
      "The data chunk is shorter than its header declares. The audio that is present was loaded.",
    );
  }
}

/** Apply session `sessionPath` to `tab`'s file, just opened. On failure the file is released. */
async function restoreInto(tab: Tab, sessionPath: string): Promise<RestoreOutcome> {
  let restored: RestoreOutcome;
  try {
    restored = await restoreSession(tab.id, sessionPath, tab.relink);
  } catch (e) {
    void closeFile(tab.id);
    throw e;
  }
  if (!restored.applied) {
    void closeFile(tab.id);
    throw new SessionMismatch(sessionPath, tab.name);
  }
  return restored;
}

/** A session's WAV no longer matches it: the tab is closed, and another WAV can be chosen. */
export class SessionMismatch extends Error {
  constructor(
    readonly sessionPath: string,
    readonly audioName: string,
  ) {
    super(`The audio file “${audioName}” no longer matches the session.`);
  }
}

function showRestoreNotices(name: string, restored: RestoreOutcome) {
  if (restored.wavChanged) {
    notify(
      "warning",
      `“${name}” has changed since the session was saved`,
      "Its format and length are the same, so the session was restored. Check that it is the recording you expect.",
    );
  }
  if (restored.editsDropped) {
    notify(
      "warning",
      "The session's edits were left out",
      `“${name}” is a different recording from the one the session was saved with, so only its effects were restored.`,
    );
  }
  showEffectWarnings(restored.warnings);
}

/** Effects that couldn't be brought across to a tab's audio, as one notice. */
export function showEffectWarnings(warnings: EffectWarning[]) {
  if (warnings.length > 0) {
    notify(
      "warning",
      warnings.length === 1 ? "An effect wasn't fully restored" : "Some effects weren't fully restored",
      warnings.map((w) => `${w.effect}: ${w.reason}`).join("\n"),
    );
  }
}

/** Latest selection-stats request per tab id. */
const statsTokens = new Map<number, number>();
let nextStatsToken = 0;

/**
 * Set the active tab's selection and refresh its stats (or restore whole-file stats).
 * The result goes to the tab that asked, even if another tab is active by then. If a
 * request from another tab supersedes it, `statsPending` stays set and the stats are
 * requested again when the tab is next activated.
 */
export async function setSelection(sel: Selection | null): Promise<void> {
  const tab = activeTab();
  if (!tab) return;
  tab.selection = sel;
  const token = ++nextStatsToken;
  statsTokens.set(tab.id, token);
  if (!sel) {
    tab.stats = tab.wholeStats;
    tab.statsPending = false;
    // Cancels any selection job still running in the backend.
    void computeStats(tab.id, null).catch(() => {});
    return;
  }
  tab.statsPending = true;
  try {
    const stats = await computeStats(tab.id, { start: sel.start, end: sel.end });
    if (statsTokens.get(tab.id) !== token || stats === null) return;
    tab.stats = stats;
    tab.statsPending = false;
  } catch (e) {
    if (statsTokens.get(tab.id) === token) {
      tab.statsPending = false;
      notify("error", "Could not compute statistics", String(e));
    }
  }
}

export function forgetStats(tabId: number) {
  statsTokens.delete(tabId);
}
