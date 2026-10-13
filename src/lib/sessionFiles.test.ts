import { describe, expect, it } from "vitest";
import {
  closePrompt,
  defaultExportPath,
  defaultSessionPath,
  isSessionPath,
  planOpens,
  withSessionExtension,
} from "./sessionFiles";

describe("closePrompt", () => {
  it("names the session file the tab saves to", () => {
    expect(closePrompt({ name: "take.wav", sessionPath: "C:\\Takes\\mix.hdsession.json" })).toBe(
      "Save changes to the session “mix.hdsession.json” before closing?",
    );
  });

  it("says a session is saved, not the WAV, when the tab has none", () => {
    expect(closePrompt({ name: "take.wav", sessionPath: null })).toBe(
      "Save a session for “take.wav” before closing? The WAV itself isn't changed.",
    );
  });
});

describe("defaultExportPath", () => {
  it("sits next to the WAV with -export", () => {
    expect(defaultExportPath("C:\\Takes\\take.wav")).toBe("C:\\Takes\\take-export.wav");
    expect(defaultExportPath("/t/Take.WAVE")).toBe("/t/Take-export.wav");
  });

  it("adds the extension to a name without one", () => {
    expect(defaultExportPath("/t/take")).toBe("/t/take-export.wav");
  });
});

describe("planOpens", () => {
  const none = () => false;

  it("keeps the requested order for a mixed selection", () => {
    const plan = planOpens(
      [
        { kind: "wav", path: "/t/a.wav" },
        { kind: "session", sessionPath: "/t/mix.hdsession.json", audioPath: "/t/take.wav", relink: false },
        { kind: "wav", path: "/t/b.wav" },
      ],
      none,
    );
    expect(plan).toEqual([
      { kind: "open", path: "/t/a.wav", sessionPath: null, relink: false },
      { kind: "open", path: "/t/take.wav", sessionPath: "/t/mix.hdsession.json", relink: false },
      { kind: "open", path: "/t/b.wav", sessionPath: null, relink: false },
    ]);
  });

  it("passes a chosen WAV through for relinking", () => {
    const plan = planOpens(
      [{ kind: "session", sessionPath: "/t/mix.hdsession.json", audioPath: "/t/take-final.wav", relink: true }],
      none,
    );
    expect(plan).toEqual([
      { kind: "open", path: "/t/take-final.wav", sessionPath: "/t/mix.hdsession.json", relink: true },
    ]);
  });

  it("activates a chosen WAV that is already open, without relinking it", () => {
    const plan = planOpens(
      [{ kind: "session", sessionPath: "/t/mix.hdsession.json", audioPath: "/t/b.wav", relink: true }],
      (p) => p === "/t/b.wav",
    );
    expect(plan).toEqual([{ kind: "activate", path: "/t/b.wav", sessionPath: "/t/mix.hdsession.json" }]);
  });

  it("activates a WAV that is already open instead of applying the session", () => {
    const plan = planOpens(
      [{ kind: "session", sessionPath: "/t/mix.hdsession.json", audioPath: "/t/take.wav", relink: false }],
      (p) => p === "/t/take.wav",
    );
    expect(plan).toEqual([{ kind: "activate", path: "/t/take.wav", sessionPath: "/t/mix.hdsession.json" }]);
  });

  it("opens a WAV once when it is requested twice in one batch", () => {
    const plan = planOpens(
      [
        { kind: "wav", path: "C:\\t\\take.wav" },
        { kind: "session", sessionPath: "C:\\t\\mix.hdsession.json", audioPath: "C:\\t\\take.wav", relink: false },
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
