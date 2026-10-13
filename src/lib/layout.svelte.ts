// Window layout preferences, remembered across restarts.

import { clickView, OPEN_KEY, readSidebarState, revealView, VIEW_KEY, type SidebarState, type SidebarView } from "./sidebar";

function readStored(): SidebarState {
  try {
    return readSidebarState((key) => localStorage.getItem(key));
  } catch {
    return readSidebarState(() => null);
  }
}

const initial = readStored();

export const layout = $state({
  /** Whether the sidebar (beside the activity bar) is shown while files are open. */
  sidebarOpen: initial.open,
  /** The view the sidebar shows, and whose activity bar button is highlighted while open. */
  sidebarView: initial.view,
});

function apply(next: SidebarState) {
  layout.sidebarOpen = next.open;
  layout.sidebarView = next.view;
  try {
    localStorage.setItem(OPEN_KEY, String(next.open));
    localStorage.setItem(VIEW_KEY, next.view);
  } catch {
    // Storage unavailable: the choice lasts for this session only.
  }
}

function current(): SidebarState {
  return { open: layout.sidebarOpen, view: layout.sidebarView };
}

/** An activity bar button was clicked: switch to its view, or collapse if it is already shown. */
export function clickSidebarView(view: SidebarView) {
  apply(clickView(current(), view));
}

/** Show `view`, opening the sidebar if it is collapsed. */
export function revealSidebarView(view: SidebarView) {
  apply(revealView(current(), view));
}
