// Scroll benchmark for checking redraw rate (Ctrl/Cmd+Shift+B). Scrolls continuously
// through the file for a few seconds and reports frames per second and draw times.
import { notify } from "./notices.svelte";
import { setViewport, view } from "./view.svelte";
import { scrollTo, visibleEnd } from "./viewport";

const DURATION_MS = 5000;
const drawTimes: number[] = [];

/** Called by the renderer with the time each waveform draw took. */
export function recordDraw(ms: number) {
  if (running) drawTimes.push(ms);
}

let running = false;

export function runScrollBenchmark() {
  const vp0 = view.vp;
  if (!vp0 || running) return;
  running = true;
  drawTimes.length = 0;
  // About ten seconds of audio per screen, like reading through a long file.
  const span = Math.min(vp0.totalFrames / 4, 48_000 * 10);
  let vp = scrollTo({ ...vp0, fpp: span / vp0.width }, 0);
  const perFrame = vp.width * vp.fpp * 0.05;
  const frames: number[] = [];
  const t0 = performance.now();
  const step = (t: number) => {
    frames.push(t);
    if (t - t0 >= DURATION_MS) {
      running = false;
      report(frames);
      return;
    }
    vp = visibleEnd(vp) >= vp.totalFrames ? scrollTo(vp, 0) : scrollTo(vp, vp.start + perFrame);
    setViewport(vp);
    requestAnimationFrame(step);
  };
  requestAnimationFrame(step);
}

function report(frames: number[]) {
  const intervals = frames.slice(1).map((t, i) => t - frames[i]);
  const fps = (intervals.length * 1000) / (frames[frames.length - 1] - frames[0]);
  const sorted = [...intervals].sort((a, b) => a - b);
  const p95 = sorted[Math.floor(sorted.length * 0.95)] ?? 0;
  const avgDraw = drawTimes.reduce((a, b) => a + b, 0) / Math.max(1, drawTimes.length);
  const maxDraw = Math.max(0, ...drawTimes);
  const body = `${fps.toFixed(1)} fps · 95th percentile frame ${p95.toFixed(1)} ms · draw avg ${avgDraw.toFixed(2)} ms, max ${maxDraw.toFixed(1)} ms (${drawTimes.length} draws)`;
  console.info("[scroll benchmark]", body);
  notify(fps >= 30 ? "info" : "warning", "Scroll benchmark", body);
}
