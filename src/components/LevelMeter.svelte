<script lang="ts">
  import {
    atRest,
    clearReadouts,
    createMeter,
    formatReadout,
    levelToFraction,
    SCALE_MARKS,
    stepBallistics,
    WARN_DB,
    type ChannelMeter,
  } from "../lib/meter";
  import { playbackStarts, takeLevels } from "../lib/meterFeed";
  import { transport } from "../lib/transport.svelte";

  /** The active file's channel labels, one bar each. */
  let { labels }: { labels: string[] } = $props();

  /** Below this width the meter runs across the window (matches App.svelte). */
  const NARROW = "(max-width: 820px)";
  /** Readouts and clip lights are refreshed at most this often. */
  const READOUT_INTERVAL_MS = 100;
  const SCALE_W = 24;
  const COLUMN = 30;
  const PEAK_W = 12;
  const RMS_W = 6;
  const PAD = 4;
  /** Narrow layout: height of each channel's bar, and of the scale along the bottom. */
  const ROW = 12;
  const SCALE_H = 14;

  // Not reactive: updated every animation frame.
  // svelte-ignore state_referenced_locally
  const meter: ChannelMeter[] = createMeter(labels.length);
  let starts = playbackStarts();

  let shown = $state(readouts());
  let horizontal = $state(window.matchMedia(NARROW).matches);
  let canvas = $state<HTMLCanvasElement>();
  let colours: Record<"accent" | "warning" | "track" | "rms" | "dim" | "grid", string> | null = null;

  function readouts() {
    return meter.map((m) => ({ max: formatReadout(m.max), clip: m.clip, silent: m.max === -Infinity }));
  }

  $effect(() => {
    const query = window.matchMedia(NARROW);
    const update = () => (horizontal = query.matches);
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  });

  function readColours(el: Element) {
    const style = getComputedStyle(el);
    const v = (name: string) => style.getPropertyValue(name).trim();
    colours = {
      accent: v("--accent"),
      warning: v("--warning"),
      track: v("--wf-lane-bg"),
      rms: v("--wf-wave"),
      dim: v("--text-dim"),
      grid: v("--wf-grid"),
    };
  }

  function draw() {
    const el = canvas;
    if (!el || !colours) return;
    const dpr = window.devicePixelRatio || 1;
    const w = el.clientWidth;
    const h = el.clientHeight;
    if (el.width !== Math.round(w * dpr) || el.height !== Math.round(h * dpr)) {
      el.width = Math.round(w * dpr);
      el.height = Math.round(h * dpr);
    }
    const ctx = el.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    ctx.font = "9px system-ui, sans-serif";
    if (horizontal) drawAcross(ctx, w, h, colours);
    else drawUp(ctx, w, h, colours);
  }

  /** Vertical meter: scale on the left, a column per channel, rising upwards. */
  function drawUp(ctx: CanvasRenderingContext2D, _w: number, h: number, c: NonNullable<typeof colours>) {
    const top = PAD;
    const len = Math.max(1, h - 2 * PAD);
    const y = (f: number) => top + len * (1 - f);
    ctx.textAlign = "right";
    ctx.textBaseline = "middle";
    for (const db of SCALE_MARKS) {
      const at = Math.round(y(levelToFraction(db))) + 0.5;
      ctx.fillStyle = c.dim;
      ctx.fillText(String(db), SCALE_W - 5, Math.min(Math.max(at, top + 4), top + len - 4));
      ctx.fillStyle = c.grid;
      ctx.fillRect(SCALE_W - 3, at - 0.5, 3, 1);
    }
    const warnY = y(levelToFraction(WARN_DB));
    meter.forEach((m, i) => {
      const x = SCALE_W + i * COLUMN + (COLUMN - PEAK_W) / 2;
      ctx.fillStyle = c.track;
      ctx.fillRect(x, top, PEAK_W, len);
      const peakY = y(levelToFraction(m.peak));
      if (peakY < top + len) {
        ctx.fillStyle = c.accent;
        ctx.fillRect(x, Math.max(peakY, warnY), PEAK_W, top + len - Math.max(peakY, warnY));
        if (peakY < warnY) {
          ctx.fillStyle = c.warning;
          ctx.fillRect(x, peakY, PEAK_W, warnY - peakY);
        }
      }
      const rmsY = y(levelToFraction(m.rms));
      ctx.fillStyle = c.rms;
      ctx.fillRect(x + (PEAK_W - RMS_W) / 2, rmsY, RMS_W, top + len - rmsY);
      if (m.hold !== -Infinity && levelToFraction(m.hold) > 0) {
        ctx.fillStyle = m.hold >= WARN_DB ? c.warning : c.accent;
        ctx.fillRect(x, Math.round(y(levelToFraction(m.hold))) - 1, PEAK_W, 2);
      }
    });
  }

  /** Horizontal meter: a row per channel, rising to the right, scale along the bottom. */
  function drawAcross(ctx: CanvasRenderingContext2D, w: number, h: number, c: NonNullable<typeof colours>) {
    const left = PAD;
    const len = Math.max(1, w - 2 * PAD);
    const x = (f: number) => left + len * f;
    const rows = h - SCALE_H;
    const step = rows / meter.length;
    const warnX = x(levelToFraction(WARN_DB));
    meter.forEach((m, i) => {
      const yTop = i * step + (step - ROW) / 2;
      ctx.fillStyle = c.track;
      ctx.fillRect(left, yTop, len, ROW);
      const peakX = x(levelToFraction(m.peak));
      if (peakX > left) {
        ctx.fillStyle = c.accent;
        ctx.fillRect(left, yTop, Math.min(peakX, warnX) - left, ROW);
        if (peakX > warnX) {
          ctx.fillStyle = c.warning;
          ctx.fillRect(warnX, yTop, peakX - warnX, ROW);
        }
      }
      const rmsX = x(levelToFraction(m.rms));
      ctx.fillStyle = c.rms;
      ctx.fillRect(left, yTop + (ROW - RMS_W) / 2, rmsX - left, RMS_W);
      if (m.hold !== -Infinity && levelToFraction(m.hold) > 0) {
        ctx.fillStyle = m.hold >= WARN_DB ? c.warning : c.accent;
        ctx.fillRect(Math.round(x(levelToFraction(m.hold))) - 1, yTop, 2, ROW);
      }
    });
    ctx.textAlign = "center";
    ctx.textBaseline = "bottom";
    for (const db of SCALE_MARKS) {
      const at = Math.round(x(levelToFraction(db))) + 0.5;
      ctx.fillStyle = c.grid;
      ctx.fillRect(at - 0.5, rows, 1, 3);
      ctx.fillStyle = c.dim;
      ctx.textAlign = db === 0 ? "right" : db === -60 ? "left" : "center";
      ctx.fillText(String(db), at, h);
    }
  }

  // Animate while playing, and until the bars have fallen after playback stops.
  let frame = 0;
  let last = 0;
  let lastReadout = 0;

  function tick(now: number) {
    const dt = last ? Math.min((now - last) / 1000, 0.1) : 0;
    last = now;
    const started = playbackStarts();
    if (started !== starts) {
      starts = started;
      clearReadouts(meter);
    }
    const input = takeLevels();
    stepBallistics(meter, input && input.length === meter.length ? input : null, dt, transport.playing);
    draw();
    if (now - lastReadout >= READOUT_INTERVAL_MS) {
      lastReadout = now;
      const next = readouts();
      if (next.some((r, i) => r.max !== shown[i].max || r.clip !== shown[i].clip)) shown = next;
    }
    if (transport.playing || !atRest(meter)) {
      frame = requestAnimationFrame(tick);
    } else {
      frame = 0;
      last = 0;
      shown = readouts();
    }
  }

  function animate() {
    if (!frame) frame = requestAnimationFrame(tick);
  }

  $effect(() => {
    if (transport.playing) animate();
  });

  $effect(() => {
    const el = canvas;
    if (!el) return;
    readColours(el);
    void horizontal;
    draw();
    const ro = new ResizeObserver(() => draw());
    ro.observe(el);
    return () => ro.disconnect();
  });

  $effect(() => () => {
    if (frame) cancelAnimationFrame(frame);
  });

  function clear() {
    clearReadouts(meter);
    shown = readouts();
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key !== "Enter" && e.key !== " ") return;
    e.preventDefault();
    // Space would otherwise also toggle playback.
    e.stopPropagation();
    clear();
  }
