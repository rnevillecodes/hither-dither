<script lang="ts">
  import * as api from "../lib/api";
  import { closeGeneric, effectsUi, settingChanged } from "../lib/effects.svelte";
  import { notify } from "../lib/notices.svelte";
  import { activeTab } from "../lib/tabs.svelte";

  const target = $derived(effectsUi.generic);
  const tab = $derived(activeTab());
  /** Shown while its effect is in the active tab's chain. */
  const visible = $derived(
    !!target && tab?.id === target.fileId && !!tab.chain?.effects.some((e) => e.slotId === target.slotId),
  );

  let params = $state<api.FxParam[]>([]);
  let loading = $state(false);
  let pos = $state<{ x: number; y: number } | null>(null);
  let panel = $state<HTMLDivElement>();

  // The effect left its tab's chain (removed or undone): close.
  $effect(() => {
    const t = effectsUi.generic;
    if (!t) return;
    const owner = activeTab();
    if (owner?.id === t.fileId && owner.chain && !owner.chain.effects.some((e) => e.slotId === t.slotId)) {
      closeGeneric();
    }
  });

  async function load(fileId: number, slotId: number) {
    loading = true;
    try {
      const list = await api.getFxParams(fileId, slotId);
      if (effectsUi.generic?.fileId === fileId && effectsUi.generic.slotId === slotId) params = list;
    } catch (e) {
      notify("error", "Could not show the effect's controls", String(e));
      closeGeneric();
    } finally {
      loading = false;
    }
  }

  // Fetch the parameters for each effect shown, and follow the plugin's own changes.
  $effect(() => {
    const t = effectsUi.generic;
    params = [];
    if (!t) return;
    void load(t.fileId, t.slotId);
    const mine = (e: api.EffectRef) => e.fileId === t.fileId && e.slotId === t.slotId;
    const unlisten = [
      api.onFxParamChanged((e) => {
        if (!mine(e)) return;
        for (const c of e.changes) {
          const p = params.find((x) => x.id === c.id);
          if (p) Object.assign(p, { value: c.value, display: c.display });
        }
      }),
      api.onFxParamsReset((e) => mine(e) && void load(t.fileId, t.slotId)),
    ];
    return () => unlisten.forEach((u) => void u.then((f) => f()));
  });

  // Values on their way to the plugin: one call at a time, the latest value of each.
  const queued = new Map<number, number>();
  let sending = false;

  async function set(p: api.FxParam, value: number) {
    const t = effectsUi.generic;
    if (!t) return;
    p.value = value;
    queued.set(p.id, value);
    if (sending) return;
    sending = true;
    try {
      while (queued.size > 0) {
        const [id, v] = queued.entries().next().value!;
        queued.delete(id);
        const display = await api.setFxParam(t.fileId, t.slotId, id, v);
        settingChanged(t.fileId);
        const shown = params.find((x) => x.id === id);
        if (shown && !queued.has(id)) shown.display = display;
      }
    } catch (e) {
      queued.clear();
      notify("error", "Could not change the effect", String(e));
    } finally {
      sending = false;
    }
  }

  const step = (p: api.FxParam) => (p.stepCount > 0 ? 1 / p.stepCount : "any");

  // Moving the panel by its title bar.
  let drag: { dx: number; dy: number; pointerId: number } | null = null;

  function onTitleDown(e: PointerEvent) {
    if (e.button !== 0 || !panel || (e.target as HTMLElement).closest("button")) return;
    const r = panel.getBoundingClientRect();
    drag = { dx: e.clientX - r.left, dy: e.clientY - r.top, pointerId: e.pointerId };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function onTitleMove(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointerId || !panel) return;
    const r = panel.getBoundingClientRect();
    pos = {
      x: Math.max(0, Math.min(window.innerWidth - r.width, e.clientX - drag.dx)),
      y: Math.max(0, Math.min(window.innerHeight - 40, e.clientY - drag.dy)),
    };
  }

  function onKeydown(e: KeyboardEvent) {
    // Arrow keys on a slider shouldn't also act on the waveform.
    e.stopPropagation();
    if (e.key === "Escape") closeGeneric();
  }
</script>

{#if visible && target}
  <div
    class="panel"
    role="dialog"
    aria-labelledby="generic-title"
    bind:this={panel}
    style:left={pos ? `${pos.x}px` : undefined}
    style:top={pos ? `${pos.y}px` : undefined}
    style:bottom={pos ? "auto" : undefined}
    onkeydown={onKeydown}
    tabindex="-1"
  >
    <div
      class="title"
      role="presentation"
      onpointerdown={onTitleDown}
      onpointermove={onTitleMove}
      onpointerup={() => (drag = null)}
    >
      <h2 id="generic-title">{target.name}</h2>
      <button class="close" aria-label="Close controls" title="Close" onclick={closeGeneric}>
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M2 2l6 6M8 2l-6 6" /></svg>
      </button>
    </div>
    {#if target.note}
      <p class="note">{target.note}</p>
    {/if}
    <div class="params">
      {#each params as p (p.id)}
        <label class="param" title="Double-click the slider to reset it">
          <span class="name">{p.title}</span>
          <span class="value mono">{p.display}{p.units ? ` ${p.units}` : ""}</span>
          <input
            type="range"
            min="0"
            max="1"
            step={step(p)}
            value={p.value}
            aria-valuetext="{p.display} {p.units}"
            oninput={(e) => void set(p, Number(e.currentTarget.value))}
            ondblclick={() => void set(p, p.defaultValue)}
          />
        </label>
      {:else}
        <p class="dim">{loading ? "Loading…" : "This effect has no parameters to adjust."}</p>
      {/each}
    </div>
  </div>
{/if}

<style>
  .panel {
    position: fixed;
    left: 24px;
    bottom: 24px;
    width: 300px;
    max-height: min(520px, calc(100vh - 48px));
    display: flex;
    flex-direction: column;
    background: var(--panel-bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.45);
    z-index: 25;
  }
  .panel:focus {
    outline: none;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 8px 8px 12px;
    border-bottom: 1px solid var(--border);
    cursor: move;
    user-select: none;
    touch-action: none;
  }
  h2 {
    flex: 1;
    min-width: 0;
    margin: 0;
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .close {
    display: flex;
    padding: 4px;
    border: none;
    background: none;
    color: var(--text-dim);
  }
  .close svg {
    width: 10px;
    height: 10px;
    stroke: currentColor;
    stroke-width: 1.5;
    stroke-linecap: round;
  }
  .note {
    margin: 8px 12px 0;
    color: var(--warning);
    font-size: 12px;
  }
  .params {
    overflow-y: auto;
    padding: 8px 12px 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .param {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 2px 8px;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .value {
    color: var(--text-dim);
  }
  input[type="range"] {
    grid-column: 1 / -1;
    width: 100%;
    margin: 0;
  }
  .dim {
    color: var(--text-dim);
    margin: 0;
  }
</style>
