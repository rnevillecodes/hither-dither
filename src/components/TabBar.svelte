<script lang="ts">
  import { activateTab, closeTab, moveTab } from "../lib/actions";
  import { tabs } from "../lib/tabs.svelte";

  /** Horizontal movement (px) before a press on a tab becomes a drag rather than a click. */
  const DRAG_THRESHOLD = 4;
  /** Releasing further than this (px) above or below the strip cancels the drag. */
  const DROP_SLACK = 60;
  /** Within this distance (px) of either end, a drag scrolls the strip. */
  const EDGE = 36;
  const EDGE_SPEED = 10;

  let strip: HTMLDivElement;

  // Dragging a tab to reorder. Pointer events rather than HTML5 drag-and-drop, which
  // the webview's OS file-drop handling (`dragDropEnabled`) intercepts on Windows.
  let dragId = $state<number | null>(null);
  /** Gap the dragged tab would be dropped into (0..length), or null if the drop would cancel. */
  let dropGap = $state<number | null>(null);
  let indicatorX = $state(0);
  let press: { id: number; pointerId: number; startX: number; x: number; y: number } | null = null;
  let scrollFrame = 0;
  let swallowClick = false;

  function onPointerDown(e: PointerEvent, id: number) {
    swallowClick = false;
    if (e.button !== 0 || (e.target as HTMLElement).closest(".close")) return;
    press = { id, pointerId: e.pointerId, startX: e.clientX, x: e.clientX, y: e.clientY };
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    window.addEventListener("pointercancel", endDrag);
    window.addEventListener("keydown", onDragKey, true);
  }

  function onPointerMove(e: PointerEvent) {
    if (!press || e.pointerId !== press.pointerId) return;
    press.x = e.clientX;
    press.y = e.clientY;
    if (dragId === null) {
      if (Math.abs(e.clientX - press.startX) < DRAG_THRESHOLD) return;
      dragId = press.id;
      // Capture only once dragging: capturing on press would retarget the click
      // that activates the tab.
      strip.setPointerCapture(e.pointerId);
      scrollFrame = requestAnimationFrame(autoScroll);
    }
    e.preventDefault();
    updateDropGap();
  }

  function onPointerUp(e: PointerEvent) {
    if (!press || e.pointerId !== press.pointerId) return;
    if (dragId !== null) {
      // The click that ends a drag (dispatched right after this) must not activate the tab.
      swallowClick = true;
      setTimeout(() => (swallowClick = false));
      if (dropGap !== null) moveTab(dragId, dropGap);
    }
    endDrag();
  }

  function onDragKey(e: KeyboardEvent) {
    if (e.key === "Escape" && dragId !== null) {
      // Don't also clear the waveform selection.
      e.stopPropagation();
      e.preventDefault();
      endDrag();
    }
  }

  function endDrag() {
    if (press && strip?.hasPointerCapture(press.pointerId)) strip.releasePointerCapture(press.pointerId);
    press = null;
    dragId = null;
    dropGap = null;
    cancelAnimationFrame(scrollFrame);
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
    window.removeEventListener("pointercancel", endDrag);
    window.removeEventListener("keydown", onDragKey, true);
  }

  /** The gap under the pointer: the number of tabs whose middle is left of it. */
  function updateDropGap() {
    if (!press) return;
    const box = strip.getBoundingClientRect();
    if (press.y < box.top - DROP_SLACK || press.y > box.bottom + DROP_SLACK) {
      dropGap = null;
      return;
    }
    const rects = [...strip.querySelectorAll<HTMLElement>("[data-tab-id]")].map((el) => el.getBoundingClientRect());
    if (rects.length === 0) return;
    const gap = rects.filter((r) => r.left + r.width / 2 < press!.x).length;
    const edge = gap < rects.length ? rects[gap].left - 1 : rects[rects.length - 1].right + 1;
    indicatorX = edge - box.left + strip.scrollLeft;
    dropGap = gap;
  }

  function autoScroll() {
    if (press && dragId !== null) {
      const box = strip.getBoundingClientRect();
      const step = press.x < box.left + EDGE ? -EDGE_SPEED : press.x > box.right - EDGE ? EDGE_SPEED : 0;
      if (step !== 0 && dropGap !== null) {
        strip.scrollLeft += step;
        updateDropGap();
      }
      scrollFrame = requestAnimationFrame(autoScroll);
    }
  }

  // The strip disappears when the last tab closes, possibly mid-drag.
  $effect(() => endDrag);

  function onClickCapture(e: MouseEvent) {
    if (swallowClick) {
      swallowClick = false;
      e.stopPropagation();
      e.preventDefault();
    }
  }

  // Keep the active tab in view when it changes (e.g. a newly opened file at the end).
  $effect(() => {
    const id = tabs.activeId;
    strip?.querySelector(`[data-tab-id="${id}"]`)?.scrollIntoView({ block: "nearest", inline: "nearest" });
  });

  function onAuxClick(e: MouseEvent, id: number) {
    if (e.button === 1) {
      e.preventDefault();
      void closeTab(id);
    }
  }