</script>

<!-- Clicking anywhere on the meter clears its readouts; its readouts stay readable as text. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="meter"
  class:horizontal
  role="group"
  aria-label="Level meter"
  aria-describedby="level-meter-hint"
  title="Click to clear the peak readouts and clip lights"
  tabindex="0"
  style:--channels={labels.length}
  onclick={clear}
  onkeydown={onKeydown}
>
  <span id="level-meter-hint" class="sr-only">Press Enter to clear the peak readouts and clip lights.</span>
  <div class="readouts">
    {#each labels as label, i (i)}
      <div class="channel">
        <span class="clip" class:on={shown[i]?.clip} aria-hidden="true"></span>
        <span
          class="max mono"
          class:silent={shown[i]?.silent}
          aria-label="{label} maximum peak {shown[i]?.silent ? 'silent' : `${shown[i]?.max} dBFS`}{shown[i]?.clip
            ? ', clipped'
            : ''}">{shown[i]?.max}</span
        >
      </div>
    {/each}
  </div>
  <canvas bind:this={canvas} aria-hidden="true"></canvas>
  <div class="labels" aria-hidden="true">
    {#each labels as label, i (i)}<span>{label}</span>{/each}
  </div>
</div>

<style>
  .meter {
    --scale: 24px;
    --column: 30px;
    display: flex;
    flex-direction: column;
    width: calc(var(--scale) + var(--channels) * var(--column));
    min-height: 0;
    padding: 6px 0;
    border-left: 1px solid var(--border);
    background: var(--panel-bg);
    cursor: pointer;
    user-select: none;
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
  .meter:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .readouts,
  .labels {
    display: grid;
    grid-template-columns: repeat(var(--channels), var(--column));
    padding-left: var(--scale);
  }
  .channel {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
  }
  .clip {
    width: 12px;
    height: 6px;
    border-radius: 1px;
    background: var(--border);
  }
  .clip.on {
    background: var(--danger);
  }
  .max {
    font-size: 10px;
    letter-spacing: -0.02em;
    color: var(--text);
  }
  .max.silent {
    color: var(--text-dim);
  }
  canvas {
    flex: 1;
    min-height: 0;
    width: 100%;
    display: block;
  }
  .labels span {
    text-align: center;
    font-size: 10px;
    color: var(--text-dim);
  }

  /* Narrow window: rows across, labels at the left, readouts at the right. */
  .meter.horizontal {
    --row: 16px;
    display: grid;
    grid-template-columns: 22px 1fr 44px;
    grid-template-rows: repeat(var(--channels), var(--row)) 14px;
    width: auto;
    padding: 4px 8px;
    border-left: none;
    border-top: 1px solid var(--border);
  }
  .horizontal .labels,
  .horizontal .readouts {
    grid-template-columns: none;
    grid-template-rows: repeat(var(--channels), var(--row));
    padding-left: 0;
    align-items: center;
  }
  .horizontal .labels {
    grid-column: 1;
    grid-row: 1 / span var(--channels);
  }
  .horizontal canvas {
    grid-column: 2;
    grid-row: 1 / -1;
    height: 100%;
  }
  .horizontal .readouts {
    grid-column: 3;
    grid-row: 1 / span var(--channels);
  }
  .horizontal .channel {
    flex-direction: row-reverse;
    justify-content: flex-end;
    gap: 6px;
    padding-left: 6px;
  }
  .horizontal .clip {
    width: 6px;
    height: 10px;
  }
</style>
