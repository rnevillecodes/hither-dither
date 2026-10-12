import { describe, expect, it } from "vitest";
import { bendFromPoint, bendPointHeight, clampBend, gain } from "./curve";

/** Fade-in gain at t = 0, 1/4, 1/2, 3/4, 1 for each bend. Shared with edit/curve.rs. */
const TABLE: [number, number[]][] = [
  [-1, [0, 0.00390625, 0.0625, 0.31640625, 1]],
  [-0.5, [0, 0.0625, 0.25, 0.5625, 1]],
  [0, [0, 0.25, 0.5, 0.75, 1]],
  [0.5, [0, 0.5, 0.7071067811865476, 0.8660254037844386, 1]],
  [1, [0, 0.7071067811865476, 0.8408964152537145, 0.9306048591020996, 1]],
];

describe("gain", () => {
  it("matches the shared table, and fade-outs mirror fade-ins", () => {
    for (const [bend, gains] of TABLE) {
      gains.forEach((want, i) => {
        const t = i / 4;
        expect(gain("in", bend, t)).toBeCloseTo(want, 12);
        expect(gain("out", bend, 1 - t)).toBeCloseTo(want, 12);
      });
    }
  });

  it("clamps the position and the bend", () => {
    expect(gain("in", 0, -1)).toBe(0);
    expect(gain("in", 0, 2)).toBe(1);
    expect(clampBend(3)).toBe(1);
    expect(clampBend(Number.NaN)).toBe(0);
  });
});

describe("bend point", () => {
  it("round-trips between bend and height", () => {
    for (const bend of [-1, -0.3, 0, 0.42, 1]) {
      expect(bendFromPoint(bendPointHeight(bend))).toBeCloseTo(bend, 9);
    }
    expect(bendPointHeight(0)).toBe(0.5);
  });

  it("clamps heights outside the curve's range", () => {
    expect(bendFromPoint(0)).toBeCloseTo(-1, 12);
    expect(bendFromPoint(1)).toBeCloseTo(1, 12);
  });
});
