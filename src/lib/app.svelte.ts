// The active document (facade over the active tab), loading a file into a tab, and
// selection statistics.
import { computeStats, getChain, getClip, getClipRuns, openFile } from "./api";
import { notify } from "./notices.svelte";
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
    const meta = await openFile(tab.id, tab.path, (p) => {
      tab.progress.fraction = p.totalFrames ? p.doneFrames / p.totalFrames : 1;
    });
    const [whole, clipRuns, clip, chain] = await Promise.all([
      computeStats(tab.id, null),
      getClipRuns(tab.id),
      getClip(tab.id),
      getChain(tab.id),
    ]);
    tab.meta = meta;
    tab.clip = clip;
    tab.chain = chain;
    tab.wholeStats = whole;
    tab.stats = whole;
    tab.clipRuns = clipRuns;
    tab.status = "ready";
    if (meta.truncated) {
      notify(
        "warning",
        `“${meta.name}” appears truncated`,
        "The data chunk is shorter than its header declares. The audio that is present was loaded.",
      );
    }
  } finally {
    clearTimeout(timer);
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
