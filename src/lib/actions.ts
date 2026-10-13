// User-triggered actions shared by the menu, toolbar, tab strip, keyboard and drag-and-drop.
import { message, open } from "@tauri-apps/plugin-dialog";
import { cancelOpen, closeFile, readSession, setActiveFile } from "./api";
import { SessionMismatch, forgetStats, loadIntoTab, setSelection } from "./app.svelte";
import { confirmClose, watchAnalysis } from "./editActions";
import { notify } from "./notices.svelte";
import { type OpenRequest, isSessionPath, planOpens } from "./sessionFiles";
import { cycle, move, neighbourAfterClose, samePath } from "./tabOrder";
import { type Tab, activeTab, findTab, newTab, overviewCache, tabs } from "./tabs.svelte";
import { haltPlayback, transport } from "./transport.svelte";
import { view } from "./view.svelte";

let dialogOpen = false;

/** Show the native open dialog; cancelling leaves the open tabs untouched. */
export async function openDialog(): Promise<void> {
  // The menu accelerator and the webview key handler can both fire for Ctrl/Cmd+O.
  if (dialogOpen) return;
  dialogOpen = true;
  try {
    const paths = await open({
      multiple: true,
      directory: false,
      filters: [
        { name: "WAV audio and sessions", extensions: ["wav", "wave", "WAV", "hdsession.json"] },
        { name: "WAV audio", extensions: ["wav", "wave", "WAV"] },
        { name: "HitherDither sessions", extensions: ["hdsession.json"] },
      ],
    });
    // Don't wait for the loads: Open must stay usable while files are loading.
    if (paths) void openPaths(paths);
  } finally {
    dialogOpen = false;
  }
}

/** Open every dropped file, each in its own tab. */
export async function dropPaths(paths: string[]): Promise<void> {
  await openPaths(paths);
}

/** Loads run one at a time, in the order requested. */
let queue: Promise<void> = Promise.resolve();

/**
 * Open each path in its own tab: a WAV, or a session file, which opens its WAV and then
 * restores the session. A WAV that is already open (or loading) activates its tab instead.
 * Resolves once all of these loads have finished.
 */
export function openPaths(paths: string[]): Promise<void> {
  // Without sessions, tabs appear at once, as before.
  if (!paths.some(isSessionPath)) return openRequests(paths.map((path) => ({ kind: "wav", path })));
  return readRequests(paths).then(openRequests);
}

/**
 * Each path as a request, reading sessions one at a time: a session whose WAV is missing asks
 * for another, and only one such dialog is open at once.
 */
async function readRequests(paths: string[]): Promise<OpenRequest[]> {
  const requests: OpenRequest[] = [];
  for (const path of paths) {
    if (!isSessionPath(path)) {
      requests.push({ kind: "wav", path });
      continue;
    }
    try {
      const info = await readSession(path);
      if (info.audioPath !== null) {
        requests.push({ kind: "session", sessionPath: path, audioPath: info.audioPath, relink: false });
        continue;
      }
      const chosen = await chooseAnotherWav(path, `The audio file “${info.audioName}” could not be found.`);
      if (chosen) requests.push({ kind: "session", sessionPath: path, audioPath: chosen, relink: true });
    } catch (e) {
      requests.push({ kind: "invalid", sessionPath: path, message: String(e) });
    }
  }
  return requests;
}

/**
 * Tell the user why session `sessionPath` can't use its own WAV, and let them pick another.
 * Resolves the chosen WAV, or null if they cancelled.
 */
async function chooseAnotherWav(sessionPath: string, why: string): Promise<string | null> {
  const name = sessionPath.split(/[\\/]/).pop();
  const choice = await message(`${why}\n\nChoose another WAV to open “${name}” with it.`, {
    title: "Could not open session",
    kind: "warning",
    buttons: { ok: "Choose Another WAV…", cancel: "Cancel" },
  });
  if (choice !== "Choose Another WAV…") return null;
  const chosen = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "WAV audio", extensions: ["wav", "wave", "WAV"] }],
  });
  return typeof chosen === "string" ? chosen : null;
}

/** A session's WAV no longer matched it: offer another, and open the session with it. */
async function relinkMismatched(mismatch: SessionMismatch): Promise<void> {
  const chosen = await chooseAnotherWav(mismatch.sessionPath, `${mismatch.message} Its format or length changed.`);
  if (chosen) {
    await openRequests([{ kind: "session", sessionPath: mismatch.sessionPath, audioPath: chosen, relink: true }]);
  }
}

