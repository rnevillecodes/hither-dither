<script lang="ts">
  import { onMount } from "svelte";
  import { type Metadata, type PeakTile, getPeaks } from "../lib/api";
  import { app, setSelection } from "../lib/app.svelte";
  import { DRAG_THRESHOLD_PX, between, extend } from "../lib/selection";
  import { view, setViewport } from "../lib/view.svelte";
  import {
    fit,
    frameIndexAtX,
    resize,
    scrollByPixels,
    scrollTo,
    visibleEnd,
    zoomAround,
  } from "../lib/viewport";
  import { PeakLoader } from "../lib/waveform/peakLoader";
  import { drawOverview, drawRuler, drawWaveform } from "../lib/waveform/renderer";
  import { type Theme, readTheme } from "../lib/waveform/theme";

  interface Props {
    meta: Metadata;
    /** Interpolated playhead frame, or null when there is none to show. */
    playhead: number | null;
    onseek: (frame: number) => void;
  }
  let { meta, playhead, onseek }: Props = $props();

  let root: HTMLDivElement;
  let waveArea: HTMLDivElement;
  let rulerCanvas: HTMLCanvasElement;
  let waveCanvas: HTMLCanvasElement;
  let overviewCanvas: HTMLCanvasElement;
  let track: HTMLDivElement;

  let dpr = window.devicePixelRatio || 1;
  let theme: Theme | null = null;
  let overviewTile: PeakTile | null = null;
  let scheduled = false;

  const loader = new PeakLoader(() => invalidate());

  /** Coalesce redraws into one per animation frame. */
  function invalidate() {
    if (scheduled) return;
    scheduled = true;
    requestAnimationFrame(draw);
  }

  function draw() {
    scheduled = false;
    const vp = view.vp;
    if (!vp || !theme || !waveCanvas) return;
    loader.ensure(vp, dpr);
    const scene = {
      vp,
      dpr,
      labels: meta.channel_labels,
      tile: loader.tile,
      selection: app.selection,
      clipRuns: app.clipRuns?.channels ?? null,
      playhead,
    };
    drawWaveform(waveCanvas.getContext("2d")!, scene, theme);
    drawRuler(rulerCanvas.getContext("2d")!, vp, dpr, meta.sample_rate, theme, playhead);
    drawOverview(overviewCanvas.getContext("2d")!, overviewTile, vp, dpr, theme, playhead);
  }

  function sizeCanvas(c: HTMLCanvasElement) {
    const r = c.getBoundingClientRect();
    const w = Math.max(1, Math.round(r.width * dpr));
    const h = Math.max(1, Math.round(r.height * dpr));
    if (c.width !== w || c.height !== h) {
      c.width = w;
      c.height = h;
      return true;
    }
    return false;
  }

  async function fetchOverview() {
    const gen = app.generation;
    const columns = Math.max(1, Math.min(16_384, overviewCanvas.width));
    const tile = await getPeaks(0, meta.frames, columns, meta.frames);
    if (gen !== app.generation) return;
    overviewTile = tile;
    invalidate();
  }

  function layout() {
    dpr = window.devicePixelRatio || 1;
    sizeCanvas(waveCanvas);
    sizeCanvas(rulerCanvas);
    const overviewResized = sizeCanvas(overviewCanvas);
    const width = waveCanvas.getBoundingClientRect().width;
    if (width > 0) {
      setViewport(view.vp && view.vp.totalFrames === meta.frames ? resize(view.vp, width) : fit(meta.frames, width));
    }
    if (overviewResized) void fetchOverview();
    invalidate();
  }

  // Reset the view whenever a new file loads.
  let lastGeneration = -1;
  $effect(() => {
    const gen = app.generation;
    if (gen === lastGeneration || !waveCanvas) return;
    lastGeneration = gen;
    loader.reset();
    overviewTile = null;
    const width = waveCanvas.getBoundingClientRect().width;
    setViewport(fit(meta.frames, Math.max(1, width)));
    view.userScrolled = false;
    void fetchOverview();
  });

  // Redraw on anything the scene depends on.
  $effect(() => {
    void view.vp;
    void app.selection;
    void app.clipRuns;
    void playhead;
    invalidate();
  });

  onMount(() => {
    theme = readTheme(root);
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onTheme = () => {
      theme = readTheme(root);
      invalidate();
    };
    media.addEventListener("change", onTheme);
    const ro = new ResizeObserver(layout);
    ro.observe(waveArea);
    // Wheel listeners must be non-passive to prevent page scrolling.
    root.addEventListener("wheel", onWheel, { passive: false });
    layout();
    return () => {
      media.removeEventListener("change", onTheme);
      ro.disconnect();
      root.removeEventListener("wheel", onWheel);
    };
  });

  function localX(e: { clientX: number }, el: HTMLElement) {
    return e.clientX - el.getBoundingClientRect().left;
  }

  function onWheel(e: WheelEvent) {
    const vp = view.vp;
    if (!vp) return;
    e.preventDefault();
    const unit = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? vp.width : 1;
    if (e.ctrlKey || e.metaKey) {
      const factor = Math.exp(-e.deltaY * unit * 0.0025);
      setViewport(zoomAround(vp, factor, localX(e, waveCanvas)));
    } else {
      const dx = e.shiftKey
        ? e.deltaY || e.deltaX
        : Math.abs(e.deltaX) > Math.abs(e.deltaY)
          ? e.deltaX
          : e.deltaY;
      setViewport(scrollByPixels(vp, dx * unit), true);
    }
  }

  // --- Selection and seeking on the lanes -------------------------------------------------

  let drag: { x0: number; anchor: number; moved: boolean; shift: boolean } | null = null;

  function onLanePointerDown(e: PointerEvent) {
    const vp = view.vp;
    if (!vp || e.button !== 0) return;
    waveCanvas.setPointerCapture(e.pointerId);
    const x = localX(e, waveCanvas);
    drag = { x0: x, anchor: frameIndexAtX(vp, x), moved: false, shift: e.shiftKey };
  }

  function onLanePointerMove(e: PointerEvent) {
    const vp = view.vp;
    if (!drag || !vp || drag.shift) return;
    const x = localX(e, waveCanvas);
    if (!drag.moved && Math.abs(x - drag.x0) < DRAG_THRESHOLD_PX) return;
    drag.moved = true;
    app.selection = between(drag.anchor, frameIndexAtX(vp, x));
    // Auto-scroll while dragging past either edge.
    if (x < 0 || x > vp.width) {
      const dx = x < 0 ? x : x - vp.width;
      setViewport(scrollByPixels(vp, dx / 4), true);
    }
  }

  function onLanePointerUp(e: PointerEvent) {
    const vp = view.vp;
    if (!drag || !vp) return;
    const frame = frameIndexAtX(vp, localX(e, waveCanvas));
    if (drag.shift) {
      void setSelection(extend(app.selection, frame, playhead ?? 0));
    } else if (drag.moved) {
      void setSelection(between(drag.anchor, frame));
    } else {
      void setSelection(null);
      onseek(frame);
    }
    drag = null;
  }

  function onRulerPointerDown(e: PointerEvent) {
    const vp = view.vp;
    if (vp && e.button === 0) onseek(frameIndexAtX(vp, localX(e, rulerCanvas)));
  }

  // --- Overview strip and scrollbar -------------------------------------------------------

  let overviewDragging = false;

  function centreOn(e: PointerEvent) {
    const vp = view.vp;
    if (!vp) return;
    const r = overviewCanvas.getBoundingClientRect();
    const frame = ((e.clientX - r.left) / r.width) * vp.totalFrames;
    setViewport(scrollTo(vp, frame - (vp.width * vp.fpp) / 2), true);
  }

  function onOverviewDown(e: PointerEvent) {
    if (e.button !== 0) return;
    overviewDragging = true;
    overviewCanvas.setPointerCapture(e.pointerId);
    centreOn(e);
  }

  let thumbDrag: { x0: number; start0: number } | null = null;

  const thumb = $derived.by(() => {
    const vp = view.vp;
    if (!vp || vp.totalFrames === 0) return { left: 0, width: 100 };
    return {
      left: (vp.start / vp.totalFrames) * 100,
      width: ((visibleEnd(vp) - vp.start) / vp.totalFrames) * 100,
    };
  });

  function onThumbDown(e: PointerEvent) {
    if (!view.vp || e.button !== 0) return;
    e.stopPropagation();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    thumbDrag = { x0: e.clientX, start0: view.vp.start };
  }

  function onThumbMove(e: PointerEvent) {
    const vp = view.vp;
    if (!thumbDrag || !vp) return;
    const framesPerPx = vp.totalFrames / track.getBoundingClientRect().width;
    setViewport(scrollTo(vp, thumbDrag.start0 + (e.clientX - thumbDrag.x0) * framesPerPx), true);
  }

  function onTrackDown(e: PointerEvent) {
    const vp = view.vp;
    if (!vp || e.button !== 0) return;
    // Page towards the click.
    const r = track.getBoundingClientRect();
    const clickFrame = ((e.clientX - r.left) / r.width) * vp.totalFrames;
    const page = vp.width * vp.fpp * 0.9;
    setViewport(scrollTo(vp, vp.start + (clickFrame < vp.start ? -page : page)), true);
  }
