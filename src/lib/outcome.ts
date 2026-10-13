// Applying a change's outcome to a tab, where it doesn't involve the waveform or analysis.
import type { EditOutcome, EditState } from "./api";
import type { Tab } from "./tabs.svelte";

export function setEditState(tab: Tab, state: EditState) {
  tab.dirty = state.dirty;
  tab.canUndo = state.can_undo;
  tab.canRedo = state.can_redo;
}

/**
 * Show an outcome that left the audio as it was (`audioChanged: false`): a chain change, or
 * undo or redo of one. The edit state and chain change; the revision, peaks, statistics and
 * analysis stay as they are.
 */
export function showChainOutcome(tab: Tab, outcome: EditOutcome) {
  tab.chain = outcome.chain;
  setEditState(tab, outcome.edit_state);
}