</script>

<div
  class="tabs"
  class:dragging={dragId !== null}
  role="tablist"
  aria-label="Open files"
  bind:this={strip}
  onclickcapture={onClickCapture}
>
  {#each tabs.list as tab (tab.id)}
    {@const active = tab.id === tabs.activeId}
    <div
      class="tab"
      class:active
      class:dirty={tab.dirty}
      class:dragged={tab.id === dragId}
      role="presentation"
      data-tab-id={tab.id}
      onpointerdown={(e) => onPointerDown(e, tab.id)}
      onauxclick={(e) => onAuxClick(e, tab.id)}
      onmousedown={(e) => e.button === 1 && e.preventDefault()}
    >
      <button
        class="name"
        role="tab"
        aria-selected={active}
        title={tab.dirty ? `${tab.path}
Unsaved changes` : tab.path}
        onclick={() => activateTab(tab.id)}
      >
        {#if tab.status === "loading"}
          <span class="spinner" aria-label="Loading"></span>
        {/if}
        <span class="label">{tab.name}</span>
        {#if tab.dirty}<span class="sr-only">(unsaved changes)</span>{/if}
      </button>
      <!-- Unsaved tabs show a dot in place of the ×, which comes back on hover or focus. -->
      <button class="close" aria-label="Close {tab.name}" title="Close (Ctrl/Cmd+W)" onclick={() => void closeTab(tab.id)}>
        <svg class="cross" viewBox="0 0 10 10" aria-hidden="true"><path d="M2 2l6 6M8 2l-6 6" /></svg>
        <svg class="dot" viewBox="0 0 10 10" aria-hidden="true"><circle cx="5" cy="5" r="3" /></svg>
      </button>
    </div>
  {/each}
  {#if dragId !== null && dropGap !== null}
    <div class="drop-marker" style:left="{indicatorX}px" aria-hidden="true"></div>
  {/if}
</div>

<style>
  .tabs {
    display: flex;
    align-items: flex-end;
    gap: 2px;
    padding: 6px 8px 0;
    background: var(--bg);
    border-bottom: 1px solid var(--border);
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: thin;
    flex-shrink: 0;
    position: relative;
  }
  .tabs.dragging {
    cursor: grabbing;
    user-select: none;
  }
  .tabs.dragging .tab:hover:not(.active) {
    background: transparent;
  }
  .tab.dragged {
    opacity: 0.5;
  }
  .drop-marker {
    position: absolute;
    top: 4px;
    bottom: 0;
    width: 2px;
    margin-left: -1px;
    background: var(--accent);
    border-radius: 1px;
    pointer-events: none;
  }
  .tab {
    display: flex;
    align-items: center;
    flex-shrink: 0;
    max-width: 240px;
    border: 1px solid transparent;
    border-bottom: none;
    border-radius: 6px 6px 0 0;
    margin-bottom: -1px;
    color: var(--text-dim);
  }
  .tab:hover {
    background: var(--button-hover);
  }
  .tab.active {
    background: var(--panel-bg);
    border-color: var(--border);
    color: var(--text);
    box-shadow: inset 0 2px 0 var(--accent);
  }
  button {
    border: none;
    background: transparent;
    color: inherit;
  }
  button:hover:not(:disabled) {
    background: transparent;
  }
  .name {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    padding: 6px 4px 6px 12px;
    border-radius: 6px 0 0 0;
  }
  /* The strip clips overflow, so draw the focus ring inside the tab. */
  .name:focus-visible,
  .close:focus-visible {
    outline-offset: -2px;
  }
  .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .close {
    display: flex;
    padding: 4px;
    margin-right: 6px;
    border-radius: 4px;
    opacity: 0.7;
  }
  .close:hover:not(:disabled) {
    opacity: 1;
    background: var(--button-hover);
  }
  .tab.active .close:hover:not(:disabled) {
    background: var(--bg);
  }
  .close .dot {
    display: none;
    fill: currentColor;
    stroke: none;
  }
  .tab.dirty .close:not(:hover):not(:focus-visible) .dot {
    display: block;
  }
  .tab.dirty .close:not(:hover):not(:focus-visible) .cross {
    display: none;
  }
  .tab.dirty .close {
    opacity: 1;
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
  .close svg {
    width: 10px;
    height: 10px;
    stroke: currentColor;
    stroke-width: 1.5;
    stroke-linecap: round;
  }
  .spinner {
    width: 10px;
    height: 10px;
    flex-shrink: 0;
    border: 2px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
