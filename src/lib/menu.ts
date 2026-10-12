// Native application menu.
import { Menu, MenuItem, PredefinedMenuItem, Submenu } from "@tauri-apps/api/menu";
import { closeActiveTab, openDialog } from "./actions";
import { applyEdit, fadeWithCurve, redo, save, saveAs, undo } from "./editActions";
import { zoomIn, zoomOut, zoomToFit } from "./view.svelte";

/** Menu items whose enabled state follows the app (see `setMenuEnabled`). */
export type ToggleId =
  | "close-tab"
  | "save"
  | "save-as"
  | "undo"
  | "redo"
  | "crop"
  | "delete"
  | "silence"
  | "fade-in"
  | "fade-out";

const items = new Map<ToggleId, MenuItem>();
/** Wanted enabled states, applied once the menu exists. */
const wanted = new Map<ToggleId, boolean>();

/** Enable or disable menu items, e.g. Close Tab only while a tab is open. */
export async function setMenuEnabled(states: Partial<Record<ToggleId, boolean>>): Promise<void> {
  const changes: Promise<void>[] = [];
  for (const [id, enabled] of Object.entries(states) as [ToggleId, boolean][]) {
    if (wanted.get(id) === enabled) continue;
    wanted.set(id, enabled);
    const item = items.get(id);
    if (item) changes.push(item.setEnabled(enabled));
  }
  await Promise.all(changes);
}

/** Quitting goes through the window's close request, which checks for unsaved changes. */
let quitHandler: () => void = () => {};
export function onQuit(handler: () => void) {
  quitHandler = handler;
}

async function toggle(id: ToggleId, text: string, action: () => void, accelerator?: string): Promise<MenuItem> {
  const item = await MenuItem.new({ id, text, accelerator, enabled: wanted.get(id) ?? false, action });
  items.set(id, item);
  return item;
}

export async function installMenu(): Promise<void> {
  const isMac = navigator.userAgent.includes("Mac");
  const quit = await MenuItem.new({ id: "quit", text: "Quit", accelerator: "CmdOrCtrl+Q", action: () => quitHandler() });
  const file = await Submenu.new({
    text: "File",
    items: [
      await MenuItem.new({ id: "open", text: "Open…", accelerator: "CmdOrCtrl+O", action: () => void openDialog() }),
      await toggle("save", "Save", () => void save(), "CmdOrCtrl+S"),
      await toggle("save-as", "Save As…", () => void saveAs(), "CmdOrCtrl+Shift+S"),
      await PredefinedMenuItem.new({ item: "Separator" }),
      await toggle("close-tab", "Close Tab", closeActiveTab, "CmdOrCtrl+W"),
      await PredefinedMenuItem.new({ item: "Separator" }),
      quit,
    ],
  });
  // Delete and the zoom shortcuts (+ / − / F) are handled in the webview so plain keys
  // stay usable in inputs.
  const edit = await Submenu.new({
    text: "Edit",
    items: [
      await toggle("undo", "Undo", () => void undo(), "CmdOrCtrl+Z"),
      await toggle("redo", "Redo", () => void redo(), "CmdOrCtrl+Shift+Z"),
      await PredefinedMenuItem.new({ item: "Separator" }),
      await toggle("crop", "Crop to Selection", () => void applyEdit("crop"), "CmdOrCtrl+T"),
      await toggle("delete", "Delete\tDel", () => void applyEdit("delete")),
      await toggle("silence", "Silence", () => void applyEdit("silence"), "CmdOrCtrl+L"),
      await toggle("fade-in", "Fade In…", () => void fadeWithCurve("fadeIn")),
      await toggle("fade-out", "Fade Out…", () => void fadeWithCurve("fadeOut")),
    ],
  });
  const viewMenu = await Submenu.new({
    text: "View",
    items: [
      await MenuItem.new({ id: "zoom-in", text: "Zoom In\t+", action: zoomIn }),
      await MenuItem.new({ id: "zoom-out", text: "Zoom Out\t−", action: zoomOut }),
      await MenuItem.new({ id: "zoom-fit", text: "Zoom to Fit\tF", action: zoomToFit }),
    ],
  });
  const menuItems = [file, edit, viewMenu];
  if (isMac) {
    menuItems.unshift(
      await Submenu.new({
        text: "HitherDither",
        items: [
          await PredefinedMenuItem.new({ item: { About: null } }),
          await PredefinedMenuItem.new({ item: "Separator" }),
          await MenuItem.new({ id: "quit-app", text: "Quit HitherDither", action: () => quitHandler() }),
        ],
      }),
    );
  }
  const menu = await Menu.new({ items: menuItems });
  await menu.setAsAppMenu();
  // The wanted states may have changed while the menu was being built.
  await Promise.all([...items].map(([id, item]) => item.setEnabled(wanted.get(id) ?? false)));
}
