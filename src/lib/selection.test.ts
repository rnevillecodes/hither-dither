import { describe, expect, it } from "vitest";
import { between, extend, times } from "./selection";
import { frameIndexAtX } from "./viewport";

describe("selection", () => {
  it("normalises drag direction and rejects empty ranges", () => {
    expect(between(200, 100)).toEqual({ start: 100, end: 200 });
    expect(between(5, 5)).toBeNull();
  });

  it("extends the nearer edge", () => {
    const sel = { start: 100, end: 200 };
    expect(extend(sel, 250, 0)).toEqual({ start: 100, end: 250 });
    expect(extend(sel, 50, 0)).toEqual({ start: 50, end: 200 });
    expect(extend(sel, 120, 0)).toEqual({ start: 120, end: 200 });
    expect(extend(null, 300, 100)).toEqual({ start: 100, end: 300 });
  });

  it("drag from 1.0 s to 2.5 s reports start, end and duration", () => {
    const rate = 48_000;
    // 1 s per 100 px.
    const vp = { start: 0, fpp: rate / 100, width: 1000, totalFrames: rate * 10 };
    const sel = between(frameIndexAtX(vp, 100), frameIndexAtX(vp, 250))!;
    const t = times(sel, rate);
    expect(t.start).toBe(1);
    expect(t.end).toBe(2.5);
    expect(t.duration).toBe(1.5);
  });
});