</script>

<div class="waveform" bind:this={root}>
  <canvas class="ruler" bind:this={rulerCanvas} onpointerdown={onRulerPointerDown}></canvas>
  <div class="lanes" bind:this={waveArea}>
    <canvas
      class="wave"
      bind:this={waveCanvas}
      onpointerdown={onLanePointerDown}
      onpointermove={onLanePointerMove}
      onpointerup={onLanePointerUp}
      onpointercancel={() => (drag = null)}
    ></canvas>
  </div>
  <canvas
    class="overview"
    bind:this={overviewCanvas}
    onpointerdown={onOverviewDown}
    onpointermove={(e) => overviewDragging && centreOn(e)}
    onpointerup={() => (overviewDragging = false)}
    title="Overview: click or drag to scroll"
  ></canvas>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="track" bind:this={track} onpointerdown={onTrackDown}>
    <div
      class="thumb"
      style:left="{thumb.left}%"
      style:width="max(24px, {thumb.width}%)"
      onpointerdown={onThumbDown}
      onpointermove={onThumbMove}
      onpointerup={() => (thumbDrag = null)}
    ></div>
  </div>
</div>

<style>
  .waveform {
    display: grid;
    grid-template-rows: 26px 1fr 40px 12px;
    height: 100%;
    min-height: 0;
    user-select: none;
  }
  canvas {
    display: block;
    width: 100%;
    height: 100%;
  }
  .ruler {
    cursor: pointer;
    background: var(--panel-bg);
  }
  .lanes {
    min-height: 0;
    position: relative;
  }
  .wave {
    position: absolute;
    inset: 0;
    cursor: text;
    touch-action: none;
  }
  .overview {
    border-top: 1px solid var(--border);
    cursor: grab;
    touch-action: none;
  }
  .track {
    position: relative;
    background: var(--panel-bg);
    border-top: 1px solid var(--border);
  }
  .thumb {
    position: absolute;
    top: 2px;
    bottom: 2px;
    border-radius: 4px;
    background: var(--scroll-thumb);
    touch-action: none;
  }
  .thumb:hover {
    background: var(--scroll-thumb-hover);
  }
</style>
