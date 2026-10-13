<script lang="ts">
  import { tick } from "svelte";
  import { MAX_EFFECTS, type EffectInfo } from "../lib/api";
  import { dropGap, dropIndex, stepIndex } from "../lib/chainOrder";
  import { canAddEffect, moveEffect, removeEffect, setBypassAll, setEffectBypass } from "../lib/editActions";
  import { closeControls, closeGeneric, editorOpen, effectsUi, showControls } from "../lib/effects.svelte";
  import type { Tab } from "../lib/tabs.svelte";

  let { tab }: { tab: Tab } = $props();

  /** Vertical movement (px) before a press on a row becomes a drag. */
  const DRAG_THRESHOLD = 4;

  const effects = $derived(tab.chain?.effects ?? []);
  const bypassAll = $derived(tab.chain?.bypassAll ?? false);
  const full = $derived(effects.length >= MAX_EFFECTS);
  const busy = $derived(tab.editing);
  /** Some effect changes what is heard. */
  const active = $derived(!bypassAll && effects.some((e) => !e.bypassed));

  let list = $state<HTMLOListElement>();

  // Dragging a row to reorder, with pointer events as in the tab strip.
  let dragSlot = $state<number | null>(null);
  let gap = $state<number | null>(null);
  let markerY = $state(0);
  let press: { slotId: number; pointerId: number; startY: number; y: number } | null = null;

  function onPointerDown(e: PointerEvent, slotId: number) {
    if (e.button !== 0 || busy || (e.target as HTMLElement).closest("button")) return;
    press = { slotId, pointerId: e.pointerId, startY: e.clientY, y: e.clientY };
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    window.addEventListener("pointercancel", endDrag);
    window.addEventListener("keydown", onDragKey, true);
  }

  function onPointerMove(e: PointerEvent) {
    if (!press || e.pointerId !== press.pointerId || !list) return;
    press.y = e.clientY;
    if (dragSlot === null) {
      if (Math.abs(e.clientY - press.startY) < DRAG_THRESHOLD) return;
      dragSlot = press.slotId;
      list.setPointerCapture(e.pointerId);
    }
    e.preventDefault();
    const rows = [...list.querySelectorAll<HTMLElement>("[data-slot-id]")].map((el) => el.getBoundingClientRect());
    if (rows.length === 0) return;
    gap = dropGap(
      rows.map((r) => r.top + r.height / 2),
      press.y,
    );
    const box = list.getBoundingClientRect();
    markerY = (gap < rows.length ? rows[gap].top - 3 : rows[rows.length - 1].bottom + 3) - box.top;
  }

  function onPointerUp(e: PointerEvent) {
    if (!press || e.pointerId !== press.pointerId) return;
    if (dragSlot !== null && gap !== null) {
      const from = effects.findIndex((fx) => fx.slotId === dragSlot);
      const index = dropIndex(from, gap, effects.length);
      if (index !== null) void moveEffect(dragSlot, index);
    }
    endDrag();
  }

  function onDragKey(e: KeyboardEvent) {
    if (e.key === "Escape" && dragSlot !== null) {
      e.stopPropagation();
      e.preventDefault();
      endDrag();
    }
  }

  function endDrag() {
    if (press && list?.hasPointerCapture(press.pointerId)) list.releasePointerCapture(press.pointerId);
    press = null;
    dragSlot = null;
    gap = null;
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
    window.removeEventListener("pointercancel", endDrag);
    window.removeEventListener("keydown", onDragKey, true);
  }

  $effect(() => endDrag);

  /** Alt+Up / Alt+Down move the focused effect, which keeps the focus. */
  async function onRowKey(e: KeyboardEvent, slotId: number, index: number) {
    if (!e.altKey || (e.key !== "ArrowUp" && e.key !== "ArrowDown") || busy) return;
    e.preventDefault();
    const to = stepIndex(index, e.key === "ArrowUp" ? -1 : 1, effects.length);
    if (to === null) return;
    await moveEffect(slotId, to);
    await tick();
    list?.querySelector<HTMLElement>(`[data-slot-id="${slotId}"]`)?.focus();
  }

  function toggleControls(fx: EffectInfo) {
    if (editorOpen(tab.id, fx.slotId)) closeControls(tab.id, fx.slotId);
    else void showControls(tab.id, fx);
  }
</script>

