<script lang="ts">
  import { tick } from "svelte";
  import { type EffectInfo, type GroupInfo, MAX_GROUP_NAME } from "../lib/api";
  import {
    type ChainRow,
    type ClickMode,
    type Drop,
    chainRows,
    clickSelection,
    dropGap,
    dropTarget,
    stepTarget,
  } from "../lib/chainOrder";
  import {
    canAddEffect,
    canGroup,
    groupSelected,
    moveEffect,
    moveGroup,
    removeEffect,
    removeGroup,
    renameGroup,
    setBypassAll,
    setEffectBypass,
    setGroupBypass,
    ungroup,
  } from "../lib/editActions";
  import { closeControls, closeGeneric, editorOpen, effectsUi, showControls } from "../lib/effects.svelte";
  import type { Tab } from "../lib/tabs.svelte";

  /** `titled` false: the sidebar already shows "Effects" above, so the heading is only read out. */
  let { tab, titled = true }: { tab: Tab; titled?: boolean } = $props();

  /** Vertical movement (px) before a press on a row becomes a drag. */
  const DRAG_THRESHOLD = 4;
  /** How far a group's effects are indented (px); dropping right of this joins the group. */
  const INDENT = 16;

  const effects = $derived(tab.chain?.effects ?? []);
  const bypassAll = $derived(tab.chain?.bypassAll ?? false);
  const rows = $derived(tab.chain ? chainRows(tab.chain, tab.collapsedGroups) : []);
  const busy = $derived(tab.editing);

  let list = $state<HTMLOListElement>();

  /** Identifies a row in the DOM, to find it again after the chain changes. */
  const rowKey = (row: ChainRow) => (row.kind === "header" ? `g${row.group.groupId}` : `s${row.effect.slotId}`);

  async function focusRow(key: string) {
    await tick();
    list?.querySelector<HTMLElement>(`[data-row="${key}"]`)?.focus();
  }

  // --- Selection -------------------------------------------------------------------------

  function select(row: ChainRow, mode: ClickMode) {
    if (row.kind === "header") {
      tab.fxSelected = [...row.group.slotIds];
      tab.fxAnchor = row.group.slotIds[0];
      return;
    }
    const order = effects.map((fx) => fx.slotId);
    const next = clickSelection(order, tab.fxSelected, tab.fxAnchor, row.effect.slotId, mode);
    tab.fxSelected = next.selected;
    tab.fxAnchor = next.anchor;
  }

  const clickMode = (e: MouseEvent | KeyboardEvent): ClickMode =>
    e.shiftKey ? "range" : e.ctrlKey || e.metaKey ? "toggle" : "only";

  // --- Dragging, with pointer events as in the tab strip ---------------------------------

  let dragged = $state<ChainRow | null>(null);
  let drop = $state<Drop | null>(null);
  let marker = $state({ y: 0, indent: false });
  let press: { row: ChainRow; pointerId: number; startY: number; mode: ClickMode } | null = null;

  function onPointerDown(e: PointerEvent, row: ChainRow) {
    const target = e.target as HTMLElement;
    if (e.button !== 0 || busy || target.closest("button, input")) return;
    press = { row, pointerId: e.pointerId, startY: e.clientY, mode: clickMode(e) };
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    window.addEventListener("pointercancel", endDrag);
    window.addEventListener("keydown", onDragKey, true);
  }

  function onPointerMove(e: PointerEvent) {
    if (!press || e.pointerId !== press.pointerId || !list) return;
    if (dragged === null) {
      if (Math.abs(e.clientY - press.startY) < DRAG_THRESHOLD) return;
      dragged = press.row;
      list.setPointerCapture(e.pointerId);
    }
    e.preventDefault();
    const boxes = [...list.querySelectorAll<HTMLElement>("[data-row]")].map((el) => el.getBoundingClientRect());
    if (boxes.length === 0) return;
    const box = list.getBoundingClientRect();
    const gap = dropGap(
      boxes.map((r) => r.top + r.height / 2),
      e.clientY,
    );
    drop = dropTarget(rows, dragged, gap, e.clientX - box.left >= INDENT);
    const y = gap < boxes.length ? boxes[gap].top - 3 : boxes[boxes.length - 1].bottom + 3;
    marker = { y: y - box.top, indent: drop?.kind === "effect" && drop.to.group !== null };
  }

  function onPointerUp(e: PointerEvent) {
    if (!press || e.pointerId !== press.pointerId) return;
    if (dragged === null) select(press.row, press.mode);
    else if (drop) void apply(dragged, drop);
    endDrag();
  }

  function onDragKey(e: KeyboardEvent) {
    if (e.key === "Escape" && dragged !== null) {
      e.stopPropagation();
      e.preventDefault();
      endDrag();
    }
  }

  function endDrag() {
    if (press && list?.hasPointerCapture(press.pointerId)) list.releasePointerCapture(press.pointerId);
    press = null;
    dragged = null;
    drop = null;
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
    window.removeEventListener("pointercancel", endDrag);
    window.removeEventListener("keydown", onDragKey, true);
  }

  $effect(() => endDrag);

  function apply(row: ChainRow, to: Drop): Promise<void> {
    if (to.kind === "group" && row.kind === "header") return moveGroup(row.group.groupId, to.index);
    if (to.kind === "effect" && row.kind === "effect") return moveEffect(row.effect.slotId, to.to);
    return Promise.resolve();
  }

  /** Whether `row` is being dragged, or is an effect of the group being dragged. */
  function isDragged(row: ChainRow): boolean {
    if (!dragged) return false;
    if (dragged.kind === "header") return row.group?.groupId === dragged.group.groupId;
    return row.kind === "effect" && row.effect.slotId === dragged.effect.slotId;
  }

  // --- Keyboard ----------------------------------------------------------------------------

  /** Alt+Up / Alt+Down move the focused row, which keeps the focus; Enter or Space selects; F2 renames. */
  async function onRowKey(e: KeyboardEvent, row: ChainRow) {
    if (e.target !== e.currentTarget || busy) return;
    if (e.altKey && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
      e.preventDefault();
      const to = stepTarget(rows, row, e.key === "ArrowUp" ? -1 : 1);
      if (!to) return;
      await apply(row, to);
      await focusRow(rowKey(row));
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      select(row, clickMode(e));
    } else if (e.key === "F2" && row.kind === "header") {
      e.preventDefault();
      startRename(row.group);
    }
  }

  // --- Groups --------------------------------------------------------------------------------

  function toggleCollapsed(group: GroupInfo) {
    const id = group.groupId;
    tab.collapsedGroups = tab.collapsedGroups.includes(id)
      ? tab.collapsedGroups.filter((g) => g !== id)
      : [...tab.collapsedGroups, id];
  }

  let renaming = $state<{ groupId: number; name: string } | null>(null);

  async function startRename(group: GroupInfo) {
    renaming = { groupId: group.groupId, name: group.name };
    await tick();
    const input = list?.querySelector<HTMLInputElement>(".rename");
    input?.focus();
    input?.select();
  }

  /** Commits the name being typed; an empty or unchanged one keeps the old name. */
  async function finishRename(commit: boolean) {
    if (!renaming) return;
    const { groupId, name } = renaming;
    renaming = null;
    const old = tab.chain?.groups.find((g) => g.groupId === groupId)?.name;
    const trimmed = name.trim();
    if (commit && trimmed && trimmed !== old) await renameGroup(groupId, trimmed);
    await focusRow(`g${groupId}`);
  }

  // Rename Group (Ctrl/Cmd+R, or the Effects menu) asks for a group's name to be edited here.
  $effect(() => {
    const id = effectsUi.renameGroup;
    if (id === null) return;
    effectsUi.renameGroup = null;
    const group = tab.chain?.groups.find((g) => g.groupId === id);
    if (group) void startRename(group);
  });

  function onRenameKey(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === "Enter") {
      e.preventDefault();
      void finishRename(true);
    } else if (e.key === "Escape") {
      e.preventDefault();
      void finishRename(false);
    }
  }

  function toggleControls(fx: EffectInfo) {
    if (editorOpen(tab.id, fx.slotId)) closeControls(tab.id, fx.slotId);
    else void showControls(tab.id, fx);
  }
