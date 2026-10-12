// Canvas 2D drawing for the waveform lanes, ruler and overview strip.
// Contexts are unscaled: all coordinates passed to the canvas are device pixels.
import { type ClipInfo, type ClipRun, type PeakTile, PEAKS_KIND_MINMAX } from "../api";
import { clipFrames, envY, fadeGain, handles, visibleJoins } from "../clipHandles";
import { gain } from "../curve";
import { ticks } from "../ruler";
import type { Selection } from "../selection";
import { type Viewport, visibleEnd, xAtFrame } from "../viewport";
import type { Theme } from "./theme";

/** Draw samples as dots once each sample is at least this many CSS px apart. */
const POINT_MIN_SPACING = 4;
const LANE_PAD = 6;

export interface Scene {
  vp: Viewport;
  dpr: number;
  labels: string[];
  tile: PeakTile | null;
  selection: Selection | null;
  clipRuns: ClipRun[][] | null;
  playhead: number | null;
  /** The clip settings the audio was rendered with (tile coordinates). */
  clip: ClipInfo | null;
  /** The settings being dragged to, previewed over the rendered audio. */
  preview: ClipInfo | null;
  /** Text shown next to the pointer while dragging (CSS px). */
  label: { x: number; y: number; text: string } | null;
  /** A fade being tried out in the curve dialog, drawn over the rendered audio. */
  rangeFade: { start: number; end: number; dir: "in" | "out"; bend: number } | null;
}

/** Gain the curve dialog's fade would apply at frame `f`. */
function rangeFadeGain(fade: NonNullable<Scene["rangeFade"]>) {
  return (f: number) =>
    f < fade.start || f >= fade.end ? 1 : gain(fade.dir, fade.bend, (f - fade.start) / (fade.end - fade.start));
}

/**
 * How the preview maps onto the rendered audio: the shift from preview frames to tile
 * frames (a trim moves the start), and the extra gain the preview's end fades apply.
 */
function previewMapping(scene: Scene) {
  const { clip, preview } = scene;
  if (!clip || !preview) return null;
  const offset = preview.trim_start - clip.trim_start;
  return {
    offset,
    /** Gain to apply on top of the rendered audio at tile frame `f`. */
    gainAt(f: number) {
      const before = Math.max(fadeGain(clip, f), 1e-3);
      return Math.min(1 / before, fadeGain(preview, f - offset) / before);
    },
  };
}

interface Lane {
  top: number;
  height: number;
}

function lanes(count: number, height: number): Lane[] {
  const h = height / count;
  return Array.from({ length: count }, (_, i) => ({ top: Math.round(i * h), height: Math.round((i + 1) * h) - Math.round(i * h) }));
}

/** Device-pixel y for amplitude `v` (−1…+1) in a lane. */
function amplitudeY(lane: Lane, v: number, dpr: number): number {
  const pad = LANE_PAD * dpr;
  const inner = lane.height - 2 * pad;
  const clamped = Math.max(-1, Math.min(1, v));
  return lane.top + pad + ((1 - clamped) / 2) * inner;
}

