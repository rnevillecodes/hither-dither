// User-triggered actions shared by the menu, toolbar, keyboard and drag-and-drop.
import { open } from "@tauri-apps/plugin-dialog";
import { loadPath } from "./app.svelte";
import { notify } from "./notices.svelte";

let dialogOpen = false;

/** Show the native open dialog; cancelling leaves the current file untouched. */
export async function openDialog(): Promise<void> {
  // The menu accelerator and the webview key handler can both fire for Ctrl/Cmd+O.
  if (dialogOpen) return;
  dialogOpen = true;
  try {
    const path = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "WAV audio", extensions: ["wav", "wave", "WAV"] }],
    });
    if (typeof path === "string") await loadPath(path);
  } finally {
    dialogOpen = false;
  }
}

/** Load the first dropped file and explain if more than one was dropped. */
export async function dropPaths(paths: string[]): Promise<void> {
  if (paths.length === 0) return;
  if (paths.length > 1) {
    const name = paths[0].split(/[\\/]/).pop();
    notify("info", "Only one file can be opened at a time", `Opening “${name}”; the other ${paths.length - 1} dropped file(s) were ignored.`);
  }
  await loadPath(paths[0]);
}
