import { describe, expect, it } from "vitest";
import { afterEdit, canApply } from "./editRules";

const sel = { start: 4_000, end: 5_000 };

describe("afterEdit", () => {
  it("crop clears the selection, rewinds and fits", () => {
    expect(afterEdit("crop", sel, 7_000, 10_000, 1_000)).toEqual({ selection: null, playhead: 0, fit: true });
  });

  it("delete clears the selection and puts the playhead at the join", () => {
    expect(afterEdit("delete", sel, 9_000, 10_000, 9_000)).toEqual({ selection: null, playhead: 4_000, fit: false });
  });

  it("silence and fades keep the selection and playhead", () => {
    for (const op of ["silence", "fadeIn", "fadeOut"] as const) {
      expect(afterEdit(op, sel, 1_234, 10_000, 10_000)).toEqual({ selection: sel, playhead: 1_234, fit: false });
    }
  });

  it("undo keeps the selection only if the length is unchanged", () => {
    expect(afterEdit("undo", sel, 100, 10_000, 10_000).selection).toEqual(sel);
    expect(afterEdit("undo", sel, 100, 9_000, 10_000).selection).toBeNull();
    expect(afterEdit("redo", sel, 100, 10_000, 9_000).selection).toBeNull();
  });

  it("keeps the playhead inside the new length", () => {
    expect(afterEdit("redo", null, 9_500, 10_000, 9_000).playhead).toBe(9_000);
  });
});

describe("canApply", () => {
  it("needs a non-empty selection", () => {
    expect(canApply("silence", null, 100)).toBe(false);
    expect(canApply("silence", { start: 5, end: 5 }, 100)).toBe(false);
    expect(canApply("fadeIn", { start: 5, end: 6 }, 100)).toBe(true);
  });

  it("refuses crop and delete of the whole file but allows the rest", () => {
    const whole = { start: 0, end: 100 };
    expect(canApply("crop", whole, 100)).toBe(false);
    expect(canApply("delete", whole, 100)).toBe(false);
    expect(canApply("silence", whole, 100)).toBe(true);
    expect(canApply("fadeOut", whole, 100)).toBe(true);
    expect(canApply("delete", { start: 0, end: 99 }, 100)).toBe(true);
  });
});
