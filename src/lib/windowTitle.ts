// The main window's title: the active tab's session, and whether it has unsaved changes.
import { SESSION_EXTENSION } from "./sessionFiles";
import type { Tab } from "./tabs.svelte";

export const APP_TITLE = "HitherDither";

/**
 * "mix — HitherDither" while the active tab is loaded and has the session `mix.hdsession.json`,
 * "mix • — HitherDither" while it also has unsaved changes, and "HitherDither" otherwise.
 */
export function windowTitle(tab: Pick<Tab, "status" | "sessionPath" | "dirty"> | null): string {
  if (!tab || tab.status !== "ready" || !tab.sessionPath) return APP_TITLE;
  const file = tab.sessionPath.split(/[\\/]/).pop() ?? tab.sessionPath;
  const name = file.toLowerCase().endsWith(SESSION_EXTENSION) ? file.slice(0, -SESSION_EXTENSION.length) : file;
  return `${name}${tab.dirty ? " •" : ""} — ${APP_TITLE}`;
}
