import { describe, expect, it } from "vitest";
import { newTab } from "./tabs.svelte";
import { windowTitle } from "./windowTitle";

function tab(sessionPath: string | null, dirty = false, status: "loading" | "ready" = "ready") {
  return Object.assign(newTab("C:\\takes\\take.wav", sessionPath), { dirty, status });
}

describe("windowTitle", () => {
  it("shows a saved session's name (Saved session)", () => {
    expect(windowTitle(tab("C:\\takes\\mix.hdsession.json"))).toBe("mix — HitherDither");
  });

  it("is the app name for a tab never saved as a session, even with changes (Never saved)", () => {
    expect(windowTitle(tab(null))).toBe("HitherDither");
    expect(windowTitle(tab(null, true))).toBe("HitherDither");
  });

  it("is the app name with no tab (No tabs)", () => {
    expect(windowTitle(null)).toBe("HitherDither");
  });

  it("marks unsaved changes until they are saved (Change after saving)", () => {
    const t = tab("C:\\takes\\mix.hdsession.json");
    t.dirty = true;
    expect(windowTitle(t)).toBe("mix • — HitherDither");
    t.dirty = false;
    expect(windowTitle(t)).toBe("mix — HitherDither");
  });

  it("follows a new session file (Save As)", () => {
    const t = tab("C:\\takes\\mix.hdsession.json");
    t.sessionPath = "C:\\takes\\mix-v2.hdsession.json";
    expect(windowTitle(t)).toBe("mix-v2 — HitherDither");
  });

  it("is the app name while the tab is loading", () => {
    expect(windowTitle(tab("C:\\takes\\mix.hdsession.json", false, "loading"))).toBe("HitherDither");
  });

  it("reads Windows and POSIX paths, matching the extension ignoring case", () => {
    expect(windowTitle(tab("/home/me/takes/mix.hdsession.json"))).toBe("mix — HitherDither");
    expect(windowTitle(tab("D:\\Takes\\Mix.HDSession.JSON"))).toBe("Mix — HitherDither");
    // Only the session extension comes off.
    expect(windowTitle(tab("/takes/v1.2.final.hdsession.json"))).toBe("v1.2.final — HitherDither");
  });
});
