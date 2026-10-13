// Editing actions shared by the Edit menu and keyboard shortcuts: edits, undo/redo, saving,
// exporting and replacing the audio.
import { message, open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import * as api from "./api";
import { PROGRESS_DELAY_MS, forgetStats, setSelection, showEffectWarnings, showLoaded } from "./app.svelte";
import { askFadeCurve, curveDialog } from "./curveDialog.svelte";
import { type EditKind, afterEdit, canApply, remapFrame, replaceNeedsConfirmation } from "./editRules";
import { revealSidebarView } from "./layout.svelte";
import { notify } from "./notices.svelte";
import { setEditState, showChainOutcome } from "./outcome";
import { closePrompt, defaultExportPath, defaultSessionPath, withSessionExtension } from "./sessionFiles";
import { samePath } from "./tabOrder";
import { type Tab, activeTab, findTab, overviewCache, tabs } from "./tabs.svelte";
import { prefetchPeaks } from "./waveform/prefetch";
import { haltPlayback, playback, transport } from "./transport.svelte";
import { view } from "./view.svelte";
import { clamp, fit } from "./viewport";

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

/** The active tab, if it can be edited now (loaded, not saving, no dialog open). */
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
  if (!tab || !canApply(op, tab.selection, tab.meta!.frames)) return Promise.resolve();
  return run(tab, op, (sel) =>
    sel && canApply(op, sel, tab.meta!.frames) ? api.applyEdit(tab.id, op, sel.start, sel.end, bend) : null,
  );
}

/** Edit › Fade In / Fade Out: ask for the curve (heard while playing), then fade with it. */
export async function fadeWithCurve(op: "fadeIn" | "fadeOut"): Promise<void> {
  const tab = editableTab();
  const sel = tab?.selection;
  if (!tab || !sel || !canEdit(op)) return;
  const bend = await askFadeCurve(op === "fadeIn" ? "in" : "out", tab.id, sel);
  try {
    if (bend !== null) await applyEdit(op, bend);
  } finally {
    // The applied fade was drawn until now; the edited peaks show it from here.
    curveDialog.applied = null;
  }
}

/** Commit a trim / end-fade handle drag on the active tab. */
export function setClip(clip: api.ClipInfo): Promise<void> {
  const tab = editableTab();
  if (!tab) return Promise.resolve();
  return run(tab, "clip", () => api.setClip(tab.id, clip));
}

/** Commit a crossfade handle drag on the active tab. */
export function setCrossfade(join: number, xfade: api.FadeSetting): Promise<void> {
  const tab = editableTab();
  if (!tab) return Promise.resolve();
  return run(tab, "clip", () => api.setCrossfade(tab.id, join, xfade));
}

export function undo(): Promise<void> {
  const tab = editableTab();
  if (!tab?.canUndo) return Promise.resolve();
  return run(tab, "undo", () => (tab.canUndo ? api.undoEdit(tab.id) : null));
}

export function redo(): Promise<void> {
  const tab = editableTab();
  if (!tab?.canRedo) return Promise.resolve();
  return run(tab, "redo", () => (tab.canRedo ? api.redoEdit(tab.id) : null));
}

/** Peaks versions, increasing across tabs, so a newer tile always wins over an older one. */
let lastPeaksVersion = 0;
function nextPeaksVersion(): number {
  return ++lastPeaksVersion;
}

/** Changes per tab, chained so each starts from the outcome of the one before. */
const queues = new Map<number, Promise<void>>();

function enqueue(tab: Tab, job: () => Promise<void>): Promise<void> {
  const next = (queues.get(tab.id) ?? Promise.resolve()).then(job);
  queues.set(tab.id, next);
  void next.finally(() => {
    if (queues.get(tab.id) === next) queues.delete(tab.id);
  });
  return next;
}

/**
 * Run a change on `tab` and show its result straight away. `call` gets the selection as
 * it is when the change runs (after any queued before it), and returns null if the change
 * no longer applies. The edited audio is analysed in the background (see `watchAnalysis`).
 */
