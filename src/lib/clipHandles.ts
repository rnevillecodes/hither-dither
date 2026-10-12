// Clip handles on the waveform: trim edges, end fades and join crossfades. Pure geometry,
// hit-testing and drag maths (clamped like the backend) so the view can preview drags.
import type { ClipInfo, JoinState } from "./api";
import { bendFromPoint, bendPointHeight, gain } from "./curve";
import { type Viewport, frameAtX, xAtFrame } from "./viewport";

/** How close (CSS px) the pointer must be to grab a handle. */
export const HIT_PX = 6;
/** Vertical inset (CSS px) of the envelope's 0 and 1 gain lines. */
export const PAD_PX = 6;

export type HandleKind =
  | "trimStart"
  | "trimEnd"
  | "fadeIn"
  | "fadeOut"
  | "fadeInBend"
  | "fadeOutBend"
  | "xfadeStart"
  | "xfadeEnd"
  | "xfadeBend";

export interface Handle {
  kind: HandleKind;
  x: number;
  y: number;
  /** Index into `clip.joins` for crossfade handles. */
  join?: number;
}

export const HANDLE_NAMES: Record<HandleKind, string> = {
  trimStart: "Trim start",
  trimEnd: "Trim end",
  fadeIn: "Fade-in length",
  fadeOut: "Fade-out length",
  fadeInBend: "Fade-in curve (double-click for linear)",
  fadeOutBend: "Fade-out curve (double-click for linear)",
  xfadeStart: "Crossfade length",
  xfadeEnd: "Crossfade length",
  xfadeBend: "Crossfade curve (double-click for linear)",
};

/** Frames heard: the trimmed length. */
export function clipFrames(clip: ClipInfo): number {
  return clip.trim_end - clip.trim_start;
}

/** Y (CSS px) of gain `g` on a canvas `height` px tall. */
export function envY(g: number, height: number): number {
  return PAD_PX + (1 - g) * (height - 2 * PAD_PX);
}

export function gainAtY(y: number, height: number): number {
  return Math.max(0, Math.min(1, 1 - (y - PAD_PX) / Math.max(1, height - 2 * PAD_PX)));
}

/** Joins inside the audio (trimmed-away joins have no handles). */
export function visibleJoins(clip: ClipInfo): { join: JoinState; i: number }[] {
  const frames = clipFrames(clip);
  return clip.joins.map((join, i) => ({ join, i })).filter(({ join }) => join.position > 0 && join.position < frames);
}

/** Every handle, in hit-test priority order (small targets before large ones). */
export function handles(clip: ClipInfo, vp: Viewport, height: number): Handle[] {
  const frames = clipFrames(clip);
  const top = envY(1, height);
  const x = (f: number) => xAtFrame(vp, f);
  const { fade_in: fi, fade_out: fo } = clip;
  const out: Handle[] = [];
  if (fi.len > 0) out.push({ kind: "fadeInBend", x: x(fi.len / 2), y: envY(bendPointHeight(fi.bend), height) });
  if (fo.len > 0) out.push({ kind: "fadeOutBend", x: x(frames - fo.len / 2), y: envY(bendPointHeight(fo.bend), height) });
  for (const { join, i } of visibleJoins(clip)) {
    if (join.len > 0) out.push({ kind: "xfadeBend", x: x(join.position), y: envY(bendPointHeight(join.bend), height), join: i });
  }
  out.push({ kind: "fadeIn", x: x(fi.len), y: top });
  out.push({ kind: "fadeOut", x: x(frames - fo.len), y: top });
  for (const { join, i } of visibleJoins(clip)) {
    out.push({ kind: "xfadeStart", x: x(join.position - join.len / 2), y: top, join: i });
    out.push({ kind: "xfadeEnd", x: x(join.position + join.len / 2), y: top, join: i });
  }
  out.push({ kind: "trimStart", x: x(0), y: height / 2 });
  out.push({ kind: "trimEnd", x: x(frames), y: height / 2 });
  return out;
}

/** The handle under (`px`, `py`), if any. Trim edges are grabbed anywhere along them. */
export function hitTest(clip: ClipInfo, vp: Viewport, height: number, px: number, py: number): Handle | null {
  for (const h of handles(clip, vp, height)) {
    const fullHeight = h.kind === "trimStart" || h.kind === "trimEnd";
    const near = fullHeight ? Math.abs(px - h.x) <= HIT_PX : Math.hypot(px - h.x, py - h.y) <= HIT_PX;
    if (near) return h;
  }
  return null;
}

