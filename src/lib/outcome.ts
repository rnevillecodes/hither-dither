// Applying a change's outcome to a tab, where it doesn't involve the waveform or analysis.
import type { ChainInfo, EditOutcome, EditState } from "./api";
import type { Tab } from "./tabs.svelte";

export function setEditState(tab: Tab, state: EditState) {
  tab.dirty = state.dirty;
  tab.canUndo = state.can_undo;
  tab.canRedo = state.can_redo;
  tab.edited = state.edited;
}

/**
 * Show an outcome that left the audio as it was (`audioChanged: false`): a chain change, or
 * undo or redo of one. The edit state and chain change; the revision, peaks, statistics and
 * analysis stay as they are.
 */
export function showChainOutcome(tab: Tab, outcome: EditOutcome) {
  setChain(tab, outcome.chain);
  setEditState(tab, outcome.edit_state);
}

/** Show `chain` as the tab's, forgetting selected effects and collapsed groups no longer in it. */
export function setChain(tab: Tab, chain: ChainInfo | null) {
  tab.chain = chain;
  const slots = new Set(chain?.effects.map((e) => e.slotId));
  const groups = new Set(chain?.groups.map((g) => g.groupId));
  tab.fxSelected = tab.fxSelected.filter((s) => slots.has(s));
  if (tab.fxAnchor !== null && !slots.has(tab.fxAnchor)) tab.fxAnchor = null;
  tab.collapsedGroups = tab.collapsedGroups.filter((g) => groups.has(g));
}
