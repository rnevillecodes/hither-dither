import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { readSession, rememberedTabs, writeSession } from "./session";

const KEY = "hither-dither.session";
let store: Map<string, string>;

beforeEach(() => {
  store = new Map();
  vi.stubGlobal("localStorage", {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => void store.set(k, v),
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("session", () => {
  it("round-trips paths in order and the active path", () => {
    writeSession(["C:\\a.wav", "C:\\b.wav"], "C:\\a.wav");
    expect(readSession()).toEqual({ paths: ["C:\\a.wav", "C:\\b.wav"], active: "C:\\a.wav" });
  });

  it("is empty when nothing was saved", () => {
    expect(readSession()).toEqual({ paths: [], active: null });
  });

  it("is empty for malformed JSON", () => {
    store.set(KEY, "{not json");
    expect(readSession()).toEqual({ paths: [], active: null });
  });

  it("is empty for an unknown version", () => {
    store.set(KEY, JSON.stringify({ v: 2, paths: ["/a.wav"], active: "/a.wav" }));
    expect(readSession()).toEqual({ paths: [], active: null });
  });

  it("drops non-string paths and an active path that isn't listed", () => {
    store.set(KEY, JSON.stringify({ v: 1, paths: ["/a.wav", 3, null, ""], active: "/gone.wav" }));
    expect(readSession()).toEqual({ paths: ["/a.wav"], active: null });
  });

  it("copes with storage that throws", () => {
    vi.stubGlobal("localStorage", {
      getItem: () => {
        throw new Error("denied");
      },
      setItem: () => {
        throw new Error("denied");
      },
    });
    expect(() => writeSession(["/a.wav"], null)).not.toThrow();
    expect(readSession()).toEqual({ paths: [], active: null });
  });
});

describe("rememberedTabs", () => {
  it("remembers session tabs by their session file and plain tabs by their WAV", () => {
    const mix = { path: "/t/take.wav", sessionPath: "/t/mix.hdsession.json" };
    const plain = { path: "/t/b.wav", sessionPath: null };
    expect(rememberedTabs([mix, plain], mix)).toEqual({
      paths: ["/t/mix.hdsession.json", "/t/b.wav"],
      active: "/t/mix.hdsession.json",
    });
    expect(rememberedTabs([mix, plain], plain).active).toBe("/t/b.wav");
    expect(rememberedTabs([], null)).toEqual({ paths: [], active: null });
  });
});
