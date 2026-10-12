<script lang="ts">
  import { onMount } from "svelte";
  import { type ClipInfo, type Metadata, type PeakTile, getPeaks } from "../lib/api";
  import { app, setSelection } from "../lib/app.svelte";
  import {
    HANDLE_NAMES,
    type Handle,
    clipChanged,
    clipFrames,
    drag as dragHandle,
    hitTest,
    resetBend,
  } from "../lib/clipHandles";
  import { setClip, setCrossfade } from "../lib/editActions";
  import { formatSeconds } from "../lib/format";
  import { activeTab, overviewCache } from "../lib/tabs.svelte";
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
  import { recordDraw } from "../lib/perf";
  import { PeakLoader } from "../lib/waveform/peakLoader";
  import { drawOverview, drawRuler, drawWaveform } from "../lib/waveform/renderer";
  import { type Theme, readTheme } from "../lib/waveform/theme";

  interface Props {
    /** Backend id of the file shown. The component is remounted when it changes. */
    fileId: number;
    meta: Metadata;
    /** Interpolated playhead frame, or null when there is none to show. */
    playhead: number | null;
    onseek: (frame: number) => void;
  }
  let { fileId, meta, playhead, onseek }: Props = $props();

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
  let destroyed = false;

  // The parent remounts this component when the file changes, so `fileId` is fixed.
  // svelte-ignore state_referenced_locally
  const loader = new PeakLoader(fileId, () => invalidate());

  /** Coalesce redraws into one per animation frame. */
  function invalidate() {
    if (scheduled) return;
    scheduled = true;
    requestAnimationFrame(draw);
  }

  function draw() {
    scheduled = false;
    const vp = view.vp;
    if (destroyed || !vp || !theme || !waveCanvas) return;
    const t0 = performance.now();
    loader.ensure(vp, dpr);
    const scene = {
      vp,
      dpr,
      labels: meta.channel_labels,
      tile: loader.tile,
      selection: app.selection,
      clipRuns: app.clipRuns?.channels ?? null,
      playhead,
      clip: app.clip,
      preview: clipDrag?.preview ?? null,
      label: clipDrag ? { x: clipDrag.px, y: clipDrag.py, text: dragLabel(clipDrag) } : null,
    };
    drawWaveform(waveCanvas.getContext("2d")!, scene, theme);
    drawRuler(rulerCanvas.getContext("2d")!, vp, dpr, meta.sample_rate, theme, playhead);
    drawOverview(overviewCanvas.getContext("2d")!, overviewTile, vp, dpr, theme, playhead);
    recordDraw(performance.now() - t0);
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

  let overviewToken = 0;
  let overviewLoading = false;

  /** Fetch the whole-file overview at the strip's width, reusing the tab's cached one. */
  async function fetchOverview() {
    const columns = Math.max(1, Math.min(16_384, overviewCanvas.width));
    const cached = overviewCache.get(fileId);
    if (cached && cached.columns === columns) {
      overviewTile = cached.tile;
      invalidate();
      return;
    }
    const token = ++overviewToken;
    overviewLoading = true;
    try {
      const tile = await getPeaks(fileId, 0, meta.frames, columns, meta.frames);
      if (token !== overviewToken || destroyed) return;
      overviewTile = tile;
      overviewCache.set(fileId, { columns, tile });
      invalidate();
    } catch {
      // The file was closed meanwhile.
    } finally {
      if (token === overviewToken) overviewLoading = false;
    }
  }

  function layout() {
    dpr = window.devicePixelRatio || 1;
    sizeCanvas(waveCanvas);
    sizeCanvas(rulerCanvas);
    const overviewResized = sizeCanvas(overviewCanvas);
    const width = waveCanvas.getBoundingClientRect().width;
    if (width > 0) {
      // `view.vp` is this tab's saved viewport (restored on activation), or null for a
      // file shown for the first time.
      setViewport(view.vp && view.vp.totalFrames === meta.frames ? resize(view.vp, width) : fit(meta.frames, width));
    }
    if (overviewResized || (!overviewTile && !overviewLoading)) void fetchOverview();
    invalidate();
  }

  // Redraw on anything the scene depends on.
  $effect(() => {
    void view.vp;
    void app.selection;
    void app.clipRuns;
    void app.clip;
    void playhead;
    invalidate();
  });

  onMount(() => {
    // The palette is fixed (always dark), so the colours only need reading once.
    theme = readTheme(root);
    const ro = new ResizeObserver(layout);
    ro.observe(waveArea);
    // Wheel listeners must be non-passive to prevent page scrolling.
    root.addEventListener("wheel", onWheel, { passive: false });
    layout();
    return () => {
      destroyed = true;
      ro.disconnect();
      root.removeEventListener("wheel", onWheel);
      window.removeEventListener("keydown", onClipDragKey, true);
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

  // --- Clip handles: trim edges, end fades and crossfades --------------------------------

  interface ClipDrag {
    /** The clip when the drag began; positions are measured in its view. */
    origin: ClipInfo;
    handle: Handle;
    x0: number;
    preview: ClipInfo;
    /** Pointer, for the label (CSS px). */
    px: number;
    py: number;
  }
  let clipDrag: ClipDrag | null = null;

  function localY(e: { clientY: number }) {
    return e.clientY - waveCanvas.getBoundingClientRect().top;
  }

  /** The handle under the pointer, if the clip can be changed now. */
  function handleAt(e: PointerEvent | MouseEvent): Handle | null {
    const vp = view.vp;
    const tab = activeTab();
    const clip = app.clip;
    if (!vp || !clip || !tab || tab.status !== "ready" || tab.editing) return null;
    const height = waveCanvas.getBoundingClientRect().height;
    return hitTest(clip, vp, height, localX(e, waveCanvas), localY(e));
  }

  function showHover(e: PointerEvent) {
    const h = handleAt(e);
    waveCanvas.style.cursor = !h ? "" : h.kind.endsWith("Bend") ? "ns-resize" : "ew-resize";
    waveCanvas.title = h ? HANDLE_NAMES[h.kind] : "";
  }

  function dragLabel(d: ClipDrag): string {
    const secs = (frames: number) => formatSeconds(frames / meta.sample_rate);
    const p = d.preview;
    switch (d.handle.kind) {
      case "trimStart":
      case "trimEnd":
        return `Length ${secs(clipFrames(p))}`;
      case "fadeIn":
        return `Fade in ${secs(p.fade_in.len)}`;
      case "fadeOut":
        return `Fade out ${secs(p.fade_out.len)}`;
      case "xfadeStart":
      case "xfadeEnd":
        return `Crossfade ${Math.round((p.joins[d.handle.join!].len / meta.sample_rate) * 1000)} ms`;
      default: {
        const bend =
          d.handle.kind === "fadeInBend"
            ? p.fade_in.bend
            : d.handle.kind === "fadeOutBend"
              ? p.fade_out.bend
              : p.joins[d.handle.join!].bend;
        return `Curve ${bend >= 0 ? "+" : "−"}${Math.abs(bend).toFixed(2)}`;
      }
    }
  }

  function startClipDrag(e: PointerEvent, handle: Handle) {
    e.preventDefault();
    waveCanvas.setPointerCapture(e.pointerId);
    const x = localX(e, waveCanvas);
    clipDrag = { origin: app.clip!, handle, x0: x, preview: app.clip!, px: x, py: localY(e) };
    window.addEventListener("keydown", onClipDragKey, true);
    invalidate();
  }

  function moveClipDrag(e: PointerEvent) {
    const vp = view.vp;
    if (!clipDrag || !vp) return;
    const x = localX(e, waveCanvas);
    const y = localY(e);
    const height = waveCanvas.getBoundingClientRect().height;
    clipDrag.preview = dragHandle(clipDrag.origin, clipDrag.handle, vp, height, clipDrag.x0, x, y);
    clipDrag.px = x;
    clipDrag.py = y;
    invalidate();
  }

  function endClipDrag() {
    clipDrag = null;
    window.removeEventListener("keydown", onClipDragKey, true);
    invalidate();
  }

  /** Send the change `preview` makes to `origin`, if any (one undo step). */
  function commitClip(origin: ClipInfo, handle: Handle, preview: ClipInfo) {
    if (handle.join !== undefined) {
      const was = origin.joins[handle.join];
      const now = preview.joins[handle.join];
      if (now.len !== was.len || now.bend !== was.bend) void setCrossfade(now.index, { len: now.len, bend: now.bend });
    } else if (clipChanged(origin, preview)) {
      void setClip(preview);
    }
  }

  function onClipDragKey(e: KeyboardEvent) {
    if (e.key !== "Escape") return;
    // Cancel the drag without also clearing the selection.
    e.preventDefault();
    e.stopPropagation();
    endClipDrag();
  }

  function onLaneDoubleClick(e: MouseEvent) {
    const h = handleAt(e);
    if (h?.kind.endsWith("Bend")) commitClip(app.clip!, h, resetBend(app.clip!, h));
  }

  // --- Selection and seeking on the lanes -------------------------------------------------

  let drag: { x0: number; anchor: number; moved: boolean; shift: boolean } | null = null;

  function onLanePointerDown(e: PointerEvent) {
    const vp = view.vp;
    if (!vp || e.button !== 0) return;
    const handle = handleAt(e);
    if (handle) return startClipDrag(e, handle);
    waveCanvas.setPointerCapture(e.pointerId);
    const x = localX(e, waveCanvas);
    drag = { x0: x, anchor: frameIndexAtX(vp, x), moved: false, shift: e.shiftKey };
  }

  function onLanePointerMove(e: PointerEvent) {
    if (clipDrag) return moveClipDrag(e);
    if (!drag) showHover(e);
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
    if (clipDrag) {
      const { origin, handle, preview } = clipDrag;
      endClipDrag();
      return commitClip(origin, handle, preview);
    }
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
      onpointercancel={() => {
        drag = null;
        if (clipDrag) endClipDrag();
      }}
      ondblclick={onLaneDoubleClick}
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
