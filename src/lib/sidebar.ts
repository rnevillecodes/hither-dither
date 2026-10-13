// The tabbed sidebar beside the waveform: which view is shown, and whether it is open.

export type SidebarView = "metrics" | "effects";

export interface SidebarState {
  open: boolean;
  view: SidebarView;
}

export const SIDEBAR_VIEWS: readonly SidebarView[] = ["metrics", "effects"];

export const OPEN_KEY = "hither-dither.sidebarOpen";
export const VIEW_KEY = "hither-dither.sidebarView";
/** The details panel's show/hide choice from before the sidebar had views. */
export const LEGACY_OPEN_KEY = "hither-dither.detailsOpen";

/** Clicking a view's button: switch to it, or collapse the sidebar if it is already shown. */
export function clickView(state: SidebarState, view: SidebarView): SidebarState {
  if (state.open && state.view === view) return { open: false, view };
  return { open: true, view };
}

/** Show `view`, opening the sidebar if it is collapsed. */
export function revealView(_state: SidebarState, view: SidebarView): SidebarState {
  return { open: true, view };
}

function isView(value: string | null): value is SidebarView {
  return SIDEBAR_VIEWS.includes(value as SidebarView);
}

/** The remembered sidebar state, read with `get` (a storage lookup returning null when unset). */
export function readSidebarState(get: (key: string) => string | null): SidebarState {
  const stored = get(VIEW_KEY);
  const open = get(OPEN_KEY) ?? get(LEGACY_OPEN_KEY);
  return { open: open !== "false", view: isView(stored) ? stored : "metrics" };
}
