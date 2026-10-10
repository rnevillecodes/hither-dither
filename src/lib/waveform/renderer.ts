// Canvas 2D drawing for the waveform lanes, ruler and overview strip.
// Contexts are unscaled: all coordinates passed to the canvas are device pixels.
import { type ClipRun, type PeakTile, PEAKS_KIND_MINMAX } from "../api";
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

  if (scene.tile) {
    if (scene.tile.kind === PEAKS_KIND_MINMAX) drawMinMax(ctx, scene, ls, theme);
    else drawSamples(ctx, scene, ls, theme);
  }

  drawLaneLabels(ctx, scene, ls, theme);

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

function drawMinMax(ctx: CanvasRenderingContext2D, scene: Scene, ls: Lane[], theme: Theme) {
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
      const yTop = Math.floor(amplitudeY(lane, max, dpr));
      const yBot = Math.ceil(amplitudeY(lane, min, dpr));
      ctx.rect(px, yTop, 1, Math.max(1, yBot - yTop));
    }
    ctx.fill();
  }
}

function drawSamples(ctx: CanvasRenderingContext2D, scene: Scene, ls: Lane[], theme: Theme) {
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
      const y = amplitudeY(lane, tile.values[base + i], dpr);
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
        const y = amplitudeY(lane, tile.values[base + i], dpr);
        ctx.moveTo(x + r, y);
        ctx.arc(x, y, r, 0, Math.PI * 2);
      }
      ctx.fill();
    }
  }
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
