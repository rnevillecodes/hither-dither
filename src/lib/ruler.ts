// Time-ruler tick generation, from hours down to individual sample indices.
import { type Viewport, frameAtX, visibleEnd, xAtFrame } from "./viewport";

export interface Tick {
  x: number;
  label: string | null;
}

/** Minimum spacing between labelled ticks, in CSS pixels. */
export const MIN_LABEL_SPACING = 90;

const TIME_STEPS = [
  0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300, 600,
  900, 1800, 3600, 7200,
];
const SAMPLE_STEPS = [1, 2, 5, 10, 20, 50, 100, 200, 500, 1000, 2000, 5000];

/** Label for a time in seconds, with as many decimals as the tick step needs. */
export function formatTickTime(secs: number, step: number): string {
  const decimals = step >= 1 ? 0 : step >= 0.1 ? 1 : step >= 0.01 ? 2 : 3;
  const sign = secs < 0 ? "-" : "";
  secs = Math.abs(secs);
  const scale = 10 ** decimals;
  const total = Math.round(secs * scale) / scale;
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total - h * 3600 - m * 60;
  const sStr = s.toFixed(decimals).padStart(decimals ? decimals + 3 : 2, "0");
  return h > 0 ? `${sign}${h}:${String(m).padStart(2, "0")}:${sStr}` : `${sign}${m}:${sStr}`;
}

export type RulerMode = "time" | "samples";

export function ticks(vp: Viewport, sampleRate: number): { mode: RulerMode; ticks: Tick[] } {
  const minFrames = MIN_LABEL_SPACING * vp.fpp;
  const minSecs = minFrames / sampleRate;
  const timeStep = TIME_STEPS.find((s) => s >= minSecs);
  const sampleMode = minSecs < 0.001;

  let stepFrames: number;
  let label: (frame: number) => string;
  let mode: RulerMode;
  if (sampleMode) {
    stepFrames = SAMPLE_STEPS.find((s) => s >= minFrames) ?? SAMPLE_STEPS[SAMPLE_STEPS.length - 1];
    label = (f) => String(f);
    mode = "samples";
  } else {
    const step = timeStep ?? TIME_STEPS[TIME_STEPS.length - 1];
    stepFrames = step * sampleRate;
    label = (f) => formatTickTime(f / sampleRate, step);
    mode = "time";
  }

  const out: Tick[] = [];
  const first = Math.ceil(frameAtX(vp, 0) / stepFrames);
  const end = visibleEnd(vp);
  for (let i = first; i * stepFrames <= end; i++) {
    const frame = i * stepFrames;
    out.push({ x: xAtFrame(vp, frame), label: label(Math.round(frame)) });
    // Four unlabelled minor ticks between labels.
    for (let k = 1; k < 5; k++) {
      const minor = frame + (k * stepFrames) / 5;
      if (minor > end) break;
      out.push({ x: xAtFrame(vp, minor), label: null });
    }
  }
  return { mode, ticks: out };
}
