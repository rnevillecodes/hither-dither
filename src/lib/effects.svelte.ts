// Effect controls and dialogs: which plugin editors are open, the generic controls panel,
// the Add Effect browser and Manage Plugins.
import * as api from "./api";
import { notify } from "./notices.svelte";

/** An effect whose generic controls are showing. */
export interface GenericTarget {
  fileId: number;
  slotId: number;
  name: string;
  /** Why the plugin's own editor isn't showing, if it was asked for. */
  note: string | null;
}

export const effectsUi = $state({
  /** `fileId:slotId` of effects whose own editor window is open. */
  openEditors: [] as string[],
  generic: null as GenericTarget | null,
  browserOpen: false,
  managerOpen: false,
});

const key = (fileId: number, slotId: number) => `${fileId}:${slotId}`;

export function editorOpen(fileId: number, slotId: number): boolean {
  return effectsUi.openEditors.includes(key(fileId, slotId));
}

function forgetEditor(fileId: number, slotId: number) {
  const i = effectsUi.openEditors.indexOf(key(fileId, slotId));
  if (i >= 0) effectsUi.openEditors.splice(i, 1);
}

/**
 * Show an effect's controls: its own editor window, or the generic controls when it has
 * none that can be shown here (with the reason) or `generic` asks for them.
 */
export async function showControls(fileId: number, effect: api.EffectInfo, generic = false): Promise<void> {
  try {
    const opened = await api.openEditor(fileId, effect.slotId, generic);
    if (opened.native) {
      if (!editorOpen(fileId, effect.slotId)) effectsUi.openEditors.push(key(fileId, effect.slotId));
      return;
    }
    effectsUi.generic = { fileId, slotId: effect.slotId, name: effect.name, note: opened.reason ?? null };
  } catch (e) {
    notify("error", `Could not show the controls of ${effect.name}`, String(e));
  }
}

/** Close an effect's own editor window. */
export function closeControls(fileId: number, slotId: number) {
  void api.closeEditor(fileId, slotId).catch(() => {});
}

export function closeGeneric() {
  effectsUi.generic = null;
}

/** Follow editor windows closing in the backend (by the user, or as their effect went). */
export function initEffects() {
  void api.onEditorClosed(({ fileId, slotId }) => forgetEditor(fileId, slotId));
}
