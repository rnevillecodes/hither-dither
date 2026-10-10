import { describe, expect, it } from "vitest";
import {
  MIN_FRAMES_PER_PIXEL,
  canZoomIn,
  canZoomOut,
  clamp,
  fit,
  frameAtX,
  frameIndexAtX,
  resize,
  scrollByPixels,
  scrollTo,
  visibleEnd,
  xAtFrame,
  zoomAround,
} from "./viewport";

const TOTAL = 48_000 * 60; // one minute at 48 kHz

describe("viewport", () => {
  it("fits the whole file", () => {
    const vp = fit(TOTAL, 1000);
    expect(vp.start).toBe(0);
    expect(visibleEnd(vp)).toBe(TOTAL);
    expect(canZoomOut(vp)).toBe(false);
  });

  it("maps pixels and frames both ways", () => {
    const vp = { start: 1000, fpp: 10, width: 500, totalFrames: TOTAL };
    expect(frameAtX(vp, 0)).toBe(1000);
    expect(frameAtX(vp, 50)).toBe(1500);
    expect(xAtFrame(vp, 1500)).toBe(50);
    expect(frameIndexAtX(vp, -1000)).toBe(0);
  });

  it("keeps the frame under the pointer fixed when zooming", () => {
    let vp = fit(TOTAL, 1000);
    for (const x of [0, 137, 500, 999]) {
      for (const factor of [2, 1.25, 0.5]) {
        const zoomedIn = zoomAround(scrollTo({ ...vp, fpp: vp.fpp / 8 }, TOTAL / 3), factor, x);
        const before = frameAtX(scrollTo({ ...vp, fpp: vp.fpp / 8 }, TOTAL / 3), x);
        expect(frameAtX(zoomedIn, x)).toBeCloseTo(before, 6);
      }
    }
    // Repeated zoom-in at one point stays anchored until the limit.
    vp = fit(TOTAL, 1000);
    const anchor = frameAtX(vp, 640);
    for (let i = 0; i < 40; i++) {
      vp = zoomAround(vp, 1.5, 640);
      expect(frameAtX(vp, 640)).toBeCloseTo(anchor, 4);
    }
    expect(vp.fpp).toBe(MIN_FRAMES_PER_PIXEL);
    expect(canZoomIn(vp)).toBe(false);
  });

  it("clamps zoom and scroll to the file", () => {
    const vp = clamp({ start: -50, fpp: 1e9, width: 800, totalFrames: TOTAL });
    expect(vp.start).toBe(0);
    expect(vp.fpp).toBe(TOTAL / 800);
    const tiny = clamp({ start: 0, fpp: 0.001, width: 800, totalFrames: TOTAL });
    expect(tiny.fpp).toBe(MIN_FRAMES_PER_PIXEL);
    const scrolled = scrollByPixels({ start: 0, fpp: 100, width: 800, totalFrames: TOTAL }, 1e9);
    expect(visibleEnd(scrolled)).toBe(TOTAL);
    expect(scrolled.start).toBe(TOTAL - 800 * 100);
  });

  it("re-clamps on resize", () => {
    const vp = resize(fit(TOTAL, 500), 1000);
    expect(vp.fpp).toBe(TOTAL / 1000);
    expect(vp.start).toBe(0);
  });

  it("allows zooming a short file to one sample per 10 px", () => {
    const vp = zoomAround(fit(100, 1000), 1e6, 0);
    expect(vp.fpp).toBe(MIN_FRAMES_PER_PIXEL);
  });
});
