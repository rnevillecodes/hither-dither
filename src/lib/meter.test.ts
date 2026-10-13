import { describe, expect, it } from "vitest";
import {
  atRest,
  clearReadouts,
  createMeter,
  dbfs,
  formatReadout,
  FLOOR_DB,
  HOLD_S,
  levelToFraction,
  SCALE_MARKS,
  stepBallistics,
  type ChannelMeter,
} from "./meter";

const FRAME = 1 / 60;

/** Linear amplitude of `db` dBFS. */
function lin(db: number): number {
  return 10 ** (db / 20);
}

/** Run `seconds` of animation frames with no new input. */
function idle(meter: ChannelMeter[], seconds: number, playing = false) {
  for (let t = 0; t < seconds - 1e-9; t += FRAME) stepBallistics(meter, null, FRAME, playing);
}

describe("scale", () => {
  it("maps the floor to 0 and full scale to 1, clamped", () => {
    expect(levelToFraction(FLOOR_DB)).toBe(0);
    expect(levelToFraction(-Infinity)).toBe(0);
    expect(levelToFraction(-90)).toBe(0);
    expect(levelToFraction(0)).toBe(1);
    expect(levelToFraction(3)).toBe(1);
    expect(levelToFraction(-30)).toBeCloseTo(0.5);
  });

  it("marks 0 to -60 dBFS", () => {
    expect(SCALE_MARKS).toEqual([0, -6, -12, -18, -24, -36, -60]);
  });

  it("converts linear levels to dBFS", () => {
    expect(dbfs(1)).toBe(0);
    expect(dbfs(0.5)).toBeCloseTo(-6.02, 2);
    expect(dbfs(0)).toBe(-Infinity);
  });
});

describe("stepBallistics", () => {
  it("rises at once and falls at 20 dB/s, emptying about 3 s after a -1 dBFS click", () => {
    const meter = createMeter(1);
    stepBallistics(meter, [[lin(-1), lin(-10)]], FRAME, true);
    expect(meter[0].peak).toBeCloseTo(-1);
    idle(meter, 1, true);
    expect(meter[0].peak).toBeCloseTo(-21, 0);
    idle(meter, 1.9, true);
    expect(meter[0].peak).toBeGreaterThan(FLOOR_DB);
    idle(meter, 0.1, true);
    expect(meter[0].peak).toBe(-Infinity);
  });

  it("holds the highest peak for 1.5 s, then lets it fall", () => {
    const meter = createMeter(1);
    stepBallistics(meter, [[lin(-1), 0]], FRAME, true);
    idle(meter, HOLD_S - 0.05, true);
    expect(meter[0].hold).toBeCloseTo(-1);
    idle(meter, 0.55, true);
    expect(meter[0].hold).toBeCloseTo(-11, 0);
  });

  it("raises the hold for a higher peak and restarts its time", () => {
    const meter = createMeter(1);
    stepBallistics(meter, [[lin(-12), 0]], FRAME, true);
    idle(meter, 1, true);
    stepBallistics(meter, [[lin(-6), 0]], FRAME, true);
    idle(meter, 1, true);
    expect(meter[0].hold).toBeCloseTo(-6);
  });

  it("keeps the RMS while playing between reports, and lets it fall once stopped", () => {
    const meter = createMeter(1);
    stepBallistics(meter, [[1, lin(-3)]], FRAME, true);
    idle(meter, 0.05, true);
    expect(meter[0].rms).toBeCloseTo(-3);
    idle(meter, 1, false);
    expect(meter[0].rms).toBeCloseTo(-23, 0);
  });

  it("lights the clip light at full scale but not at -0.5 dBFS, and it stays on", () => {
    const meter = createMeter(2);
    stepBallistics(meter, [[1, 0.5], [lin(-0.5), 0.5]], FRAME, true);
    expect(meter.map((m) => m.clip)).toEqual([true, false]);
    idle(meter, 5);
    expect(meter[0].clip).toBe(true);
  });

  it("keeps the readouts while the bars fall to rest", () => {
    const meter = createMeter(1);
    stepBallistics(meter, [[lin(-4.2), lin(-12)]], FRAME, true);
    expect(atRest(meter)).toBe(false);
    idle(meter, 6);
    expect(atRest(meter)).toBe(true);
    expect(formatReadout(meter[0].max)).toBe("-4.2");
  });

  it("clears the readouts and clip lights", () => {
    const meter = createMeter(1);
    stepBallistics(meter, [[1.2, 0.5]], FRAME, true);
    clearReadouts(meter);
    expect(meter[0].clip).toBe(false);
    expect(formatReadout(meter[0].max)).toBe("–∞");
  });
});

describe("formatReadout", () => {
  it("shows one decimal place, 0.0 near full scale and –∞ for silence", () => {
    expect(formatReadout(-4.24)).toBe("-4.2");
    expect(formatReadout(-0.01)).toBe("0.0");
    expect(formatReadout(1.5)).toBe("1.5");
    expect(formatReadout(-Infinity)).toBe("–∞");
  });
});