<section aria-labelledby="effects-heading">
  <header>
    <h2 id="effects-heading">Effects</h2>
    {#if effects.length > 0}
      <button
        class="small"
        class:active={bypassAll}
        aria-pressed={bypassAll}
        disabled={busy}
        title="Hear the audio without any effects"
        onclick={() => void setBypassAll(!bypassAll)}>Bypass All</button
      >
    {/if}
  </header>

  {#if effects.length === 0}
    <p class="dim">Add effects to hear them as you play. They're written into the audio when you export.</p>
  {:else}
    <div class="chain-wrap">
      <ol class="chain" class:dragging={dragSlot !== null} bind:this={list} aria-label="Effects in processing order">
        {#each effects as fx, i (fx.slotId)}
          {@const open = editorOpen(tab.id, fx.slotId)}
          <!-- A row takes the focus so Alt+Up/Down can move it; its buttons do the rest. -->
          <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
          <li
            class="effect"
            class:bypassed={fx.bypassed || bypassAll}
            class:dragged={fx.slotId === dragSlot}
            data-slot-id={fx.slotId}
            tabindex="0"
            aria-label="{fx.name}{fx.bypassed ? ', bypassed' : ''}. Alt+Up or Alt+Down to move."
            onpointerdown={(e) => onPointerDown(e, fx.slotId)}
            onkeydown={(e) => void onRowKey(e, fx.slotId, i)}
          >
            <svg class="grip" viewBox="0 0 6 10" aria-hidden="true">
              <circle cx="1" cy="1" r="1" /><circle cx="5" cy="1" r="1" /><circle cx="1" cy="5" r="1" />
              <circle cx="5" cy="5" r="1" /><circle cx="1" cy="9" r="1" /><circle cx="5" cy="9" r="1" />
            </svg>
            <span class="name" title="{fx.name} — {fx.vendor}">{fx.name}</span>
            <button
              class="icon"
              class:on={!fx.bypassed}
              aria-pressed={fx.bypassed}
              aria-label="Bypass {fx.name}"
              title={fx.bypassed ? "Turn on" : "Bypass"}
              disabled={busy}
              onclick={() => void setEffectBypass(fx.slotId, !fx.bypassed)}
            >
              <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M6 1.5v4M3.2 3.3a4 4 0 1 0 5.6 0" /></svg>
            </button>
            <button
              class="icon"
              class:active={open}
              aria-label="{open ? 'Close' : 'Show'} the controls of {fx.name}"
              title={open ? "Close controls" : "Show controls"}
              disabled={busy}
              onclick={() => toggleControls(fx)}
            >
              <svg viewBox="0 0 12 12" aria-hidden="true"><rect x="1.5" y="2" width="9" height="8" rx="1" /><path d="M1.5 4.5h9" /></svg>
            </button>
            {#if fx.hasEditor}
              {@const generic = effectsUi.generic?.fileId === tab.id && effectsUi.generic.slotId === fx.slotId}
              <button
                class="icon"
                class:active={generic}
                aria-label="{generic ? 'Close' : 'Show'} the generic controls of {fx.name}"
                title={generic ? "Close generic controls" : "Show generic controls"}
                disabled={busy}
                onclick={() => (generic ? closeGeneric() : void showControls(tab.id, fx, true))}
              >
                <svg viewBox="0 0 12 12" aria-hidden="true">
                  <path d="M3 2v8M6 2v8M9 2v8" /><circle cx="3" cy="7" r="1.2" /><circle cx="6" cy="4" r="1.2" />
                  <circle cx="9" cy="8" r="1.2" />
                </svg>
              </button>
            {/if}
            <button
              class="icon"
              aria-label="Remove {fx.name}"
              title="Remove"
              disabled={busy}
              onclick={() => void removeEffect(fx.slotId)}
            >
              <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M3 3l6 6M9 3l-6 6" /></svg>
            </button>
          </li>
        {/each}
      </ol>
      {#if dragSlot !== null && gap !== null}
        <div class="drop-marker" style:top="{markerY}px" aria-hidden="true"></div>
      {/if}
    </div>
    {#if active}
      <p class="note">The waveform and statistics show the audio before effects.</p>
    {/if}
  {/if}

  <span class="add" title={full ? `The chain is full (${MAX_EFFECTS} effects)` : undefined}>
    <button disabled={busy || !canAddEffect()} onclick={() => (effectsUi.browserOpen = true)}>Add Effect…</button>
  </span>
</section>

<style>
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin: 0 0 8px;
  }
  h2 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-dim);
    margin: 0;
  }
  .small {
    padding: 1px 8px;
    font-size: 12px;
  }
  .dim,
  .note {
    color: var(--text-dim);
    margin: 0 0 8px;
  }
  .note {
    font-size: 12px;
    margin-top: 6px;
  }
  .chain-wrap {
    position: relative;
  }
  .chain {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .chain.dragging {
    cursor: grabbing;
    user-select: none;
  }
  .effect {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 4px 4px 8px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--button-bg);
    cursor: grab;
  }
  .effect.dragged {
    opacity: 0.5;
  }
  .effect.bypassed .name {
    color: var(--text-dim);
  }
  .grip {
    width: 6px;
    height: 10px;
    flex-shrink: 0;
    fill: var(--text-dim);
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .icon {
    display: flex;
    padding: 4px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-dim);
  }
  .icon:hover:not(:disabled) {
    color: var(--text);
  }
  .icon.on {
    color: var(--accent);
  }
  .icon.active {
    border-color: var(--accent);
    color: var(--accent);
  }
  .icon svg {
    width: 12px;
    height: 12px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
    stroke-linecap: round;
  }
  .icon svg circle {
    fill: var(--button-bg);
  }
  .drop-marker {
    position: absolute;
    left: 0;
    right: 0;
    height: 2px;
    margin-top: -1px;
    background: var(--accent);
    border-radius: 1px;
    pointer-events: none;
  }
  .add {
    display: inline-block;
    margin-top: 8px;
  }
</style>