export function drawWaveform(ctx: CanvasRenderingContext2D, scene: Scene, theme: Theme) {
  const { vp, dpr } = scene;
  const W = ctx.canvas.width;
  const H = ctx.canvas.height;
  const channels = Math.max(1, scene.labels.length);
  const ls = lanes(channels, H);

  ctx.clearRect(0, 0, W, H);
  ls.forEach((lane, c) => {
    ctx.fillStyle = c % 2 ? theme.laneAltBg : theme.laneBg;
    ctx.fillRect(0, lane.top, W, lane.height);
  });

  // Selection highlight (behind the waveform, across all lanes).
  if (scene.selection) {
    const x0 = xAtFrame(vp, scene.selection.start) * dpr;
    const x1 = xAtFrame(vp, scene.selection.end) * dpr;
    if (x1 >= 0 && x0 <= W) {
      ctx.fillStyle = theme.selection;
      ctx.fillRect(x0, 0, Math.max(1, x1 - x0), H);
      ctx.fillStyle = theme.selectionEdge;
      ctx.fillRect(Math.round(x0), 0, Math.max(1, dpr), H);
      ctx.fillRect(Math.round(x1), 0, Math.max(1, dpr), H);
    }
  }

  // Clip-run markers.
  if (scene.clipRuns) {
    const end = visibleEnd(vp);
    scene.clipRuns.forEach((runs, c) => {
      const lane = ls[c];
      if (!lane) return;
      ctx.fillStyle = theme.clip;
      const markers: [number, number][] = [];
      for (let i = firstRunEnding(runs, vp.start); i < runs.length && runs[i].start <= end; i++) {
        const r = runs[i];
        const x0 = xAtFrame(vp, r.start) * dpr;
        const x1 = xAtFrame(vp, r.start + r.len) * dpr;
        const w = Math.max(2 * dpr, x1 - x0);
        ctx.fillRect(x0, lane.top, w, lane.height);
        markers.push([x0, w]);
      }
      ctx.fillStyle = theme.clipMarker;
      for (const [x, w] of markers) ctx.fillRect(x, lane.top, w, 3 * dpr);
    });
  }

  // Zero lines and ±1.0 guides.
  ctx.fillStyle = theme.zero;
  for (const lane of ls) ctx.fillRect(0, Math.round(amplitudeY(lane, 0, dpr)), W, Math.max(1, Math.floor(dpr)));
  ctx.fillStyle = theme.grid;
  for (const lane of ls) {
    ctx.fillRect(0, Math.round(amplitudeY(lane, 1, dpr)), W, 1);
    ctx.fillRect(0, Math.round(amplitudeY(lane, -1, dpr)), W, 1);
    ctx.fillRect(0, lane.top + lane.height - 1, W, 1);
  }

  const mapping = previewMapping(scene);
  const fadeAt = scene.rangeFade ? rangeFadeGain(scene.rangeFade) : null;
  const gainAt =
    mapping || fadeAt ? (f: number) => (mapping ? mapping.gainAt(f) : 1) * (fadeAt ? fadeAt(f) : 1) : null;
  if (scene.tile) {
    if (scene.tile.kind === PEAKS_KIND_MINMAX) drawMinMax(ctx, scene, ls, theme, gainAt);
    else drawSamples(ctx, scene, ls, theme, gainAt);
  }

  drawLaneLabels(ctx, scene, ls, theme);
  drawClip(ctx, scene, theme, mapping?.offset ?? 0);
  if (scene.rangeFade) drawRangeFade(ctx, scene, theme, scene.rangeFade);

  if (scene.playhead !== null) {
    const x = Math.round(xAtFrame(vp, scene.playhead) * dpr);
    if (x >= 0 && x <= W) {
      ctx.fillStyle = theme.playhead;
      ctx.fillRect(x - Math.floor(dpr / 2), 0, Math.max(1, Math.round(dpr * 1.5)), H);
    }
  }
}

