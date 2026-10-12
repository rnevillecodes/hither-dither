import { describe, expect, it } from "vitest";
import type { ClipInfo } from "./api";
import { bendPointHeight } from "./curve";
import { clipChanged, drag, envY, fadeGain, handles, hitTest, resetBend } from "./clipHandles";
import type { Viewport } from "./viewport";

const H = 200;
/** 1 frame per pixel, file 1000 frames wide. */
const vp: Viewport = { start: 0, fpp: 1, width: 1000, totalFrames: 1000 };

function clip(over: Partial<ClipInfo> = {}): ClipInfo {
  return {
    untrimmed_frames: 1000,
    trim_start: 0,
    trim_end: 1000,
    fade_in: { len: 0, bend: 0 },
    fade_out: { len: 0, bend: 0 },
    joins: [],
    ...over,
  };
}

const find = (c: ClipInfo, kind: string) => handles(c, vp, H).find((h) => h.kind === kind)!;

describe("hitTest", () => {
  it("grabs trim edges anywhere along them", () => {
    expect(hitTest(clip(), vp, H, 2, 150)?.kind).toBe("trimStart");
    expect(hitTest(clip(), vp, H, 998, 20)?.kind).toBe("trimEnd");
  });

  it("prefers the fade handle at the top over the trim edge under it", () => {
    expect(hitTest(clip(), vp, H, 1, envY(1, H))?.kind).toBe("fadeIn");
  });

  it("prefers a bend point over everything else", () => {
    const c = clip({ fade_in: { len: 10, bend: 0 } });
    const bend = find(c, "fadeInBend");
    expect(hitTest(c, vp, H, bend.x, bend.y)?.kind).toBe("fadeInBend");
  });

  it("leaves the rest of the waveform for selecting", () => {
    // Spec: pressing 20 px in from the start makes a selection.
    expect(hitTest(clip(), vp, H, 20, 100)).toBeNull();
  });

  it("shows crossfade handles only for joins inside the audio", () => {
    const join = { index: 1, position: 500, len: 20, bend: 0, max_len: 400 };
    const c = clip({ joins: [join, { ...join, index: 2, position: -5 }] });
    const xf = handles(c, vp, H).filter((h) => h.kind.startsWith("xfade"));
    expect(xf.map((h) => [h.kind, h.x])).toEqual([
      ["xfadeBend", 500],
      ["xfadeStart", 490],
      ["xfadeEnd", 510],
    ]);
  });
});

describe("drag", () => {
  it("trims the start and can restore it", () => {
    const trimmed = drag(clip(), find(clip(), "trimStart"), vp, H, 0, 150, 100);
    expect(trimmed.trim_start).toBe(150);
    const restored = drag(trimmed, find(trimmed, "trimStart"), vp, H, 0, -400, 100);
    expect(restored.trim_start).toBe(0);
  });

  it("can't trim to nothing", () => {
    const c = drag(clip(), find(clip(), "trimStart"), vp, H, 0, 5000, 100);
    expect(c.trim_start).toBe(999);
    const e = drag(clip(), find(clip(), "trimEnd"), vp, H, 1000, -5000, 100);
    expect(e.trim_end).toBe(1);
  });

  it("keeps fades from overlapping", () => {
    // Spec: a 2.0 s file with a 1.5 s fade-out stops the fade-in at 0.5 s.
    const c = clip({ fade_out: { len: 750, bend: 0 } });
    const d = drag(c, find(c, "fadeIn"), vp, H, 0, 900, 0);
    expect(d.fade_in.len).toBe(250);
  });

  it("sets the fade-out from the end", () => {
    const d = drag(clip(), find(clip(), "fadeOut"), vp, H, 1000, 800, 0);
    expect(d.fade_out.len).toBe(200);
  });

  it("shortens fades when trimming leaves too little audio", () => {
    const c = clip({ fade_in: { len: 300, bend: 0 } });
    const d = drag(c, find(c, "trimEnd"), vp, H, 1000, 200, 100);
    expect(d.trim_end).toBe(200);
    expect(d.fade_in.len).toBe(200);
  });

  it("bends from the pointer height and resets on double-click", () => {
    const c = clip({ fade_out: { len: 100, bend: 0 } });
    const h = find(c, "fadeOutBend");
    const up = drag(c, h, vp, H, h.x, h.x, envY(bendPointHeight(0.6), H));
    expect(up.fade_out.bend).toBeCloseTo(0.6, 9);
    const top = drag(c, h, vp, H, h.x, h.x, -50);
    expect(top.fade_out.bend).toBeCloseTo(1, 9);
    expect(resetBend(up, h).fade_out.bend).toBe(0);
  });

  it("sizes crossfades symmetrically, even and within the limit", () => {
    const join = { index: 1, position: 500, len: 10, bend: 0, max_len: 120 };
    const c = clip({ joins: [join] });
    const h = find(c, "xfadeEnd");
    expect(drag(c, h, vp, H, h.x, 533.7, 0).joins[0].len).toBe(66);
    expect(drag(c, h, vp, H, h.x, 900, 0).joins[0].len).toBe(120);
    expect(drag(c, h, vp, H, h.x, 500, 0).joins[0].len).toBe(0);
  });
});

describe("fadeGain", () => {
  it("multiplies the end fades and is 1 between them", () => {
    const c = clip({ fade_in: { len: 100, bend: 0 }, fade_out: { len: 200, bend: 0 } });
    expect(fadeGain(c, 50)).toBeCloseTo(0.5);
    expect(fadeGain(c, 500)).toBe(1);
    expect(fadeGain(c, 900)).toBeCloseTo(0.5);
  });
});

describe("clipChanged", () => {
  it("ignores crossfade-only changes (sent separately)", () => {
    const join = { index: 1, position: 500, len: 10, bend: 0, max_len: 120 };
    expect(clipChanged(clip({ joins: [join] }), clip({ joins: [{ ...join, len: 50 }] }))).toBe(false);
    expect(clipChanged(clip(), clip({ trim_start: 1 }))).toBe(true);
  });
});