</script>

<section aria-labelledby="effects-heading">
  <header>
    <h2 id="effects-heading" class:sr-only={!titled}>Effects</h2>
    <span class="add" title="Add Effect…">
      <button
        class="small"
        aria-label="Add Effect"
        disabled={busy || !canAddEffect()}
        onclick={() => (effectsUi.browserOpen = true)}
      >
        <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M6 2v8M2 6h8" /></svg>
        Add
      </button>
    </span>
    {#if effects.length > 0}
      <span title="Group the selected effects (Ctrl/Cmd+G)">
        <button class="small" disabled={busy || !canGroup()} onclick={() => void groupSelected()}>Group</button>
      </span>
      <button
        class="small bypass-all"
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
      <ol class="chain" class:dragging={dragged !== null} bind:this={list} aria-label="Effects in processing order">
        {#each rows as row (rowKey(row))}
          {#if row.kind === "header"}
            {@const g = row.group}
            {@const count = `${g.slotIds.length} ${g.slotIds.length === 1 ? "effect" : "effects"}`}
            {@const selected = g.slotIds.every((s) => tab.fxSelected.includes(s))}
            <!-- A row takes the focus so Alt+Up/Down can move it; its buttons do the rest. -->
            <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
            <li
              class="row header"
              class:bypassed={g.bypassed || bypassAll}
              class:selected
              class:dragged={isDragged(row)}
              data-row={rowKey(row)}
              tabindex="0"
              aria-label="Group {g.name}, {count}{g.bypassed ? ', bypassed' : ''}. Alt+Up or Alt+Down to move, F2 to rename."
              onpointerdown={(e) => onPointerDown(e, row)}
              onkeydown={(e) => void onRowKey(e, row)}
            >
              <button
                class="icon fold"
                aria-expanded={!row.collapsed}
                aria-label="{row.collapsed ? 'Expand' : 'Collapse'} {g.name}"
                title={row.collapsed ? "Expand" : "Collapse"}
                onclick={() => toggleCollapsed(g)}
              >
                <svg viewBox="0 0 12 12" aria-hidden="true" class:collapsed={row.collapsed}><path d="M3 4.5l3 3 3-3" /></svg>
              </button>
              {#if renaming?.groupId === g.groupId}
                <input
                  class="rename"
                  aria-label="Group name"
                  maxlength={MAX_GROUP_NAME}
                  bind:value={renaming.name}
                  onkeydown={onRenameKey}
                  onblur={() => void finishRename(true)}
                />
              {:else}
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <span class="name" title="{g.name} — double-click to rename" ondblclick={() => void startRename(g)}
                  >{g.name}</span
                >
                <span class="count">{count}</span>
              {/if}
              <button
                class="icon"
                class:on={!g.bypassed}
                aria-pressed={g.bypassed}
                aria-label="Bypass group {g.name}"
                title={g.bypassed ? "Turn the group on" : "Bypass the group"}
                disabled={busy}
                onclick={() => void setGroupBypass(g.groupId, !g.bypassed)}
              >
                <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M6 1.5v4M3.2 3.3a4 4 0 1 0 5.6 0" /></svg>
              </button>
              <button
                class="icon"
                aria-label="Ungroup {g.name}"
                title="Ungroup (Ctrl/Cmd+Shift+G)"
                disabled={busy}
                onclick={() => void ungroup(g.groupId)}
              >
                <svg viewBox="0 0 12 12" aria-hidden="true">
                  <rect x="1.5" y="1.5" width="4" height="4" rx="0.5" /><rect x="6.5" y="6.5" width="4" height="4" rx="0.5" />
                </svg>
              </button>
              <button
                class="icon"
                aria-label="Remove group {g.name} and its effects"
                title="Remove the group and its effects"
                disabled={busy}
                onclick={() => void removeGroup(g.groupId)}
              >
                <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M3 3l6 6M9 3l-6 6" /></svg>
              </button>
            </li>
          {:else}
            {@const fx = row.effect}
            {@const open = editorOpen(tab.id, fx.slotId)}
            {@const held = !!row.group?.bypassed}
            <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
            <li
              class="row effect"
              class:member={row.group !== null}
              class:bypassed={fx.bypassed || held || bypassAll}
              class:selected={tab.fxSelected.includes(fx.slotId)}
              class:dragged={isDragged(row)}
              data-row={rowKey(row)}
              tabindex="0"
              aria-label="{fx.name}{fx.bypassed ? ', bypassed' : ''}{held ? ', group bypassed' : ''}. Alt+Up or Alt+Down to move."
              onpointerdown={(e) => onPointerDown(e, row)}
              onkeydown={(e) => void onRowKey(e, row)}
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
          {/if}
        {/each}
      </ol>
      {#if dragged !== null && drop !== null}
        <div
          class="drop-marker"
          style:top="{marker.y}px"
          style:left="{marker.indent ? INDENT : 0}px"
          aria-hidden="true"
        ></div>
      {/if}
    </div>
  {/if}
</section>

<style>
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 8px;
  }
  h2 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-dim);
    margin: 0;
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
  .small {
    padding: 1px 8px;
    font-size: 12px;
  }
  .bypass-all {
    margin-left: auto;
  }
  .dim {
    color: var(--text-dim);
    margin: 0 0 8px;
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
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 4px 4px 8px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--button-bg);
    cursor: grab;
  }
  .header {
    padding-left: 2px;
    background: transparent;
  }
  .member {
    margin-left: 16px;
  }
  .row.selected {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 14%, var(--button-bg));
  }
  .row.dragged {
    opacity: 0.5;
  }
  .row.bypassed .name {
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
  .header .name {
    flex: 0 1 auto;
    font-weight: 600;
  }
  .count {
    flex: 1;
    color: var(--text-dim);
    font-size: 11px;
    white-space: nowrap;
  }
  .rename {
    flex: 1;
    min-width: 0;
    font: inherit;
    padding: 0 4px;
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
  .fold svg {
    transition: transform 0.1s;
  }
  .fold svg.collapsed {
    transform: rotate(-90deg);
  }
  .drop-marker {
    position: absolute;
    right: 0;
    height: 2px;
    margin-top: -1px;
    background: var(--accent);
    border-radius: 1px;
    pointer-events: none;
  }
  .add button {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .add svg {
    width: 10px;
    height: 10px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
  }
</style>
