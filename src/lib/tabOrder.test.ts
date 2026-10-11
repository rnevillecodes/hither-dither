import { describe, expect, it } from "vitest";
import { cycle, move, neighbourAfterClose, samePath } from "./tabOrder";

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

describe("move", () => {
  it("moves the last tab to the front", () => {
    expect(move(["a", "b", "c"], "c", 0)).toEqual(["c", "a", "b"]);
  });

  it("moves the first tab to the back", () => {
    expect(move(["a", "b", "c"], "a", 3)).toEqual(["b", "c", "a"]);
  });

  it("moves a tab into a middle gap", () => {
    expect(move(["a", "b", "c", "d"], "a", 2)).toEqual(["b", "a", "c", "d"]);
    expect(move(["a", "b", "c", "d"], "d", 1)).toEqual(["a", "d", "b", "c"]);
  });

  it("leaves the order unchanged when dropped next to itself", () => {
    expect(move(["a", "b", "c"], "b", 1)).toEqual(["a", "b", "c"]);
    expect(move(["a", "b", "c"], "b", 2)).toEqual(["a", "b", "c"]);
  });

  it("ignores an unknown id", () => {
    expect(move(["a", "b"], "x", 0)).toEqual(["a", "b"]);
  });

  it("clamps an out-of-range gap", () => {
    expect(move(["a", "b", "c"], "b", -5)).toEqual(["b", "a", "c"]);
    expect(move(["a", "b", "c"], "b", 99)).toEqual(["a", "c", "b"]);
  });
});
