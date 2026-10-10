// Native application menu.
import { Menu, MenuItem, PredefinedMenuItem, Submenu } from "@tauri-apps/api/menu";
import { closeActiveTab, openDialog } from "./actions";
import { zoomIn, zoomOut, zoomToFit } from "./view.svelte";

let closeTabItem: MenuItem | null = null;
/** Wanted enabled state of Close Tab, applied once the menu exists. */
let closeTabEnabled = false;

/** Enable File › Close Tab only while a tab is open. */
export async function setCloseTabEnabled(enabled: boolean): Promise<void> {
  closeTabEnabled = enabled;
  await closeTabItem?.setEnabled(enabled);
}

export async function installMenu(): Promise<void> {
  const isMac = navigator.userAgent.includes("Mac");
  closeTabItem = await MenuItem.new({
    id: "close-tab",
    text: "Close Tab",
    accelerator: "CmdOrCtrl+W",
    enabled: closeTabEnabled,
    action: closeActiveTab,
  });
  const file = await Submenu.new({
    text: "File",
    items: [
      await MenuItem.new({ id: "open", text: "Open…", accelerator: "CmdOrCtrl+O", action: () => void openDialog() }),
      closeTabItem,
      await PredefinedMenuItem.new({ item: "Separator" }),
      await PredefinedMenuItem.new({ item: "Quit", text: "Quit" }),
    ],
  });
  // Zoom shortcuts (+ / − / F) are handled in the webview so plain keys stay usable in inputs.
  const viewMenu = await Submenu.new({
    text: "View",
    items: [
      await MenuItem.new({ id: "zoom-in", text: "Zoom In\t+", action: zoomIn }),
      await MenuItem.new({ id: "zoom-out", text: "Zoom Out\t−", action: zoomOut }),
      await MenuItem.new({ id: "zoom-fit", text: "Zoom to Fit\tF", action: zoomToFit }),
    ],
  });
  const items = [file, viewMenu];
  if (isMac) {
    items.unshift(
      await Submenu.new({
        text: "HitherDither",
        items: [
          await PredefinedMenuItem.new({ item: { About: null } }),
          await PredefinedMenuItem.new({ item: "Separator" }),
          await PredefinedMenuItem.new({ item: "Quit" }),
        ],
      }),
    );
  }
  const menu = await Menu.new({ items });
  await menu.setAsAppMenu();
  // The state may have changed while the menu was being built.
  await closeTabItem.setEnabled(closeTabEnabled);
}
