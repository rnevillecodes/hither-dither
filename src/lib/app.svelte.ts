// Shared application state and the file-loading flow.
import { type ClipRuns, type Metadata, type Stats, computeStats, getClipRuns, openFile } from "./api";
import { notify } from "./notices.svelte";
import type { Selection } from "./selection";

/** Loads shorter than this never show the progress indicator. */
export const PROGRESS_DELAY_MS = 500;

export const app = $state({
  meta: null as Metadata | null,
  clipRuns: null as ClipRuns | null,
  wholeStats: null as Stats | null,
  selection: null as Selection | null,
  /** Stats currently displayed (whole file or selection). */
  stats: null as Stats | null,
  statsPending: false,
  /** Set while a load runs; `visible` once it has lasted PROGRESS_DELAY_MS. */
  loading: null as { name: string; fraction: number; visible: boolean } | null,
  /** Incremented on every successful load so views can reset. */
  generation: 0,
});

let loadToken = 0;

export async function loadPath(path: string): Promise<void> {
  const token = ++loadToken;
  const name = path.split(/[\\/]/).pop() ?? path;
  app.loading = { name, fraction: 0, visible: false };
  const timer = setTimeout(() => {
    if (token === loadToken && app.loading) app.loading.visible = true;
  }, PROGRESS_DELAY_MS);
  try {
    const meta = await openFile(path, (p) => {
      if (token === loadToken && app.loading) {
        app.loading.fraction = p.totalFrames ? p.doneFrames / p.totalFrames : 1;
      }
    });
    if (token !== loadToken) return;
    const [whole, clipRuns] = await Promise.all([computeStats(null), getClipRuns()]);
    app.meta = meta;
    app.wholeStats = whole;
    app.stats = whole;
    app.clipRuns = clipRuns;
    app.selection = null;
    app.statsPending = false;
    app.generation++;
    if (meta.truncated) {
      notify(
        "warning",
        `“${meta.name}” appears truncated`,
        "The data chunk is shorter than its header declares. The audio that is present was loaded.",
      );
    }
  } catch (e) {
    if (token !== loadToken) return;
    const msg = String(e);
    // A newer open superseded this one; nothing to report.
    if (!msg.endsWith("was cancelled")) notify("error", "Could not open file", msg);
  } finally {
    clearTimeout(timer);
    if (token === loadToken) app.loading = null;
  }
}

let statsToken = 0;

/** Set the selection and refresh stats for it (or restore whole-file stats). */
export async function setSelection(sel: Selection | null): Promise<void> {
  app.selection = sel;
  const token = ++statsToken;
  if (!sel) {
    app.stats = app.wholeStats;
    app.statsPending = false;
    // Cancels any selection job still running in the backend.
    void computeStats(null);
    return;
  }
  app.statsPending = true;
  try {
    const stats = await computeStats({ start: sel.start, end: sel.end });
    if (token !== statsToken || stats === null) return;
    app.stats = stats;
    app.statsPending = false;
  } catch (e) {
    if (token === statsToken) {
      app.statsPending = false;
      notify("error", "Could not compute statistics", String(e));
    }
  }
}
