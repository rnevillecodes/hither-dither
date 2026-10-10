import { describe, expect, it } from "vitest";
import { MIN_LABEL_SPACING, formatTickTime, ticks } from "./ruler";
import { fit } from "./viewport";

describe("ruler", () => {
  it("labels hours for long files zoomed out", () => {
    const r = ticks(fit(48_000 * 3 * 3600, 1200), 48_000);
    expect(r.mode).toBe("time");
    const labels = r.ticks.filter((t) => t.label).map((t) => t.label);
    expect(labels).toContain("1:00:00");
  });

  it("labels spacing never closer than the minimum", () => {
    for (const fpp of [0.1, 1, 7, 100, 10_000, 1e6]) {
      const r = ticks({ start: 12_345, fpp, width: 1000, totalFrames: 1e10 }, 44_100);
      const xs = r.ticks.filter((t) => t.label).map((t) => t.x);
      for (let i = 1; i < xs.length; i++) {
        expect(xs[i] - xs[i - 1]).toBeGreaterThanOrEqual(MIN_LABEL_SPACING - 1e-6);
      }
      expect(xs.length).toBeGreaterThan(0);
    }
  });

  it("switches to sample indices when zoomed to individual samples", () => {
    const r = ticks({ start: 1000, fpp: 0.1, width: 1000, totalFrames: 1e6 }, 48_000);
    expect(r.mode).toBe("samples");
    const labels = r.ticks.filter((t) => t.label).map((t) => Number(t.label));
    expect(labels[0]).toBeGreaterThanOrEqual(1000);
    expect(labels.every((n) => Number.isInteger(n))).toBe(true);
    expect(labels[1] - labels[0]).toBe(10);
  });

  it("uses millisecond labels just above sample level", () => {
    const r = ticks({ start: 0, fpp: 1, width: 1000, totalFrames: 1e6 }, 48_000);
    expect(r.mode).toBe("time");
    expect(r.ticks.find((t) => t.label && t.label !== "0:00.000")?.label).toBe("0:00.002");
  });

  it("formats tick times", () => {
    expect(formatTickTime(0, 1)).toBe("0:00");
    expect(formatTickTime(75, 5)).toBe("1:15");
    expect(formatTickTime(3725, 60)).toBe("1:02:05");
    expect(formatTickTime(1.5, 0.5)).toBe("0:01.5");
    expect(formatTickTime(0.012, 0.002)).toBe("0:00.012");
  });
});
