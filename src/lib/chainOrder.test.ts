import { describe, expect, it } from "vitest";
import type { ChainInfo, EffectInfo, GroupInfo } from "./api";
import { type ChainRow, chainRows, clickSelection, dropGap, dropTarget, groupOfSelection, stepTarget } from "./chainOrder";

const fx = (slotId: number): EffectInfo => ({
  slotId,
  classId: "0".repeat(32),
  name: `FX ${slotId}`,
  vendor: "V",
  hasEditor: true,
  bypassed: false,
});

const group = (groupId: number, slotIds: number[]): GroupInfo => ({ groupId, name: `Group ${groupId}`, bypassed: false, slotIds });

const chain = (slots: number[], groups: GroupInfo[] = []): ChainInfo => ({
  effects: slots.map(fx),
  bypassAll: false,
  latency: 0,
  groups,
});

/** EQ (1), then group 10 holding De-esser (2) and Compressor (3), then Reverb (4). */
const grouped = chain([1, 2, 3, 4], [group(10, [2, 3])]);

/** The row of effect `slot`, or of group `id`'s header. */
const effectRow = (rows: ChainRow[], slot: number) => rows.find((r) => r.kind === "effect" && r.effect.slotId === slot)!;
const headerRow = (rows: ChainRow[], id: number) => rows.find((r) => r.kind === "header" && r.group.groupId === id)!;

describe("chainRows", () => {
  it("puts a header before a group's effects, which share its top-level position", () => {
    const rows = chainRows(grouped, []);
    expect(rows.map((r) => [r.kind, r.kind === "effect" ? r.effect.slotId : r.group.groupId, r.top])).toEqual([
      ["effect", 1, 0],
      ["header", 10, 1],
      ["effect", 2, 1],
      ["effect", 3, 1],
      ["effect", 4, 2],
    ]);
    expect(effectRow(rows, 3)).toMatchObject({ member: 1 });
    expect(effectRow(rows, 4)).toMatchObject({ member: -1, group: null });
  });

  it("hides a collapsed group's effects", () => {
    const rows = chainRows(grouped, [10]);
    expect(rows.map((r) => r.kind)).toEqual(["effect", "header", "effect"]);
    expect(headerRow(rows, 10)).toMatchObject({ collapsed: true });
  });
});

describe("dropGap", () => {
  const mids = [10, 30, 50];

  it("counts the rows whose middle is above the pointer", () => {
    expect(dropGap(mids, 0)).toBe(0);
    expect(dropGap(mids, 20)).toBe(1);
    expect(dropGap(mids, 49)).toBe(2);
    expect(dropGap(mids, 99)).toBe(3);
  });
});

describe("dropTarget", () => {
  const top = (index: number) => ({ kind: "effect", to: { group: null, index } });

  describe("without groups", () => {
    const rows = chainRows(chain([1, 2, 3]), []);
    const drop = (slot: number, gap: number) => dropTarget(rows, effectRow(rows, slot), gap, false);

    it("drags Compressor above EQ", () => {
      expect(dropTarget(chainRows(chain([1, 2]), []), effectRow(chainRows(chain([1, 2]), []), 2), 0, false)).toEqual(top(0));
    });

    it("accounts for the effect leaving its place when moving later", () => {
      expect(drop(1, 2)).toEqual(top(1));
      expect(drop(1, 3)).toEqual(top(2));
      expect(drop(3, 1)).toEqual(top(1));
    });

    it("ignores drops that leave the effect where it is", () => {
      expect(drop(2, 1)).toBeNull();
      expect(drop(2, 2)).toBeNull();
    });

    it("clamps gaps outside the chain", () => {
      expect(drop(1, 10)).toEqual(top(2));
      expect(drop(3, -4)).toEqual(top(0));
    });
  });

  describe("with a group", () => {
    // Group 10 holding EQ (1) and Compressor (3), then Reverb (4).
    const rows = chainRows(chain([1, 3, 4], [group(10, [1, 3])]), []);
    const drop = (slot: number, gap: number, indented = false) => dropTarget(rows, effectRow(rows, slot), gap, indented);

    it("drags Reverb between the group's effects", () => {
      expect(drop(4, 2)).toEqual({ kind: "effect", to: { group: 10, index: 1 } });
    });

    it("joins the group just below its header", () => {
      expect(drop(4, 1)).toEqual({ kind: "effect", to: { group: 10, index: 0 } });
    });

    it("joins the group below its last effect only when indented", () => {
      expect(drop(4, 3, true)).toEqual({ kind: "effect", to: { group: 10, index: 2 } });
      // Not indented, Reverb is already just after the group.
      expect(drop(4, 3, false)).toBeNull();
    });

    it("takes an effect out of the group when dropped outside it", () => {
      expect(drop(3, 4)).toEqual(top(2));
      expect(drop(1, 0)).toEqual(top(0));
    });

    it("reorders within the group, counting after the effect is taken out", () => {
      expect(drop(1, 3, true)).toEqual({ kind: "effect", to: { group: 10, index: 1 } });
      expect(drop(3, 1)).toEqual({ kind: "effect", to: { group: 10, index: 0 } });
      expect(drop(1, 2)).toBeNull();
    });
  });

  describe("a group's header", () => {
    const rows = chainRows(grouped, []);
    const drop = (gap: number) => dropTarget(rows, headerRow(rows, 10), gap, false);

    it("moves the group above EQ", () => {
      expect(drop(0)).toEqual({ kind: "group", index: 0 });
    });

    it("moves the group to the end", () => {
      expect(drop(5)).toEqual({ kind: "group", index: 2 });
    });

    it("only drops between top-level items", () => {
      expect(drop(3)).toBeNull();
      expect(drop(2)).toBeNull();
      // Where it already is.
      expect(drop(1)).toBeNull();
      expect(drop(4)).toBeNull();
    });
  });

  it("passes a collapsed group instead of joining it", () => {
    const rows = chainRows(grouped, [10]);
    expect(dropTarget(rows, effectRow(rows, 1), 2, true)).toEqual(top(1));
    expect(dropTarget(rows, effectRow(rows, 4), 2, true)).toBeNull();
  });
});