/** The fades' lengths clamped to fit `frames` (fade-in first), as the backend does. */
function fitFades(clip: ClipInfo): ClipInfo {
  const frames = clipFrames(clip);
  const fiLen = Math.min(clip.fade_in.len, frames);
  const foLen = Math.min(clip.fade_out.len, frames - fiLen);
  return { ...clip, fade_in: { ...clip.fade_in, len: fiLen }, fade_out: { ...clip.fade_out, len: foLen } };
}

const clampInt = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, Math.round(v)));

/**
 * The clip after dragging `handle` from x0 to (`px`, `py`), starting from `origin` (the
 * clip when the drag began). Positions are in the view of `origin`, before any change.
 */
export function drag(
  origin: ClipInfo,
  handle: Handle,
  vp: Viewport,
  height: number,
  x0: number,
  px: number,
  py: number,
): ClipInfo {
  const frames = clipFrames(origin);
  const at = frameAtX(vp, px);
  const delta = at - frameAtX(vp, x0);
  const bend = bendFromPoint(gainAtY(py, height));
  const joins = origin.joins.slice();
  const setJoin = (i: number, change: Partial<JoinState>) => (joins[i] = { ...joins[i], ...change });
  switch (handle.kind) {
    case "trimStart": {
      const trim_start = clampInt(origin.trim_start + delta, 0, origin.trim_end - 1);
      return fitFades({ ...origin, trim_start });
    }
    case "trimEnd": {
      const trim_end = clampInt(origin.trim_end + delta, origin.trim_start + 1, origin.untrimmed_frames);
      return fitFades({ ...origin, trim_end });
    }
    case "fadeIn":
      return { ...origin, fade_in: { ...origin.fade_in, len: clampInt(at, 0, frames - origin.fade_out.len) } };
    case "fadeOut":
      return { ...origin, fade_out: { ...origin.fade_out, len: clampInt(frames - at, 0, frames - origin.fade_in.len) } };
    case "fadeInBend":
      return { ...origin, fade_in: { ...origin.fade_in, bend } };
    case "fadeOutBend":
      return { ...origin, fade_out: { ...origin.fade_out, bend } };
    case "xfadeStart":
    case "xfadeEnd": {
      const j = joins[handle.join!];
      // Even lengths: half each side of the join.
      const len = Math.min(j.max_len, 2 * Math.floor(Math.abs(at - j.position)));
      setJoin(handle.join!, { len: len - (len % 2) });
      return { ...origin, joins };
    }
    case "xfadeBend":
      setJoin(handle.join!, { bend });
      return { ...origin, joins };
  }
}

/** `handle`'s setting reset to a linear curve (double-click on a bend point). */
export function resetBend(origin: ClipInfo, handle: Handle): ClipInfo {
  switch (handle.kind) {
    case "fadeInBend":
      return { ...origin, fade_in: { ...origin.fade_in, bend: 0 } };
    case "fadeOutBend":
      return { ...origin, fade_out: { ...origin.fade_out, bend: 0 } };
    case "xfadeBend": {
      const joins = origin.joins.slice();
      joins[handle.join!] = { ...joins[handle.join!], bend: 0 };
      return { ...origin, joins };
    }
    default:
      return origin;
  }
}

/** Gain of the end fades at output frame `f` (1 outside them). */
export function fadeGain(clip: ClipInfo, f: number): number {
  const frames = clipFrames(clip);
  const { fade_in: fi, fade_out: fo } = clip;
  let g = 1;
  if (fi.len > 0 && f < fi.len) g *= gain("in", fi.bend, f / fi.len);
  if (fo.len > 0 && f >= frames - fo.len) g *= gain("out", fo.bend, (f - (frames - fo.len)) / fo.len);
  return g;
}

/** Whether a drag result changes the clip settings sent with `set_clip`. */
export function clipChanged(a: ClipInfo, b: ClipInfo): boolean {
  return (
    a.trim_start !== b.trim_start ||
    a.trim_end !== b.trim_end ||
    a.fade_in.len !== b.fade_in.len ||
    a.fade_in.bend !== b.fade_in.bend ||
    a.fade_out.len !== b.fade_out.len ||
    a.fade_out.bend !== b.fade_out.bend
  );
}
