// Editing actions shared by the Edit menu and keyboard shortcuts: edits, undo/redo and saving.
import { message, save as saveDialog } from "@tauri-apps/plugin-dialog";
import * as api from "./api";
import { PROGRESS_DELAY_MS, setSelection } from "./app.svelte";
import { askFadeCurve, curveDialog } from "./curveDialog.svelte";
import { type EditKind, afterEdit, canApply } from "./editRules";
import { notify } from "./notices.svelte";
import { samePath } from "./tabOrder";
import { type Tab, activeTab, findTab, overviewCache, tabs } from "./tabs.svelte";
import { haltPlayback, transport } from "./transport.svelte";
import { view } from "./view.svelte";
import { clamp } from "./viewport";

const TITLES: Record<EditKind, string> = {
  crop: "Could not crop",
  delete: "Could not delete",
  silence: "Could not silence",
  fadeIn: "Could not fade in",
  fadeOut: "Could not fade out",
  undo: "Could not undo",
  clip: "Could not change the clip",
  redo: "Could not redo",
};

/** The active tab, if it can be edited now (loaded, not busy, no dialog open). */
function editableTab(): Tab | null {
  const tab = activeTab();
  return tab && tab.status === "ready" && tab.meta && !tab.editing && !curveDialog.open ? tab : null;
}

/** Whether Edit › `op` is available for the active tab. */
export function canEdit(op: api.EditOp): boolean {
  const tab = editableTab();
  return !!tab && canApply(op, tab.selection, tab.meta!.frames);
}

export function canUndo(): boolean {
  return !!editableTab()?.canUndo;
}

export function canRedo(): boolean {
  return !!editableTab()?.canRedo;
}

/** Apply `op` to the active tab's selection; `bend` shapes fades (0 = linear). */
export function applyEdit(op: api.EditOp, bend = 0): Promise<void> {
  const tab = editableTab();
  const sel = tab?.selection;
  if (!tab || !sel || !canApply(op, sel, tab.meta!.frames)) return Promise.resolve();
  return run(tab, op, sel, (p) => api.applyEdit(tab.id, op, sel.start, sel.end, bend, p));
}

/** Edit › Fade In / Fade Out: ask for the curve, then fade the selection with it. */
export async function fadeWithCurve(op: "fadeIn" | "fadeOut"): Promise<void> {
  if (!canEdit(op)) return;
  const bend = await askFadeCurve(op === "fadeIn" ? "in" : "out");
  if (bend !== null) await applyEdit(op, bend);
}

/** Commit a trim / end-fade handle drag on the active tab. */
export function setClip(clip: api.ClipInfo): Promise<void> {
  const tab = editableTab();
  if (!tab) return Promise.resolve();
  return run(tab, "clip", tab.selection, (p) => api.setClip(tab.id, clip, p));
}

/** Commit a crossfade handle drag on the active tab. */
export function setCrossfade(join: number, xfade: api.FadeSetting): Promise<void> {
  const tab = editableTab();
  if (!tab) return Promise.resolve();
  return run(tab, "clip", tab.selection, (p) => api.setCrossfade(tab.id, join, xfade, p));
}

export function undo(): Promise<void> {
  const tab = editableTab();
  if (!tab?.canUndo) return Promise.resolve();
  return run(tab, "undo", tab.selection, (p) => api.undoEdit(tab.id, p));
}

export function redo(): Promise<void> {
  const tab = editableTab();
  if (!tab?.canRedo) return Promise.resolve();
  return run(tab, "redo", tab.selection, (p) => api.redoEdit(tab.id, p));
}

/** Run a backend edit on `tab`, showing progress once it has lasted a moment. */
async function run(
  tab: Tab,
  kind: EditKind,
  selection: Tab["selection"],
  call: (onProgress: (p: api.LoadProgress) => void) => Promise<api.EditOutcome>,
): Promise<void> {
  if (tabs.activeId === tab.id) haltPlayback();
  tab.editing = true;
  tab.progress = { fraction: 0, visible: false };
  const timer = setTimeout(() => (tab.progress.visible = true), PROGRESS_DELAY_MS);
  const oldFrames = tab.meta!.frames;
  try {
    const outcome = await call((p) => {
      tab.progress.fraction = p.totalFrames ? p.doneFrames / p.totalFrames : 1;
    });
    if (!findTab(tab.id)) return;
    const after = afterEdit(kind, selection, playheadOf(tab), oldFrames, outcome.metadata.frames);
    await showOutcome(tab, outcome, after);
  } catch (e) {
    // Closing the tab cancels the edit; there is nothing to report.
    if (findTab(tab.id)) notify("error", TITLES[kind], String(e));
  } finally {
    clearTimeout(timer);
    tab.editing = false;
    tab.progress.visible = false;
  }
}

/** The tab's playhead: live in `transport` while active, saved on the tab otherwise. */
function playheadOf(tab: Tab): number {
  return tabs.activeId === tab.id ? transport.playhead : tab.playhead;
}

