import { describe, expect, it } from "vitest";
import {
  formatBitDepth,
  formatBytes,
  formatDb,
  formatDuration,
  formatInt,
  formatPercent,
  formatRate,
  formatSeconds,
} from "./format";

describe("format", () => {
  it("formats durations as h:mm:ss.mmm", () => {
    expect(formatDuration(441_000 / 44_100)).toBe("0:00:10.000");
    expect(formatDuration(3723.4567)).toBe("1:02:03.457");
    expect(formatDuration(0.0004)).toBe("0:00:00.000");
    expect(formatDuration(59.9996)).toBe("0:01:00.000");
  });

  it("formats the 44.1 kHz / 16-bit / stereo scenario", () => {
    expect(formatRate(44_100)).toBe("44,100 Hz");
    expect(formatInt(441_000)).toBe("441,000");
    expect(formatBitDepth(16, 16, "PCM")).toBe("16-bit");
    expect(formatBitDepth(24, 20, "PCM")).toBe("24-bit (20 valid)");
  });

  it("formats sizes", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1_764_044)).toBe("1.68 MiB (1,764,044 bytes)");
  });

  it("formats levels with -inf for silence", () => {
    expect(formatDb(null)).toBe("-inf dBFS");
    expect(formatDb(-0.0001)).toBe("0.0 dBFS");
    expect(formatDb(-3.0103)).toBe("-3.0 dBFS");
    expect(formatDb(-18.04, "LUFS")).toBe("-18.0 LUFS");
    expect(formatPercent(-0.001)).toBe("0.00%");
    expect(formatSeconds(1.5)).toBe("1.500 s");
  });
});