describe("stepTarget", () => {
  const rows = chainRows(grouped, []);
  const step = (row: ChainRow, s: -1 | 1) => stepTarget(rows, row, s);

  it("moves Compressor out of the group with Alt+Down on its last effect", () => {
    expect(step(effectRow(rows, 3), 1)).toEqual({ kind: "effect", to: { group: null, index: 2 } });
    expect(step(effectRow(rows, 2), -1)).toEqual({ kind: "effect", to: { group: null, index: 1 } });
  });

  it("moves within the group elsewhere", () => {
    expect(step(effectRow(rows, 2), 1)).toEqual({ kind: "effect", to: { group: 10, index: 1 } });
    expect(step(effectRow(rows, 3), -1)).toEqual({ kind: "effect", to: { group: 10, index: 0 } });
  });

  it("moves a neighbouring effect into an expanded group at its end", () => {
    expect(step(effectRow(rows, 4), -1)).toEqual({ kind: "effect", to: { group: 10, index: 2 } });
    expect(step(effectRow(rows, 1), 1)).toEqual({ kind: "effect", to: { group: 10, index: 0 } });
  });

  it("passes a collapsed group", () => {
    const folded = chainRows(grouped, [10]);
    expect(stepTarget(folded, effectRow(folded, 4), -1)).toEqual({ kind: "effect", to: { group: null, index: 1 } });
  });

  it("moves a group past the item before or after it", () => {
    expect(step(headerRow(rows, 10), -1)).toEqual({ kind: "group", index: 0 });
    expect(step(headerRow(rows, 10), 1)).toEqual({ kind: "group", index: 2 });
  });

  it("stops at the ends", () => {
    expect(step(effectRow(rows, 1), -1)).toBeNull();
    expect(step(effectRow(rows, 4), 1)).toBeNull();
    const first = chainRows(chain([2, 3, 1], [group(10, [2, 3])]), []);
    expect(stepTarget(first, headerRow(first, 10), -1)).toBeNull();
  });
});

describe("clickSelection", () => {
  // EQ, Compressor, Reverb, Limiter.
  const order = [1, 2, 3, 4];

  it("selects a range with Shift+click", () => {
    const one = clickSelection(order, [], null, 1, "only");
    expect(one).toEqual({ selected: [1], anchor: 1 });
    expect(clickSelection(order, one.selected, one.anchor, 3, "range")).toEqual({ selected: [1, 2, 3], anchor: 1 });
    expect(clickSelection(order, [3], 3, 1, "range").selected).toEqual([1, 2, 3]);
  });

  it("toggles one with Ctrl/Cmd+click", () => {
    expect(clickSelection(order, [1, 2], 1, 2, "toggle")).toEqual({ selected: [1], anchor: 2 });
    expect(clickSelection(order, [1], 1, 4, "toggle").selected).toEqual([1, 4]);
  });

  it("selects just the clicked effect without an anchor to range from", () => {
    expect(clickSelection(order, [2], null, 3, "range")).toEqual({ selected: [3], anchor: 3 });
  });
});

describe("groupOfSelection", () => {
  it("finds the group holding a selected effect", () => {
    expect(groupOfSelection(grouped, [3])).toBe(10);
    expect(groupOfSelection(grouped, [1, 4])).toBeNull();
  });
});