function run(
  tab: Tab,
  kind: EditKind,
  call: (selection: Tab["selection"]) => Promise<api.EditOutcome> | null,
): Promise<void> {
  return enqueue(tab, async () => {
    if (!findTab(tab.id) || tab.editing || !tab.meta) return;
    const selection = tab.selection;
    const oldFrames = tab.meta.frames;
    const oldClip = tab.clip;
    try {
      const pending = call(selection);
      if (!pending) return;
      const outcome = await pending;
      if (!findTab(tab.id)) return;
      // Undoing or redoing a chain change: the audio, and everything drawn from it, stay.
      if (!outcome.audioChanged) return showChainOutcome(tab, outcome);
      const after = afterEdit(kind, selection, playheadOf(tab), oldFrames, outcome.metadata.frames);
      await showOutcome(tab, kind, selection, oldClip, outcome, after);
    } catch (e) {
      // Closing the tab cancels the edit; there is nothing to report.
      if (findTab(tab.id)) notify("error", TITLES[kind], String(e));
    }
  });
}

/**
 * Run a change to `tab`'s effect chain, in turn with its edits. Chain changes leave the
 * audio, peaks and analysis alone. Resolves the outcome, or null if it failed (with a
 * notice titled `title`) or no longer applies.
 */
function runChain(
  tab: Tab,
  title: string,
  call: () => Promise<api.EditOutcome>,
): Promise<api.EditOutcome | null> {
  let result: api.EditOutcome | null = null;
  return enqueue(tab, async () => {
    if (!findTab(tab.id) || tab.editing) return;
    try {
      const outcome = await call();
      if (!findTab(tab.id)) return;
      showChainOutcome(tab, outcome);
      result = outcome;
    } catch (e) {
      if (findTab(tab.id)) notify("error", title, String(e));
    }
  }).then(() => result);
}

/** The active tab, if its effects can be changed now. */
function effectsTab(): Tab | null {
  const tab = editableTab();
  return tab?.chain ? tab : null;
}

export function canAddEffect(): boolean {
  const chain = effectsTab()?.chain;
  return !!chain && chain.effects.length < api.MAX_EFFECTS;
}

export function canBypassAll(): boolean {
  return !!effectsTab()?.chain?.effects.length;
}

/** Add effect `classId` to the end of the active tab's chain; resolves the new effect. */
export async function addEffect(classId: string): Promise<api.EffectInfo | null> {
  const tab = effectsTab();
  if (!tab || !canAddEffect()) return null;
  const outcome = await runChain(tab, "Could not add the effect", () => api.addEffect(tab.id, classId));
  // However it was added, show the new effect in the sidebar.
  if (outcome) revealSidebarView("effects");
  return outcome?.chain.effects.at(-1) ?? null;
}

export async function removeEffect(slotId: number): Promise<void> {
  const tab = effectsTab();
  if (tab) await runChain(tab, "Could not remove the effect", () => api.removeEffect(tab.id, slotId));
}

/** Move an effect of the active tab to position `index` of its chain. */
export async function moveEffect(slotId: number, index: number): Promise<void> {
  const tab = effectsTab();
  if (tab) await runChain(tab, "Could not move the effect", () => api.moveEffect(tab.id, slotId, index));
}

export async function setEffectBypass(slotId: number, on: boolean): Promise<void> {
  const tab = effectsTab();
  if (tab) await runChain(tab, "Could not bypass the effect", () => api.setEffectBypass(tab.id, slotId, on));
}

export async function setBypassAll(on: boolean): Promise<void> {
  const tab = effectsTab();
  if (tab) await runChain(tab, "Could not bypass the effects", () => api.setBypassAll(tab.id, on));
}

/** Whether Bypass All is on for the active tab. */
export function bypassAllOn(): boolean {
  return !!activeTab()?.chain?.bypassAll;
}

/** Effects › Bypass All Effects. */
export function toggleBypassAll(): Promise<void> {
  return canBypassAll() ? setBypassAll(!bypassAllOn()) : Promise.resolve();
}

/** The tab's playhead: live in `transport` while active, saved on the tab otherwise. */
function playheadOf(tab: Tab): number {
  return tabs.activeId === tab.id ? transport.playhead : tab.playhead;
}

