import { describe, expect, it } from "vitest";
import { MAX_PRESET_NAME, validPresetName } from "./api";

describe("validPresetName", () => {
  it("trims the name", () => {
    expect(validPresetName("  Vocal cleanup ")).toBe("Vocal cleanup");
  });

  it("refuses an empty or blank name", () => {
    expect(validPresetName("")).toBeNull();
    expect(validPresetName("   \t")).toBeNull();
  });

  it("allows up to 64 characters, counted as the backend counts them", () => {
    expect(validPresetName("x".repeat(MAX_PRESET_NAME))).toBe("x".repeat(64));
    expect(validPresetName("x".repeat(MAX_PRESET_NAME + 1))).toBeNull();
    // Characters outside the BMP are one character each, not two UTF-16 units.
    expect(validPresetName("🎛".repeat(64))).toBe("🎛".repeat(64));
    expect(validPresetName("🎛".repeat(65))).toBeNull();
  });
});
