import { describe, expect, it } from "vitest";
import { dropGap, dropIndex, stepIndex } from "./chainOrder";

describe("dropGap", () => {
  const mids = [10, 30, 50];

  it("counts the rows whose middle is above the pointer", () => {
    expect(dropGap(mids, 0)).toBe(0);
    expect(dropGap(mids, 20)).toBe(1);
    expect(dropGap(mids, 49)).toBe(2);
    expect(dropGap(mids, 99)).toBe(3);
  });
});

describe("dropIndex", () => {
  it("drags Compressor above EQ", () => {
    // EQ, Compressor: dropping the second into the first gap makes it first.
    expect(dropIndex(1, 0, 2)).toBe(0);
  });

  it("accounts for the effect leaving its place when moving later", () => {
    expect(dropIndex(0, 2, 3)).toBe(1);
    expect(dropIndex(0, 3, 3)).toBe(2);
    expect(dropIndex(2, 1, 3)).toBe(1);
  });

  it("ignores drops that leave the effect where it is", () => {
    expect(dropIndex(1, 1, 3)).toBeNull();
    expect(dropIndex(1, 2, 3)).toBeNull();
    expect(dropIndex(5, 0, 3)).toBeNull();
  });

  it("clamps gaps outside the chain", () => {
    expect(dropIndex(0, 10, 3)).toBe(2);
    expect(dropIndex(2, -4, 3)).toBe(0);
  });
});

describe("stepIndex", () => {
  it("moves the second effect first with Alt+Up", () => {
    expect(stepIndex(1, -1, 2)).toBe(0);
  });

  it("moves later with Alt+Down", () => {
    expect(stepIndex(0, 1, 3)).toBe(1);
  });

  it("stops at the ends", () => {
    expect(stepIndex(0, -1, 3)).toBeNull();
    expect(stepIndex(2, 1, 3)).toBeNull();
  });
});
