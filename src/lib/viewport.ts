// Pure pixel <-> frame mapping for the waveform view. All functions return new objects.

/** Closest zoom: one sample per 10 pixels. */
export const MIN_FRAMES_PER_PIXEL = 0.1;

export interface Viewport {
  /** Frame at the left edge (may be fractional). */
  start: number;
  /** Frames per CSS pixel. */
  fpp: number;
  /** View width in CSS pixels. */
  width: number;
  /** Total frames in the file. */
  totalFrames: number;
}

export function maxFpp(totalFrames: number, width: number): number {
  return Math.max(MIN_FRAMES_PER_PIXEL, totalFrames / Math.max(1, width));
}

/** Clamp zoom to [1 sample / 10 px, whole file] and keep the view inside the file. */
export function clamp(vp: Viewport): Viewport {
  const fpp = Math.min(Math.max(vp.fpp, MIN_FRAMES_PER_PIXEL), maxFpp(vp.totalFrames, vp.width));
  const maxStart = Math.max(0, vp.totalFrames - vp.width * fpp);
  const start = Math.min(Math.max(vp.start, 0), maxStart);
  return { ...vp, fpp, start };
}

/** Whole file in view. */
export function fit(totalFrames: number, width: number): Viewport {
  return clamp({ start: 0, fpp: maxFpp(totalFrames, width), width, totalFrames });
}

export function frameAtX(vp: Viewport, x: number): number {
  return vp.start + x * vp.fpp;
}

export function xAtFrame(vp: Viewport, frame: number): number {
  return (frame - vp.start) / vp.fpp;
}

/** Frame index under pixel `x`, rounded and clamped to `[0, totalFrames]`. */
export function frameIndexAtX(vp: Viewport, x: number): number {
  return Math.min(Math.max(Math.round(frameAtX(vp, x)), 0), vp.totalFrames);
}

export function visibleEnd(vp: Viewport): number {
  return Math.min(vp.totalFrames, vp.start + vp.width * vp.fpp);
}

/** Zoom by `factor` (> 1 zooms in) keeping the frame under pixel `x` fixed. */
export function zoomAround(vp: Viewport, factor: number, x: number): Viewport {
  const anchor = frameAtX(vp, x);
  const fpp = Math.min(Math.max(vp.fpp / factor, MIN_FRAMES_PER_PIXEL), maxFpp(vp.totalFrames, vp.width));
  return clamp({ ...vp, fpp, start: anchor - x * fpp });
}

export function scrollByPixels(vp: Viewport, dx: number): Viewport {
  return clamp({ ...vp, start: vp.start + dx * vp.fpp });
}

/** Scroll so `frame` is at the left edge (clamped). */
export function scrollTo(vp: Viewport, frame: number): Viewport {
  return clamp({ ...vp, start: frame });
}

/** Keep the zoom but change width (e.g. on resize), clamping as needed. */
export function resize(vp: Viewport, width: number): Viewport {
  return clamp({ ...vp, width });
}

export function canZoomIn(vp: Viewport): boolean {
  return vp.fpp > MIN_FRAMES_PER_PIXEL + 1e-12;
}

export function canZoomOut(vp: Viewport): boolean {
  return vp.fpp < maxFpp(vp.totalFrames, vp.width) - 1e-9;
}