/** Show the audio `outcome` describes in `tab`: metadata, stats, clip markers and view. */
async function showOutcome(
  tab: Tab,
  outcome: api.EditOutcome,
  after: { selection: Tab["selection"]; playhead: number; fit: boolean },
): Promise<void> {
  const [whole, clipRuns] = await Promise.all([api.computeStats(tab.id, null), api.getClipRuns(tab.id)]);
  if (!findTab(tab.id)) return;
  const frames = outcome.metadata.frames;
  tab.meta = outcome.metadata;
  tab.wholeStats = whole;
  tab.clipRuns = clipRuns;
  tab.clip = outcome.clip;
  setEditState(tab, outcome.edit_state);

  const active = tabs.activeId === tab.id;
  const vp = active ? view.vp : tab.vp;
  const nextVp = after.fit || !vp ? null : clamp({ ...vp, totalFrames: frames });
  if (active) {
    view.vp = nextVp;
    transport.playhead = after.playhead;
    transport.playStart = after.playhead;
  } else {
    tab.vp = nextVp;
    tab.playhead = after.playhead;
    tab.playStart = after.playhead;
  }
  overviewCache.delete(tab.id);
  // Remounts the waveform, which picks up the viewport set above.
  tab.revision = outcome.edit_state.revision;

  if (active) {
    void setSelection(after.selection);
  } else {
    // Selection stats are fetched when the tab is next activated.
    tab.selection = after.selection;
    tab.stats = whole;
    tab.statsPending = !!after.selection;
  }
}

function setEditState(tab: Tab, state: api.EditState) {
  tab.dirty = state.dirty;
  tab.canUndo = state.can_undo;
  tab.canRedo = state.can_redo;
}

export function canSave(): boolean {
  return !!editableTab()?.dirty;
}

export function canSaveAs(): boolean {
  return !!editableTab();
}

/** The menu accelerator and the webview key handler can both fire for one key press. */
let lastSaveAt = -Infinity;
function duplicatePress(): boolean {
  const now = performance.now();
  const dup = now - lastSaveAt < 300;
  lastSaveAt = now;
  return dup;
}

/** File › Save: overwrite the active tab's file, confirming the first time. */
export async function save(): Promise<boolean> {
  const tab = editableTab();
  if (!tab?.dirty || duplicatePress()) return false;
  if (!tab.overwriteConfirmed) {
    const choice = await message(
      `Overwrite “${tab.name}”?

Its original audio will be replaced and can't be recovered.`,
      { title: "Save", kind: "warning", buttons: { ok: "Overwrite", cancel: "Cancel" } },
    );
    if (choice !== "Overwrite") return false;
    tab.overwriteConfirmed = true;
  }
  return saveTab(tab, null);
}

/** File › Save As: write the active tab's audio to a new file and switch the tab to it. */
export async function saveAs(): Promise<boolean> {
  const tab = editableTab();
  if (!tab || duplicatePress()) return false;
  const path = await saveDialog({
    title: "Save As",
    defaultPath: tab.path,
    filters: [{ name: "WAV audio", extensions: ["wav"] }],
  });
  if (!path || !findTab(tab.id)) return false;
  const itself = samePath(path, tab.path);
  const other = tabs.list.find((t) => t.id !== tab.id && samePath(t.path, path));
  if (other) {
    notify("error", "Could not save", `“${other.name}” is open in another tab. Close it first, or choose another name.`);
    return false;
  }
  // The save dialog has already asked about replacing an existing file.
  const ok = await saveTab(tab, itself ? null : path);
  if (ok) tab.overwriteConfirmed = true;
  return ok;
}

/**
 * Ask whether to save `tab`'s unsaved changes before it closes. Resolves true if it may
 * close (saved, or the user chose not to save), false to keep it open.
 */
export async function confirmClose(tab: Tab): Promise<boolean> {
  if (!tab.dirty) return true;
  const choice = await message(`Save changes to “${tab.name}” before closing?`, {
    title: "Unsaved changes",
    kind: "warning",
    buttons: { yes: "Save", no: "Don't Save", cancel: "Cancel" },
  });
  if (choice === "Save") return saveTab(tab, null);
  return choice === "Don't Save";
}

/** Write `tab` to `path` (its own file when null). Resolves whether it was saved. */
async function saveTab(tab: Tab, path: string | null): Promise<boolean> {
  if (tab.editing) return false;
  if (tabs.activeId === tab.id) haltPlayback();
  tab.editing = true;
  try {
    const outcome = await api.saveFile(tab.id, path);
    if (!findTab(tab.id)) return true;
    // Same audio and analysis, so the waveform and stats stay as they are.
    tab.meta = outcome.metadata;
    tab.path = outcome.metadata.path;
    tab.name = outcome.metadata.name;
    tab.clip = outcome.clip;
    setEditState(tab, outcome.edit_state);
    return true;
  } catch (e) {
    if (findTab(tab.id)) notify("error", "Could not save", String(e));
    return false;
  } finally {
    tab.editing = false;
  }
}
