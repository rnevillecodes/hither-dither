import { describe, expect, it } from "vitest";
import { defaultSessionPath, isSessionPath, planOpens, withSessionExtension } from "./sessionFiles";

describe("planOpens", () => {
  const none = () => false;

  it("keeps the requested order for a mixed selection", () => {
    const plan = planOpens(
      [
        { kind: "wav", path: "/t/a.wav" },
        { kind: "session", sessionPath: "/t/mix.hdsession.json", audioPath: "/t/take.wav" },
        { kind: "wav", path: "/t/b.wav" },
      ],
      none,
    );
    expect(plan).toEqual([
      { kind: "open", path: "/t/a.wav", sessionPath: null },
      { kind: "open", path: "/t/take.wav", sessionPath: "/t/mix.hdsession.json" },
      { kind: "open", path: "/t/b.wav", sessionPath: null },
    ]);
  });

  it("activates a WAV that is already open instead of applying the session", () => {
    const plan = planOpens(
      [{ kind: "session", sessionPath: "/t/mix.hdsession.json", audioPath: "/t/take.wav" }],
      (p) => p === "/t/take.wav",
    );
    expect(plan).toEqual([{ kind: "activate", path: "/t/take.wav", sessionPath: "/t/mix.hdsession.json" }]);
  });

  it("opens a WAV once when it is requested twice in one batch", () => {
    const plan = planOpens(
      [
        { kind: "wav", path: "C:\\t\\take.wav" },
        { kind: "session", sessionPath: "C:\\t\\mix.hdsession.json", audioPath: "C:\\t\\take.wav" },
      ],
      none,
    );
    expect(plan.map((a) => a.kind)).toEqual(["open", "activate"]);
  });

  it("reports sessions that couldn't be read in their place", () => {
    const plan = planOpens(
      [
        { kind: "invalid", sessionPath: "/t/x.hdsession.json", message: "not valid" },
        { kind: "wav", path: "/t/a.wav" },
      ],
      none,
    );
    expect(plan[0]).toEqual({ kind: "error", sessionPath: "/t/x.hdsession.json", message: "not valid" });
    expect(plan[1].kind).toBe("open");
  });
});

describe("isSessionPath", () => {
  it("matches the full extension, ignoring case", () => {
    expect(isSessionPath("C:\\Takes\\x.hdsession.json")).toBe(true);
    expect(isSessionPath("/takes/X.HDSESSION.JSON")).toBe(true);
  });

  it("doesn't match other files", () => {
    expect(isSessionPath("notes.json")).toBe(false);
    expect(isSessionPath("x.hdsession")).toBe(false);
    expect(isSessionPath("x.wav")).toBe(false);
    expect(isSessionPath("/takes/.hdsession.json")).toBe(false);
  });
});

describe("defaultSessionPath", () => {
  it("proposes the tab's own session file", () => {
    expect(defaultSessionPath({ path: "/t/take.wav", sessionPath: "/s/mix.hdsession.json" })).toBe(
      "/s/mix.hdsession.json",
    );
  });

  it("otherwise sits next to the WAV with its name", () => {
    expect(defaultSessionPath({ path: "C:\\Takes\\take.wav", sessionPath: null })).toBe(
      "C:\\Takes\\take.hdsession.json",
    );
    expect(defaultSessionPath({ path: "/t/Take.WAVE", sessionPath: null })).toBe("/t/Take.hdsession.json");
  });
});

describe("withSessionExtension", () => {
  it("completes a name typed without the full extension", () => {
    expect(withSessionExtension("/t/mix")).toBe("/t/mix.hdsession.json");
    expect(withSessionExtension("/t/mix.json")).toBe("/t/mix.hdsession.json");
    expect(withSessionExtension("/t/mix.hdsession")).toBe("/t/mix.hdsession.json");
  });

  it("keeps a full extension", () => {
    expect(withSessionExtension("/t/mix.hdsession.json")).toBe("/t/mix.hdsession.json");
    expect(withSessionExtension("/t/MIX.HDSESSION.JSON")).toBe("/t/MIX.HDSESSION.JSON");
  });
});