function openRequests(requests: OpenRequest[]): Promise<void> {
  const batch: Tab[] = [];
  const isOpen = (path: string) => tabs.list.some((t) => samePath(t.path, path));
  for (const action of planOpens(requests, isOpen)) {
    if (action.kind === "error") {
      notify("error", "Could not open session", action.message);
      continue;
    }
    if (action.kind === "activate") {
      const existing = tabs.list.find((t) => samePath(t.path, action.path));
      if (existing) activateTab(existing.id);
      if (existing && action.sessionPath) {
        notify(
          "warning",
          `“${existing.name}” is already open`,
          "Close its tab first to open the session, which would replace its edits and effects.",
        );
      }
      continue;
    }
    tabs.list.push(newTab(action.path, action.sessionPath, action.relink));
    // Use the reactive proxy from the list, not the plain object.
    batch.push(tabs.list[tabs.list.length - 1]);
  }
  if (batch.length === 0) return queue;
  const run = async () => {
    let lastReady: number | null = null;
    for (const tab of batch) {
      const result = await loadTab(tab);
      if (result === "ready") {
        lastReady = tab.id;
      } else if (result !== "closed") {
        const wasActive = tabs.activeId === tab.id;
        removeTab(tab.id);
        // The last file that loaded stays active, not whichever neighbour the close picked.
        if (wasActive && findTab(lastReady)) activateTab(lastReady);
        // Asked after this queue run, so the dialog doesn't hold up the other loads.
        if (result instanceof SessionMismatch) void Promise.resolve().then(() => relinkMismatched(result));
        else notify("error", tab.sessionPath ? "Could not open session" : "Could not open file", result);
      }
    }
  };
  queue = queue.then(run, run);
  return queue;
}

/**
 * Load one queued tab: "ready", "closed" (by the user, meanwhile), an error message, or a
 * session whose WAV no longer matches it.
 */
async function loadTab(tab: Tab): Promise<string | SessionMismatch> {
  if (!findTab(tab.id)) return "closed"; // closed while queued
  activateTab(tab.id);
  let error: string | SessionMismatch | null = null;
  try {
    await loadIntoTab(tab);
  } catch (e) {
    error = e instanceof SessionMismatch ? e : String(e);
  }
  if (!findTab(tab.id)) {
    // Closed meanwhile. Cancelling may have come too late to stop the backend
    // installing the file, so release it; there is nothing to report.
    void closeFile(tab.id);
    return "closed";
  }
  if (error !== null) return error;
  activateTab(tab.id);
  // A restored session's edits are still being analysed.
  if (tab.analysing) watchAnalysis(tab, tab.revision);
  return "ready";
}

/**
 * Make `id` the active tab: save the outgoing tab's viewport and playhead, stop
 * playback, and restore the incoming tab's saved state. `null` shows no tab.
 */
export function activateTab(id: number | null) {
  if (id === tabs.activeId) return;
  haltPlayback();
  const out = activeTab();
  if (out) {
    out.vp = view.vp;
    out.userScrolled = view.userScrolled;
    out.playhead = transport.playhead;
    out.playStart = transport.playStart;
  }
  tabs.activeId = id;
  // Effect editor windows follow their tab.
  void setActiveFile(id).catch(() => {});
  const tab = findTab(id);
  view.vp = tab?.vp ?? null;
  view.userScrolled = tab?.userScrolled ?? false;
  transport.playhead = tab?.playhead ?? 0;
  transport.playStart = tab?.playStart ?? 0;
  // Selection stats superseded by another tab's request while this one was inactive.
  if (tab?.selection && tab.statsPending) void setSelection(tab.selection);
}

/** Move to the next (`1`) or previous (`-1`) tab, wrapping around. */
export function cycleTab(step: 1 | -1) {
  const next = cycle(
    tabs.list.map((t) => t.id),
    tabs.activeId,
    step,
  );
  if (next !== null) activateTab(next);
}

/**
 * Move a tab into the gap before position `to` (0..length) of the tab strip. Only the
 * order changes: the active tab, playback and every tab's view state are left alone.
 */
export function moveTab(id: number, to: number) {
  const order = move(
    tabs.list.map((t) => t.id),
    id,
    to,
  );
  const byId = new Map(tabs.list.map((t) => [t.id, t]));
  tabs.list.splice(0, tabs.list.length, ...order.map((i) => byId.get(i)!));
}

/**
 * Close a tab, cancelling its load or releasing its file in the backend. A tab with
 * unsaved changes asks first and stays open if the user cancels (or saving fails).
 */
export async function closeTab(id: number): Promise<void> {
  let tab = findTab(id);
  if (!tab) return;
  if (tab.dirty) {
    if (!(await confirmClose(tab))) return;
    tab = findTab(id);
    if (!tab) return;
  }
  if (tab.status === "loading") void cancelOpen(id);
  else void closeFile(id);
  removeTab(id);
}

let lastCloseAt = -Infinity;

/** Close the active tab (File › Close Tab, Ctrl/Cmd+W). */
export function closeActiveTab() {
  // The menu accelerator and the webview key handler can both fire for one key press.
  const now = performance.now();
  if (now - lastCloseAt < 100) return;
  lastCloseAt = now;
  if (tabs.activeId !== null) void closeTab(tabs.activeId);
}

/**
 * Before quitting, ask about each tab with unsaved changes in tab order. Resolves false
 * if the user cancels (or a save fails), so the app keeps running.
 */
export async function confirmQuit(): Promise<boolean> {
  for (const tab of [...tabs.list]) {
    if (!tab.dirty || !findTab(tab.id)) continue;
    activateTab(tab.id);
    if (!(await confirmClose(tab))) return false;
  }
  return true;
}

function removeTab(id: number) {
  const ids = tabs.list.map((t) => t.id);
  const next = neighbourAfterClose(ids, id, tabs.activeId);
  if (id === tabs.activeId) {
    haltPlayback();
    // Nothing to save for a tab that is going away.
    tabs.activeId = null;
  }
  tabs.list.splice(ids.indexOf(id), 1);
  overviewCache.delete(id);
  forgetStats(id);
  activateTab(next);
}
