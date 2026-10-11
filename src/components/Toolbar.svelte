<script lang="ts">
  import type { Snippet } from "svelte";
  import { layout, toggleDetails } from "../lib/layout.svelte";
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

  <!-- The toolbar is only shown while tabs exist. -->
  <button
    class="panel-toggle"
    class:active={layout.detailsOpen}
    onclick={toggleDetails}
    aria-pressed={layout.detailsOpen}
    aria-controls="details-panel"
    aria-label="Details panel"
    title={layout.detailsOpen ? "Hide details panel" : "Show details panel"}
  >
    <svg viewBox="0 0 16 16" aria-hidden="true">
      <rect x="1.5" y="2.5" width="13" height="11" rx="1.5" />
      <path d="M10 2.5v11" />
      <path class="fill" d="M10 3h3.5a.5.5 0 0 1 .5.5v9a.5.5 0 0 1-.5.5H10z" />
    </svg>
  </button>
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
  .panel-toggle {
    display: flex;
    padding: 5px 7px;
    margin-left: auto;
  }
  .selection ~ .panel-toggle {
    margin-left: 0;
  }
  .panel-toggle svg {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.2;
  }
  .panel-toggle .fill {
    fill: currentColor;
    stroke: none;
    opacity: 0;
  }
  .panel-toggle.active .fill {
    opacity: 0.45;
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