/** Show the audio `outcome` describes in `tab`: metadata, clip markers, view and playback. */
async function showOutcome(
  tab: Tab,
  kind: EditKind,
  selection: Tab["selection"],
  oldClip: api.ClipInfo | null,
  outcome: api.EditOutcome,
  after: { selection: Tab["selection"]; playhead: number; fit: boolean },
): Promise<void> {
  const frames = outcome.metadata.frames;
  const active = tabs.activeId === tab.id;
  const vp = active ? view.vp : tab.vp;
  const nextVp = !vp ? null : after.fit ? fit(frames, vp.width) : clamp({ ...vp, totalFrames: frames });
  const version = nextPeaksVersion();
  // Markers in changed audio go at once; exact ones follow with the analysis. The waveform
  // gets its tile for the result first, so it changes over in one go.
  const [clipRuns] = await Promise.all([
    api.getClipRuns(tab.id).catch(() => null),
    active ? prefetchPeaks(tab.id, nextVp, version) : null,
  ]);
  if (!findTab(tab.id)) return;
  tab.meta = outcome.metadata;
  if (clipRuns) tab.clipRuns = clipRuns;
  tab.clip = outcome.clip;
  tab.chain = outcome.chain;
  setEditState(tab, outcome.edit_state);
  tab.revision = outcome.edit_state.revision;
  tab.peaksVersion = version;

  if (active) {
    view.vp = nextVp;
    if (transport.playing) {
      // The playhead follows playback, which follows the edit. Stop returns to the same
      // audio as before, or the join that replaced it.
      transport.playStart = remapFrame(kind, selection, transport.playStart, frames, oldClip, outcome.clip);
      const changed = after.selection?.start !== selection?.start || after.selection?.end !== selection?.end;
      if (playback.fromSelection && changed) {
        // The selection being played was cleared or changed: play what is selected now.
        const range = after.selection ?? { start: 0, end: frames };
        playback.fromSelection = !!after.selection;
        void api.setPlayRange(range.start, range.end).catch(() => {});
      }
    } else {
      transport.playhead = after.playhead;
      transport.playStart = after.playhead;
    }
  } else {
    tab.vp = nextVp;
    tab.playhead = after.playhead;
    tab.playStart = after.playhead;
  }

  if (active) {
    void setSelection(after.selection);
  } else {
    // Selection stats are fetched when the tab is next activated.
    tab.selection = after.selection;
    tab.stats = tab.wholeStats;
    tab.statsPending = !!after.selection;
  }
  watchAnalysis(tab, outcome.edit_state.revision);
}

/**
 * Mark `tab`'s whole-file stats as updating until revision `revision` has been analysed,
 * then show the exact stats, clip markers and peaks (unless a newer change came first).
 */
export function watchAnalysis(tab: Tab, revision: number) {
  tab.analysing = true;
  void api
    .waitAnalysis(tab.id, revision)
    .then(async (done) => {
      if (!done) return;
      const [whole, clipRuns] = await Promise.all([api.computeStats(tab.id, null), api.getClipRuns(tab.id)]);
      if (findTab(tab.id) && tab.revision === revision && whole) await showAnalysis(tab, whole, clipRuns);
    })
    .catch(() => {});
}

/** Show exact whole-file results for `tab`'s current audio. */
async function showAnalysis(tab: Tab, whole: api.Stats, clipRuns: api.ClipRuns) {
  const version = nextPeaksVersion();
  if (tabs.activeId === tab.id) await prefetchPeaks(tab.id, view.vp, version);
  // A newer change may have been shown meanwhile.
  if (!findTab(tab.id) || tab.peaksVersion >= version) return;
  tab.wholeStats = whole;
  tab.clipRuns = clipRuns;
  tab.analysing = false;
  tab.peaksVersion = version;
  if (!tab.selection) tab.stats = whole;
  // Fetching the whole-file stats cancels a selection request still running: ask again.
  else if (tab.statsPending && tabs.activeId === tab.id) void setSelection(tab.selection);
}

/** File › Save is available: loaded, not exporting, and unsaved or without a session file yet. */
export function canSave(): boolean {
  const tab = editableTab();
  return !!tab && (tab.dirty || !tab.sessionPath);
}

/** File › Save As and Export are available: the active tab is loaded and not exporting. */
export function canSaveAs(): boolean {
  return !!editableTab();
}

