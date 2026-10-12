<script lang="ts">
  import { bendFromPoint, bendPointHeight, gain } from "../lib/curve";
  import { closeCurveDialog, curveDialog } from "../lib/curveDialog.svelte";

  // Graph size and inset (CSS px).
  const W = 160;
  const H = 100;
  const PAD = 8;

  let dialog = $state<HTMLDivElement>();
  let svg = $state<SVGSVGElement>();
  let dragging = false;

  const title = $derived(curveDialog.dir === "in" ? "Fade In" : "Fade Out");
  const gx = (t: number) => PAD + t * (W - 2 * PAD);
  const gy = (g: number) => PAD + (1 - g) * (H - 2 * PAD);
  const path = $derived.by(() => {
    const points: string[] = [];
    for (let i = 0; i <= 48; i++) {
      const t = i / 48;
      points.push(`${gx(t).toFixed(1)},${gy(gain(curveDialog.dir, curveDialog.bend, t)).toFixed(1)}`);
    }
    return `M${points.join("L")}`;
  });
  const point = $derived({ x: gx(0.5), y: gy(bendPointHeight(curveDialog.bend)) });
  const bendText = $derived(
    curveDialog.bend === 0 ? "Linear" : `${curveDialog.bend > 0 ? "+" : "−"}${Math.abs(curveDialog.bend).toFixed(2)}`,
  );

  // Focus the dialog's first button when it opens.
  $effect(() => {
    if (curveDialog.open) queueMicrotask(() => dialog?.querySelector<HTMLElement>("button.primary")?.focus());
  });

  function setFromPointer(e: PointerEvent) {
    if (!svg) return;
    const r = svg.getBoundingClientRect();
    const y = ((e.clientY - r.top) / r.height) * H;
    curveDialog.bend = bendFromPoint(1 - (y - PAD) / (H - 2 * PAD));
  }

  function onPointerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    dragging = true;
    svg?.setPointerCapture(e.pointerId);
    setFromPointer(e);
  }

  function onKeydown(e: KeyboardEvent) {
    // Keep app shortcuts (Space, Delete…) from acting behind the dialog.
    e.stopPropagation();
    if (e.key === "Escape") {
      e.preventDefault();
      closeCurveDialog(null);
    } else if (e.key === "Enter" && (e.target as HTMLElement).tagName !== "BUTTON") {
      e.preventDefault();
      closeCurveDialog(curveDialog.bend);
    } else if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      e.preventDefault();
      const step = e.key === "ArrowUp" ? 0.05 : -0.05;
      curveDialog.bend = Math.max(-1, Math.min(1, Math.round((curveDialog.bend + step) * 100) / 100));
    } else if (e.key === "Tab") {
      // Keep focus inside the dialog.
      const items = [...(dialog?.querySelectorAll<HTMLElement>("button, [tabindex='0']") ?? [])];
      const i = items.indexOf(document.activeElement as HTMLElement);
      const next = (i + (e.shiftKey ? -1 : 1) + items.length) % items.length;
      e.preventDefault();
      items[next]?.focus();
    }
  }
</script>

{#if curveDialog.open}
  <div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && closeCurveDialog(null)}>
    <div
      class="dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="curve-title"
      tabindex="-1"
      bind:this={dialog}
      onkeydown={onKeydown}
    >
      <h2 id="curve-title">{title}</h2>
      <p class="hint">Drag the point to bend the curve. Double-click it for linear.</p>
      <svg
        bind:this={svg}
        viewBox="0 0 {W} {H}"
        width={W * 1.5}
        height={H * 1.5}
        role="slider"
        tabindex="0"
        aria-label="Fade curve"
        aria-valuemin={-1}
        aria-valuemax={1}
        aria-valuenow={curveDialog.bend}
        aria-valuetext={bendText}
        onpointerdown={onPointerDown}
        onpointermove={(e) => dragging && setFromPointer(e)}
        onpointerup={() => (dragging = false)}
        ondblclick={() => (curveDialog.bend = 0)}
      >
        <rect x={PAD} y={PAD} width={W - 2 * PAD} height={H - 2 * PAD} class="frame" />
        <line x1={gx(0)} y1={gy(curveDialog.dir === "in" ? 0 : 1)} x2={gx(1)} y2={gy(curveDialog.dir === "in" ? 1 : 0)} class="linear" />
        <path d={path} class="curve" />
        <circle cx={point.x} cy={point.y} r="4.5" class="point" />
      </svg>
      <div class="value mono">{bendText}</div>
      <div class="buttons">
        <button onclick={() => closeCurveDialog(null)}>Cancel</button>
        <button class="primary" onclick={() => closeCurveDialog(curveDialog.bend)}>Apply</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: color-mix(in srgb, var(--bg) 60%, transparent);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 40;
  }
  .dialog {
    background: var(--panel-bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 16px 18px;
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.45);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }
  .dialog:focus {
    outline: none;
  }
  h2 {
    margin: 0;
    font-size: 15px;
    align-self: flex-start;
  }
  .hint {
    margin: 0;
    color: var(--text-dim);
    font-size: 12px;
    align-self: flex-start;
  }
  svg {
    cursor: ns-resize;
    touch-action: none;
    border-radius: 4px;
  }
  svg:focus-visible {
    outline: 2px solid var(--accent);
  }
  .frame {
    fill: var(--bg);
    stroke: var(--border);
  }
  .linear {
    stroke: var(--text-dim);
    stroke-dasharray: 3 3;
    opacity: 0.6;
  }
  .curve {
    fill: none;
    stroke: var(--accent);
    stroke-width: 2;
  }
  .point {
    fill: var(--accent);
  }
  .value {
    color: var(--text-dim);
  }
  .buttons {
    display: flex;
    gap: 8px;
    align-self: flex-end;
  }
</style>
