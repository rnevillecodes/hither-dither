import { describe, expect, it } from "vitest";
import type { PluginEntry } from "./api";
import { browse, matches, scanText } from "./effectBrowser";

const STEREO = 0x3;
const SURROUND_51 = 0x3f;
const stereo = { speaker_arrangement: STEREO, layout: "stereo" };
const surround = { speaker_arrangement: SURROUND_51, layout: "5.1" };

let next = 0;
function plugin(name: string, vendor: string, arrangements: number[], extra: Partial<PluginEntry> = {}): PluginEntry {
  return {
    path: `C:/VST3/${name}.vst3`,
    classId: (next++).toString(16).padStart(32, "0"),
    name,
    vendor,
    version: "1.0",
    subCategories: "Fx",
    hasEditor: true,
    layouts: arrangements.map((arrangement) => ({ channels: arrangement.toString(2).split("1").length - 1, arrangement })),
    status: "ready",
    reason: null,
    ...extra,
  };
}

const eq = plugin("Pro-Q 4", "FabFilter", [STEREO, SURROUND_51]);
const reverb = plugin("Supermassive", "Valhalla DSP", [STEREO]);
const comp = plugin("Pro-C 3", "FabFilter", [STEREO]);

describe("matches", () => {
  it("finds effects by name or vendor, ignoring case", () => {
    expect(matches(eq, "q")).toBe(true);
    expect(matches(eq, "fabfilter")).toBe(true);
    expect(matches(reverb, "eq")).toBe(false);
    expect(matches(eq, "")).toBe(true);
  });

  it("needs every word", () => {
    expect(matches(eq, "fab q")).toBe(true);
    expect(matches(eq, "fab reverb")).toBe(false);
  });
});

describe("browse", () => {
  it("groups by vendor, both in name order", () => {
    const groups = browse([reverb, eq, comp], "", stereo);
    expect(groups.map((g) => g.vendor)).toEqual(["FabFilter", "Valhalla DSP"]);
    expect(groups[0].entries.map((e) => e.name)).toEqual(["Pro-C 3", "Pro-Q 4"]);
  });

  it("filters by the search", () => {
    const groups = browse([reverb, eq, comp], "q 4", stereo);
    expect(groups.flatMap((g) => g.entries.map((e) => e.name))).toEqual(["Pro-Q 4"]);
  });

  it("greys out stereo-only effects on a 5.1 file, with the reason", () => {
    const entries = browse([reverb, eq], "", surround).flatMap((g) => g.entries);
    expect(entries.find((e) => e.name === "Supermassive")).toMatchObject({
      usable: false,
      reason: "Doesn't support 5.1 audio",
    });
    expect(entries.find((e) => e.name === "Pro-Q 4")).toMatchObject({ usable: true, reason: null });
  });

  it("leaves out failed, incompatible, disabled and effect-less plugins", () => {
    const hidden = [
      plugin("Crashy", "X", [], { status: "failed", reason: "Crashed while being scanned" }),
      plugin("Intel Only", "X", [], { status: "incompatible", classId: null }),
      plugin("Turned Off", "X", [STEREO], { status: "disabled" }),
      plugin("Synth", "X", [], { status: "noEffects", classId: null }),
    ];
    expect(browse([...hidden, eq], "", stereo).flatMap((g) => g.entries.map((e) => e.name))).toEqual(["Pro-Q 4"]);
  });
});

describe("scanText", () => {
  it("shows progress only while scanning", () => {
    expect(scanText({ done: 12, total: 200 })).toBe("Scanning plugins… 12 of 200");
    expect(scanText({ done: 200, total: 200 })).toBeNull();
    expect(scanText(null)).toBeNull();
  });
});