export function canExport(): boolean {
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

/** File › Save: write the active tab's session file, or ask where the first time. */
export async function save(): Promise<boolean> {
  const tab = editableTab();
  if (!tab || duplicatePress()) return false;
  return saveTabSession(tab);
}

/** File › Save As: save the active tab's session to a new session file. */
export async function saveAs(): Promise<boolean> {
  const tab = editableTab();
  if (!tab || duplicatePress()) return false;
  return askSessionPath(tab);
}

/** Save `tab`'s session to its own session file, asking where if it has none. */
function saveTabSession(tab: Tab): Promise<boolean> {
  return tab.sessionPath ? writeSession(tab, tab.sessionPath, true) : askSessionPath(tab);
}

async function askSessionPath(tab: Tab): Promise<boolean> {
  const chosen = await saveDialog({
    title: "Save As",
    defaultPath: defaultSessionPath(tab),
    filters: [{ name: "HitherDither sessions", extensions: ["hdsession.json"] }],
  });
  if (!chosen || !findTab(tab.id)) return false;
  // The dialog has asked about replacing the name it was given, but not one completed here.
  const path = withSessionExtension(chosen);
  return writeSession(tab, path, path === chosen);
}

/**
 * Save `tab`'s session to `path`, which becomes the tab's session file. Unless `replace`,
 * an existing file is only replaced once the user agrees. Resolves whether it was saved.
 */
async function writeSession(tab: Tab, path: string, replace: boolean): Promise<boolean> {
  // Let changes already on their way land first, so the session has them.
  await queues.get(tab.id);
  if (!findTab(tab.id) || tab.editing) return false;
  try {
    const saved = await api.saveSession(tab.id, path, replace);
    if (!saved.written) {
      const name = path.split(/[\\/]/).pop();
      const choice = await message(`“${name}” already exists. Replace it?`, {
        title: "Save As",
        kind: "warning",
        buttons: { ok: "Replace", cancel: "Cancel" },
      });
      return choice === "Replace" && findTab(tab.id) !== null && writeSession(tab, path, true);
    }
    tab.sessionPath = path;
    setEditState(tab, saved.editState);
    return true;
  } catch (e) {
    if (findTab(tab.id)) notify("error", "Could not save", String(e));
    return false;
  }
}

/**
 * Ask whether to save `tab`'s unsaved changes before it closes. Saving writes its session
 * (asking where if it has none), never the WAV. Resolves true if it may close (saved, or
 * the user chose not to save), false to keep it open.
 */
export async function confirmClose(tab: Tab): Promise<boolean> {
  if (!tab.dirty) return true;
  const choice = await message(closePrompt(tab), {
    title: "Unsaved changes",
    kind: "warning",
    buttons: { yes: "Save", no: "Don't Save", cancel: "Cancel" },
  });
  if (choice === "Save") return saveTabSession(tab);
  return choice === "Don't Save";
}

/** File › Export: render the active tab's audio to a new WAV. The tab is left as it is. */
export async function exportAudio(): Promise<boolean> {
  const tab = editableTab();
  if (!tab || duplicatePress()) return false;
  const path = await saveDialog({
    title: "Export",
    defaultPath: defaultExportPath(tab.path),
    filters: [{ name: "WAV audio", extensions: ["wav"] }],
  });
  if (!path || !findTab(tab.id)) return false;
  if (samePath(path, tab.path)) {
    notify(
      "error",
      "Could not export",
      `“${tab.name}” is the audio being edited, so it can't be overwritten. Export to another name.`,
    );
    return false;
  }
  const other = tabs.list.find((t) => t.id !== tab.id && samePath(t.path, path));
  if (other) {
    notify("error", "Could not export", `“${other.name}” is open in another tab. Close it first, or choose another name.`);
    return false;
  }
  // The dialog has already asked about replacing an existing file.
  return exportTab(tab, path);
}

/**
 * Render `tab`'s audio to `path`. Shows progress once it has lasted a moment, and keeps the
 * tab from being edited meanwhile. Resolves whether it was exported.
 */
async function exportTab(tab: Tab, path: string): Promise<boolean> {
  if (tab.editing) return false;
  tab.editing = true;
  // Let changes already on their way land first.
  await queues.get(tab.id);
  if (tabs.activeId === tab.id) haltPlayback();
  tab.progress = { fraction: 0, visible: false };
  const timer = setTimeout(() => (tab.progress.visible = true), PROGRESS_DELAY_MS);
  try {
    const outcome = await api.exportFile(tab.id, path, (p) => {
      tab.progress.fraction = p.totalFrames ? p.doneFrames / p.totalFrames : 1;
    });
    if (findTab(tab.id) && outcome.clampedSamples > 0) {
      const n = outcome.clampedSamples;
      const name = path.split(/[\\/]/).pop();
      notify(
        "warning",
        `Some samples in “${name}” were clipped`,
        `The effects pushed ${n.toLocaleString()} ${n === 1 ? "sample" : "samples"} past full scale, so ${n === 1 ? "it was" : "they were"} clipped to fit the file's format.`,
      );
    }
    return true;
  } catch (e) {
    if (findTab(tab.id)) notify("error", "Could not export", String(e));
    return false;
  } finally {
    clearTimeout(timer);
    tab.editing = false;
    tab.progress.visible = false;
  }
}

/** File › Replace Audio is available: the active tab is loaded and not exporting. */
export function canReplaceAudio(): boolean {
  return !!editableTab();
}

/**
 * File › Replace Audio: swap the active tab's WAV for another, keeping its effects and their
 * settings and its session file. The edits and undo history are dropped, so it asks first
 * when there is anything to lose.
 */
export async function replaceAudio(): Promise<boolean> {
  const tab = editableTab();
  if (!tab || duplicatePress()) return false;
  const chosen = await openDialog({
    title: "Replace Audio",
    multiple: false,
    directory: false,
    filters: [{ name: "WAV audio", extensions: ["wav", "wave", "WAV"] }],
  });
  if (typeof chosen !== "string" || !findTab(tab.id)) return false;
  const title = "Could not replace the audio";
  if (samePath(chosen, tab.path)) {
    notify("error", title, `“${tab.name}” is already this tab's audio.`);
    return false;
  }
  const other = tabs.list.find((t) => t.id !== tab.id && samePath(t.path, chosen));
  if (other) {
    notify("error", title, `“${other.name}” is open in another tab. Close it first, or choose another file.`);
    return false;
  }
  if (replaceNeedsConfirmation(tab)) {
    const choice = await message(
      `Replace the audio of “${tab.name}”?\n\nIts edits and undo history will be discarded. Its effects and their settings are kept.`,
      { title: "Replace Audio", kind: "warning", buttons: { ok: "Replace", cancel: "Cancel" } },
    );
    if (choice !== "Replace" || !findTab(tab.id)) return false;
  }
  return replaceTabAudio(tab, chosen);
}

async function replaceTabAudio(tab: Tab, path: string): Promise<boolean> {
  if (tab.editing) return false;
  tab.editing = true;
  // Let changes already on their way land first.
  await queues.get(tab.id);
  if (tabs.activeId === tab.id) haltPlayback();
  const old = tab.meta;
  // Shown as loading (the waveform goes) and not editable until the new audio is in.
  tab.meta = null;
  tab.progress = { fraction: 0, visible: false };
  const timer = setTimeout(() => (tab.progress.visible = true), PROGRESS_DELAY_MS);
  try {
    const outcome = await api.replaceAudio(tab.id, path, (p) => {
      tab.progress.fraction = p.totalFrames ? p.doneFrames / p.totalFrames : 1;
    });
    if (!findTab(tab.id)) return true;
    // A different recording: nothing of the old audio's view, selection or caches applies.
    tab.path = outcome.metadata.path;
    tab.name = outcome.metadata.name;
    tab.selection = null;
    tab.statsPending = false;
    tab.vp = null;
    tab.userScrolled = false;
    tab.playhead = 0;
    tab.playStart = 0;
    tab.peaksVersion = nextPeaksVersion();
    overviewCache.delete(tab.id);
    forgetStats(tab.id);
    if (tabs.activeId === tab.id) {
      view.vp = null;
      view.userScrolled = false;
      transport.playhead = 0;
      transport.playStart = 0;
    }
    await showLoaded(tab, outcome.metadata, outcome.editState);
    showEffectWarnings(outcome.warnings);
    return true;
  } catch (e) {
    if (findTab(tab.id)) {
      tab.meta = old;
      notify("error", "Could not replace the audio", String(e));
    }
    return false;
  } finally {
    clearTimeout(timer);
    tab.editing = false;
    tab.progress.visible = false;
  }
}
