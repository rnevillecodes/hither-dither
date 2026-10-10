import { describe, expect, it } from "vitest";
import { cycle, neighbourAfterClose, samePath } from "./tabOrder";

describe("neighbourAfterClose", () => {
  it("activates the tab to the right when closing the active middle tab", () => {
    expect(neighbourAfterClose(["a", "b", "c"], "b", "b")).toBe("c");
  });

  it("activates the tab to the left when closing the active last tab", () => {
    expect(neighbourAfterClose(["a", "b", "c"], "c", "c")).toBe("b");
  });

  it("leaves nothing active when closing the only tab", () => {
    expect(neighbourAfterClose(["a"], "a", "a")).toBeNull();
  });

  it("keeps the active tab when closing another one", () => {
    expect(neighbourAfterClose(["a", "b", "c"], "a", "c")).toBe("c");
  });
});

describe("cycle", () => {
  it("wraps from the last tab to the first", () => {
    expect(cycle([1, 2, 3], 3, 1)).toBe(1);
  });

  it("wraps from the first tab to the last", () => {
    expect(cycle([1, 2, 3], 1, -1)).toBe(3);
  });

  it("steps within the strip", () => {
    expect(cycle([1, 2, 3], 1, 1)).toBe(2);
    expect(cycle([1, 2, 3], 3, -1)).toBe(2);
  });

  it("handles no tabs and an unknown active tab", () => {
    expect(cycle([], null, 1)).toBeNull();
    expect(cycle([1, 2], null, 1)).toBe(1);
  });
});

describe("samePath", () => {
  it("ignores separator style and trailing slashes", () => {
    expect(samePath("C:\\audio\\a.wav", "C:/audio/a.wav", false)).toBe(true);
    expect(samePath("/x//y/a.wav", "/x/y/a.wav", false)).toBe(true);
  });

  it("is case-insensitive only when asked (Windows)", () => {
    expect(samePath("C:\\Audio\\A.WAV", "c:\\audio\\a.wav", true)).toBe(true);
    expect(samePath("/audio/A.wav", "/audio/a.wav", false)).toBe(false);
  });

  it("distinguishes different files", () => {
    expect(samePath("/audio/a.wav", "/audio/b.wav", true)).toBe(false);
  });
});