/** Index of the first run that ends after `frame` (runs are sorted by start). */
function firstRunEnding(runs: ClipRun[], frame: number): number {
  let lo = 0;
  let hi = runs.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (runs[mid].start + runs[mid].len < frame) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

type GainAt = ((frame: number) => number) | null;

function drawMinMax(ctx: CanvasRenderingContext2D, scene: Scene, ls: Lane[], theme: Theme, gainAt: GainAt) {
  const { vp, dpr } = scene;
  const tile = scene.tile!;
  const W = ctx.canvas.width;
  const tileFpp = tile.span / tile.count;
  const pxFpp = vp.fpp / dpr;
  const single = tileFpp >= pxFpp * 0.98;
  ctx.fillStyle = theme.wave;

  for (let c = 0; c < ls.length && c < tile.channels; c++) {
    const lane = ls[c];
    const base = c * tile.count * 2;
    ctx.beginPath();
    for (let px = 0; px < W; px++) {
      const a = vp.start + px * pxFpp;
      const b = a + pxFpp;
      let i0: number;
      let i1: number;
      if (single) {
        i0 = i1 = Math.floor(((a + b) / 2 - tile.start) / tileFpp);
      } else {
        i0 = Math.floor((a - tile.start) / tileFpp);
        i1 = Math.ceil((b - tile.start) / tileFpp) - 1;
      }
      i0 = Math.max(i0, 0);
      i1 = Math.min(i1, tile.count - 1);
      if (i0 > i1) continue;
      let min = Infinity;
      let max = -Infinity;
      for (let i = i0; i <= i1; i++) {
        const lo = tile.values[base + 2 * i];
        const hi = tile.values[base + 2 * i + 1];
        if (lo < min) min = lo;
        if (hi > max) max = hi;
      }
      if (min > max) continue;
      if (gainAt) {
        const g = gainAt((a + b) / 2);
        min *= g;
        max *= g;
      }
      const yTop = Math.floor(amplitudeY(lane, max, dpr));
      const yBot = Math.ceil(amplitudeY(lane, min, dpr));
      ctx.rect(px, yTop, 1, Math.max(1, yBot - yTop));
    }
    ctx.fill();
  }
}

function drawSamples(ctx: CanvasRenderingContext2D, scene: Scene, ls: Lane[], theme: Theme, gainAt: GainAt) {
  const { vp, dpr } = scene;
  const tile = scene.tile!;
  const W = ctx.canvas.width;
  const first = Math.max(0, Math.floor(vp.start - tile.start) - 1);
  const last = Math.min(tile.count - 1, Math.ceil(visibleEnd(vp) - tile.start) + 1);
  const showPoints = 1 / vp.fpp >= POINT_MIN_SPACING;
  ctx.strokeStyle = theme.wave;
  ctx.fillStyle = theme.wave;
  ctx.lineWidth = Math.max(1, dpr);

  for (let c = 0; c < ls.length && c < tile.channels; c++) {
    const lane = ls[c];
    const base = c * tile.count;
    ctx.beginPath();
    for (let i = first; i <= last; i++) {
      const x = xAtFrame(vp, tile.start + i) * dpr;
      const v = tile.values[base + i] * (gainAt ? gainAt(tile.start + i) : 1);
      const y = amplitudeY(lane, v, dpr);
      if (i === first) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.stroke();
    if (showPoints) {
      const r = 2.25 * dpr;
      ctx.beginPath();
      for (let i = first; i <= last; i++) {
        const x = xAtFrame(vp, tile.start + i) * dpr;
        if (x < -r || x > W + r) continue;
        const v = tile.values[base + i] * (gainAt ? gainAt(tile.start + i) : 1);
      const y = amplitudeY(lane, v, dpr);
        ctx.moveTo(x + r, y);
        ctx.arc(x, y, r, 0, Math.PI * 2);
      }
      ctx.fill();
    }
  }
}

/**
 * Trim edges, fade lines, crossfade curves and their handles, for the clip being shown
 * (the preview while dragging). `offset` shifts preview frames onto tile frames.
 */
function drawClip(ctx: CanvasRenderingContext2D, scene: Scene, theme: Theme, offset: number) {
  const shown = scene.preview ?? scene.clip;
  if (!shown) return;
  const { dpr } = scene;
  const W = ctx.canvas.width;
  const H = ctx.canvas.height;
  const cssH = H / dpr;
  // A viewport in which preview frames land where they will be heard.
  const vp = { ...scene.vp, start: scene.vp.start - offset };
  const x = (f: number) => xAtFrame(vp, f) * dpr;
  const y = (g: number) => envY(g, cssH) * dpr;
  const frames = clipFrames(shown);

  // While dragging, dim rendered audio that the preview trims away.
  if (scene.preview && scene.clip) {
    ctx.fillStyle = "rgba(0, 0, 0, 0.55)";
    const startX = x(0);
    const endX = x(frames);
    if (startX > 0) ctx.fillRect(0, 0, Math.min(startX, W), H);
    if (endX < W) ctx.fillRect(Math.max(endX, 0), 0, W - Math.max(endX, 0), H);
  }

  ctx.strokeStyle = theme.selectionEdge;
  ctx.lineWidth = Math.max(1, 1.5 * dpr);
  /** Stroke a fade curve from frame `f0` to `f1`. */
  const curve = (f0: number, f1: number, at: (t: number) => number) => {
    const x0 = x(f0);
    const x1 = x(f1);
    if (x1 < 0 || x0 > W) return;
    const steps = Math.max(2, Math.min(256, Math.ceil(Math.abs(x1 - x0) / (3 * dpr))));
    ctx.beginPath();
    for (let i = 0; i <= steps; i++) {
      const t = i / steps;
      const px = x0 + (x1 - x0) * t;
      if (i === 0) ctx.moveTo(px, y(at(t)));
      else ctx.lineTo(px, y(at(t)));
    }
    ctx.stroke();
  };
  const { fade_in: fi, fade_out: fo } = shown;
  if (fi.len > 0) curve(0, fi.len, (t) => gain("in", fi.bend, t));
  if (fo.len > 0) curve(frames - fo.len, frames, (t) => gain("out", fo.bend, t));
  for (const { join } of visibleJoins(shown)) {
    const jx = x(join.position);
    ctx.save();
    ctx.setLineDash([3 * dpr, 3 * dpr]);
    ctx.beginPath();
    ctx.moveTo(Math.round(jx) + 0.5, 0);
    ctx.lineTo(Math.round(jx) + 0.5, H);
    ctx.stroke();
    ctx.restore();
    if (join.len > 0) {
      const a = join.position - join.len / 2;
      const b = join.position + join.len / 2;
      curve(a, b, (t) => gain("out", join.bend, t));
      curve(a, b, (t) => gain("in", join.bend, t));
    }
  }

  // Handles.
  ctx.fillStyle = theme.selectionEdge;
  for (const h of handles(shown, vp, cssH)) {
    const hx = h.x * dpr;
    if (hx < -10 * dpr || hx > W + 10 * dpr) continue;
    const hy = h.y * dpr;
    switch (h.kind) {
      case "trimStart":
      case "trimEnd": {
        // A bracket: full-height edge with short arms pointing into the audio.
        const dir = h.kind === "trimStart" ? 1 : -1;
        const w = 2 * dpr;
        ctx.fillRect(hx - (dir > 0 ? 0 : w), 0, w, H);
        ctx.fillRect(hx - (dir > 0 ? 0 : 8 * dpr), 0, 8 * dpr, w);
        ctx.fillRect(hx - (dir > 0 ? 0 : 8 * dpr), H - w, 8 * dpr, w);
        break;
      }
      case "fadeIn":
      case "fadeOut":
      case "xfadeStart":
      case "xfadeEnd": {
        const r = 4 * dpr;
        ctx.fillRect(hx - r, hy - r, 2 * r, 2 * r);
        break;
      }
      default: {
        ctx.beginPath();
        ctx.arc(hx, hy, 4.5 * dpr, 0, Math.PI * 2);
        ctx.fill();
      }
    }
  }

  if (scene.label) {
    const { text } = scene.label;
    ctx.font = `${11 * dpr}px system-ui, sans-serif`;
    ctx.textAlign = "left";
    ctx.textBaseline = "middle";
    const w = ctx.measureText(text).width + 10 * dpr;
    const h = 18 * dpr;
    const lx = Math.min(Math.max(scene.label.x * dpr + 12 * dpr, 2 * dpr), W - w - 2 * dpr);
    const ly = Math.min(Math.max(scene.label.y * dpr - h - 6 * dpr, 2 * dpr), H - h - 2 * dpr);
    ctx.fillStyle = theme.laneAltBg;
    ctx.fillRect(lx, ly, w, h);
    ctx.fillStyle = theme.text;
    ctx.fillText(text, lx + 5 * dpr, ly + h / 2);
  }
}

/** The curve dialog's fade line over its range. */
function drawRangeFade(
  ctx: CanvasRenderingContext2D,
  scene: Scene,
  theme: Theme,
  fade: NonNullable<Scene["rangeFade"]>,
) {
  const { vp, dpr } = scene;
  const W = ctx.canvas.width;
  const cssH = ctx.canvas.height / dpr;
  const x0 = xAtFrame(vp, fade.start) * dpr;
  const x1 = xAtFrame(vp, fade.end) * dpr;
  if (x1 < 0 || x0 > W) return;
  const steps = Math.max(2, Math.min(256, Math.ceil(Math.abs(x1 - x0) / (3 * dpr))));
  ctx.strokeStyle = theme.selectionEdge;
  ctx.lineWidth = Math.max(1, 1.5 * dpr);
  ctx.beginPath();
  for (let i = 0; i <= steps; i++) {
    const t = i / steps;
    const y = envY(gain(fade.dir, fade.bend, t), cssH) * dpr;
    if (i === 0) ctx.moveTo(x0 + (x1 - x0) * t, y);
    else ctx.lineTo(x0 + (x1 - x0) * t, y);
  }
  ctx.stroke();
}

function drawLaneLabels(ctx: CanvasRenderingContext2D, scene: Scene, ls: Lane[], theme: Theme) {
  const { dpr } = scene;
  const W = ctx.canvas.width;
  ctx.font = `${11 * dpr}px system-ui, sans-serif`;
  ctx.textBaseline = "middle";
  ls.forEach((lane, c) => {
    if (lane.height < 30 * dpr) return;
    // Channel label, top-left.
    ctx.textAlign = "left";
    ctx.fillStyle = theme.text;
    ctx.font = `600 ${12 * dpr}px system-ui, sans-serif`;
    ctx.fillText(scene.labels[c] ?? `Ch ${c + 1}`, 8 * dpr, lane.top + 12 * dpr);
    // Amplitude scale, right edge.
    ctx.font = `${10 * dpr}px system-ui, sans-serif`;
    ctx.textAlign = "right";
    const x = W - 6 * dpr;
    const scale: [string, number][] = [
      ["+1.0", amplitudeY(lane, 1, dpr) + 7 * dpr],
      ["0", amplitudeY(lane, 0, dpr) - 7 * dpr],
      ["−1.0", amplitudeY(lane, -1, dpr) - 7 * dpr],
    ];
    for (const [text, y] of scale) {
      // Backing so the label stays readable over the waveform.
      const w = ctx.measureText(text).width + 6 * dpr;
      ctx.fillStyle = c % 2 ? theme.laneAltBg : theme.laneBg;
      ctx.fillRect(x - w + 3 * dpr, y - 6 * dpr, w, 12 * dpr);
      ctx.fillStyle = theme.textDim;
      ctx.fillText(text, x, y);
    }
  });
}

export function drawRuler(ctx: CanvasRenderingContext2D, vp: Viewport, dpr: number, sampleRate: number, theme: Theme, playhead: number | null) {
  const W = ctx.canvas.width;
  const H = ctx.canvas.height;
  ctx.clearRect(0, 0, W, H);
  const { ticks: ts } = ticks(vp, sampleRate);
  ctx.fillStyle = theme.textDim;
  ctx.font = `${10.5 * dpr}px system-ui, sans-serif`;
  ctx.textAlign = "left";
  ctx.textBaseline = "top";
  for (const t of ts) {
    const x = Math.round(t.x * dpr);
    if (t.label) {
      ctx.fillRect(x, H - 9 * dpr, 1, 9 * dpr);
      ctx.fillStyle = theme.text;
      ctx.fillText(t.label, x + 3 * dpr, 4 * dpr);
      ctx.fillStyle = theme.textDim;
    } else {
      ctx.fillRect(x, H - 4 * dpr, 1, 4 * dpr);
    }
  }
  ctx.fillRect(0, H - 1, W, 1);
  if (playhead !== null) {
    const x = xAtFrame(vp, playhead) * dpr;
    ctx.fillStyle = theme.playhead;
    ctx.beginPath();
    ctx.moveTo(x - 5 * dpr, H - 8 * dpr);
    ctx.lineTo(x + 5 * dpr, H - 8 * dpr);
    ctx.lineTo(x, H);
    ctx.fill();
  }
}

/** Whole-file strip with the visible window highlighted. `tile` covers the whole file. */
export function drawOverview(
  ctx: CanvasRenderingContext2D,
  tile: PeakTile | null,
  vp: Viewport,
  dpr: number,
  theme: Theme,
  playhead: number | null,
) {
  const W = ctx.canvas.width;
  const H = ctx.canvas.height;
  ctx.clearRect(0, 0, W, H);
  ctx.fillStyle = theme.laneAltBg;
  ctx.fillRect(0, 0, W, H);
  const total = vp.totalFrames;
  if (tile && total > 0) {
    ctx.fillStyle = theme.overviewWave;
    const mid = H / 2;
    const amp = (H - 4 * dpr) / 2;
    ctx.beginPath();
    for (let px = 0; px < W; px++) {
      let min = Infinity;
      let max = -Infinity;
      const i0 = Math.floor((px / W) * tile.count);
      const i1 = Math.max(i0, Math.ceil(((px + 1) / W) * tile.count) - 1);
      for (let c = 0; c < tile.channels; c++) {
        for (let i = i0; i <= i1 && i < tile.count; i++) {
          if (tile.kind === PEAKS_KIND_MINMAX) {
            const base = c * tile.count * 2 + 2 * i;
            min = Math.min(min, tile.values[base]);
            max = Math.max(max, tile.values[base + 1]);
          } else {
            const v = tile.values[c * tile.count + i];
            min = Math.min(min, v);
            max = Math.max(max, v);
          }
        }
      }
      if (min > max) continue;
      const y0 = Math.floor(mid - Math.min(1, max) * amp);
      const y1 = Math.ceil(mid - Math.max(-1, min) * amp);
      ctx.rect(px, y0, 1, Math.max(1, y1 - y0));
    }
    ctx.fill();
    const x0 = (vp.start / total) * W;
    const x1 = (visibleEnd(vp) / total) * W;
    ctx.fillStyle = theme.overviewWindow;
    ctx.fillRect(x0, 0, Math.max(3 * dpr, x1 - x0), H);
    if (playhead !== null) {
      ctx.fillStyle = theme.playhead;
      ctx.fillRect(Math.round((playhead / total) * W), 0, Math.max(1, dpr), H);
    }
  }
}
