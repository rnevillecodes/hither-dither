<script lang="ts">
  import type { Snippet } from "svelte";
  import { app } from "../lib/app.svelte";
  import { formatSeconds } from "../lib/format";
  import { times } from "../lib/selection";
  import { view, zoomIn, zoomOut, zoomToFit } from "../lib/view.svelte";
  import { canZoomIn, canZoomOut } from "../lib/viewport";

  let { transport }: { transport?: Snippet } = $props();

  const sel = $derived(app.selection && app.meta ? times(app.selection, app.meta.sample_rate) : null);
</script>

<header class="toolbar">
  <!-- Files are opened from File › Open…, by dropping them, or from the empty screen. -->
  {#if app.meta}
    <div class="group">{@render transport?.()}</div>

    <div class="group" role="group" aria-label="Zoom">
      <button onclick={zoomOut} disabled={!view.vp || !canZoomOut(view.vp)} title="Zoom out (−)">−</button>
      <button onclick={zoomIn} disabled={!view.vp || !canZoomIn(view.vp)} title="Zoom in (+)">+</button>
      <button onclick={zoomToFit} title="Zoom to fit (F)">Fit</button>
    </div>

    <div class="selection mono" aria-live="polite">
      {#if sel}
        <span><b>Start</b> {formatSeconds(sel.start)}</span>
        <span><b>End</b> {formatSeconds(sel.end)}</span>
        <span><b>Duration</b> {formatSeconds(sel.duration)}</span>
      {:else}
        <span class="dim">No selection. Drag across the waveform to select.</span>
      {/if}
    </div>
  {/if}
</header>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 8px 12px;
    background: var(--panel-bg);
    border-bottom: 1px solid var(--border);
    min-height: 46px;
    flex-wrap: wrap;
  }
  .group {
    display: flex;
    gap: 4px;
    align-items: center;
  }
  .selection {
    display: flex;
    gap: 14px;
    margin-left: auto;
  }
  .selection b {
    font-weight: 600;
    color: var(--text-dim);
    margin-right: 4px;
  }
  .dim {
    color: var(--text-dim);
  }
</style>
