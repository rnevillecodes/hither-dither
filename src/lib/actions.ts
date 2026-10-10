// User-triggered actions shared by the menu, toolbar, tab strip, keyboard and drag-and-drop.
import { open } from "@tauri-apps/plugin-dialog";
import { cancelOpen, closeFile } from "./api";
import { forgetStats, loadIntoTab, setSelection } from "./app.svelte";
import { notify } from "./notices.svelte";
import { cycle, neighbourAfterClose, samePath } from "./tabOrder";
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
      filters: [{ name: "WAV audio", extensions: ["wav", "wave", "WAV"] }],
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
 * Open each path in its own tab. A path that is already open (or loading) activates
 * its tab instead. Resolves once all of these loads have finished.
 */
export function openPaths(paths: string[]): Promise<void> {
  const batch: Tab[] = [];
  for (const path of paths) {
    const existing = tabs.list.find((t) => samePath(t.path, path));
    if (existing) {
      activateTab(existing.id);
      continue;
    }
    tabs.list.push(newTab(path));
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
        notify("error", "Could not open file", result);
      }
    }
  };
  queue = queue.then(run, run);
  return queue;
}

/** Load one queued tab: "ready", "closed" (by the user, meanwhile) or an error message. */
async function loadTab(tab: Tab): Promise<string> {
  if (!findTab(tab.id)) return "closed"; // closed while queued
  activateTab(tab.id);
  let error: string | null = null;
  try {
    await loadIntoTab(tab);
  } catch (e) {
    error = String(e);
  }
  if (!findTab(tab.id)) {
    // Closed meanwhile. Cancelling may have come too late to stop the backend
    // installing the file, so release it; there is nothing to report.
    void closeFile(tab.id);
    return "closed";
  }
  if (error !== null) return error;
  activateTab(tab.id);
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

/** Close a tab, cancelling its load or releasing its file in the backend. */
export function closeTab(id: number) {
  const tab = findTab(id);
  if (!tab) return;
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
  if (tabs.activeId !== null) closeTab(tabs.activeId);
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
