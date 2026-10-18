// The effect presets as last listed by the backend, replaced by the list each preset command
// returns, and the state of the preset dialogs.
import * as api from "./api";

/** What the preset name prompt is asking for. */
export type NamePrompt =
  | { kind: "save"; fileId: number; groupId: number; initial: string }
  | { kind: "rename"; presetId: number; initial: string };

export const presets = $state({
  list: [] as api.PresetInfo[],
  /** Manage Presets is open. */
  managerOpen: false,
  prompt: null as NamePrompt | null,
});

/** Fetch the preset list. */
export async function refreshPresets(): Promise<void> {
  try {
    presets.list = await api.listPresets();
  } catch {
    // Keep what is shown; the next change replaces it.
  }
}

/** The preset named `name`, ignoring case, other than preset `except`. */
export function presetNamed(name: string, except?: number): api.PresetInfo | undefined {
  const key = name.trim().toLowerCase();
  return presets.list.find((p) => p.id !== except && p.name.toLowerCase() === key);
}
