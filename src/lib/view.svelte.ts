// Waveform viewport state shared between the waveform, toolbar and keyboard shortcuts.
import { type Viewport, canZoomIn, canZoomOut, fit, scrollTo, zoomAround } from "./viewport";

export const view = $state({
  vp: null as Viewport | null,
  /** Set when the user scrolls manually; suppresses playhead auto-scroll until the next play. */
  userScrolled: false,
});

/** Keyboard/toolbar zoom step. */
export const ZOOM_STEP = 2;

export function setViewport(vp: Viewport, byUser = false) {
  view.vp = vp;
  if (byUser) view.userScrolled = true;
}

export function zoomIn() {
  if (view.vp) setViewport(zoomAround(view.vp, ZOOM_STEP, view.vp.width / 2));
}

export function zoomOut() {
  if (view.vp) setViewport(zoomAround(view.vp, 1 / ZOOM_STEP, view.vp.width / 2));
}

export function zoomToFit() {
  if (view.vp) setViewport(fit(view.vp.totalFrames, view.vp.width));
}

/** Scroll so `frame` is visible, placing it `margin` of the width from the left edge. */
export function reveal(frame: number, margin = 0.1) {
  if (view.vp) setViewport(scrollTo(view.vp, frame - view.vp.width * view.vp.fpp * margin));
}

export function zoomState() {
  return {
    canIn: view.vp ? canZoomIn(view.vp) : false,
    canOut: view.vp ? canZoomOut(view.vp) : false,
  };
}
