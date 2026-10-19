import { describe, expect, it } from "vitest";
import type { ChainInfo, EditOutcome, Metadata } from "./api";
import { showChainOutcome } from "./outcome";
import { newTab } from "./tabs.svelte";

const meta = { frames: 1_000, name: "a.wav" } as Metadata;

function outcome(chain: ChainInfo, dirty: boolean): EditOutcome {
  return {
    metadata: meta,
    edit_state: { can_undo: true, can_redo: false, dirty, edited: false, revision: 3 },
    clip: {
      untrimmed_frames: 1_000,
      trim_start: 0,
      trim_end: 1_000,
      fade_in: { len: 0, bend: 0 },
      fade_out: { len: 0, bend: 0 },
      joins: [],
    },
    chain,
    audioChanged: false,
  };
}

const eq = { slotId: 7, classId: "0".repeat(32), name: "EQ", vendor: "V", hasEditor: true, bypassed: false };

describe("showChainOutcome", () => {
  it("updates the chain and edit state, leaving peaks and analysis alone", () => {
    const tab = newTab("a.wav");
    Object.assign(tab, { meta, revision: 3, peaksVersion: 12, statsPending: true, analysing: true });
    const chain = { effects: [eq], bypassAll: false, latency: 64, groups: [] };

    showChainOutcome(tab, outcome(chain, true));

    expect(tab.chain).toEqual(chain);
    expect([tab.dirty, tab.canUndo, tab.canRedo]).toEqual([true, true, false]);
    expect([tab.revision, tab.peaksVersion, tab.statsPending, tab.analysing]).toEqual([3, 12, true, true]);
  });

  it("follows the backend's unsaved state when everything is bypassed", () => {
    const tab = newTab("a.wav");
    tab.dirty = true;
    showChainOutcome(tab, outcome({ effects: [{ ...eq, bypassed: true }], bypassAll: false, latency: 0, groups: [] }, false));
    expect(tab.dirty).toBe(false);
    expect(tab.peaksVersion).toBe(0);
    expect(tab.statsPending).toBe(false);
  });

  it("forgets selected effects and collapsed groups that left the chain", () => {
    const tab = newTab("a.wav");
    Object.assign(tab, { fxSelected: [7, 8], fxAnchor: 8, collapsedGroups: [1, 2] });
    const group = { groupId: 2, name: "G", bypassed: false, slotIds: [7] };
    showChainOutcome(tab, outcome({ effects: [eq], bypassAll: false, latency: 0, groups: [group] }, true));
    expect(tab.fxSelected).toEqual([7]);
    expect(tab.fxAnchor).toBeNull();
    expect(tab.collapsedGroups).toEqual([2]);
  });
});
