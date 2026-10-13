import { describe, expect, it } from "vitest";
import { clickView, LEGACY_OPEN_KEY, OPEN_KEY, readSidebarState, revealView, VIEW_KEY } from "./sidebar";

function storage(values: Record<string, string>) {
  return (key: string) => values[key] ?? null;
}

describe("clickView", () => {
  it("switches to another view", () => {
    expect(clickView({ open: true, view: "metrics" }, "effects")).toEqual({ open: true, view: "effects" });
  });

  it("collapses the sidebar when the shown view is clicked", () => {
    expect(clickView({ open: true, view: "effects" }, "effects")).toEqual({ open: false, view: "effects" });
  });

  it("opens the clicked view when collapsed", () => {
    expect(clickView({ open: false, view: "effects" }, "metrics")).toEqual({ open: true, view: "metrics" });
    expect(clickView({ open: false, view: "effects" }, "effects")).toEqual({ open: true, view: "effects" });
  });
});

describe("revealView", () => {
  it("always opens on the view", () => {
    expect(revealView({ open: false, view: "metrics" }, "effects")).toEqual({ open: true, view: "effects" });
    expect(revealView({ open: true, view: "effects" }, "effects")).toEqual({ open: true, view: "effects" });
  });
});

describe("readSidebarState", () => {
  it("starts open on Metrics on first launch", () => {
    expect(readSidebarState(storage({}))).toEqual({ open: true, view: "metrics" });
  });

  it("starts collapsed when the old details panel was hidden", () => {
    expect(readSidebarState(storage({ [LEGACY_OPEN_KEY]: "false" }))).toEqual({ open: false, view: "metrics" });
  });

  it("prefers the new choice over the old one", () => {
    const get = storage({ [LEGACY_OPEN_KEY]: "false", [OPEN_KEY]: "true", [VIEW_KEY]: "effects" });
    expect(readSidebarState(get)).toEqual({ open: true, view: "effects" });
  });

  it("falls back to Metrics for an unknown view", () => {
    expect(readSidebarState(storage({ [VIEW_KEY]: "plugins" }))).toEqual({ open: true, view: "metrics" });
  });
});
