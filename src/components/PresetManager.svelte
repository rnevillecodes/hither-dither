<script lang="ts">
  import { deletePreset } from "../lib/editActions";
  import { presets, refreshPresets } from "../lib/presets.svelte";

  let dialog = $state<HTMLDivElement>();
  let busy = $state(false);

  $effect(() => {
    if (!presets.managerOpen) return;
    void refreshPresets();
    queueMicrotask(() => dialog?.focus());
  });

  function close() {
    presets.managerOpen = false;
  }

  function rename(id: number, name: string) {
    presets.prompt = { kind: "rename", presetId: id, initial: name };
  }

  async function remove(preset: (typeof presets.list)[number]) {
    busy = true;
    try {
      await deletePreset(preset);
    } finally {
      busy = false;
      dialog?.focus();
    }
  }

  function onKeydown(e: KeyboardEvent) {
    // Keep app shortcuts (Space, Delete…) from acting behind the dialog.
    e.stopPropagation();
    // The name prompt above handles its own Escape.
    if (e.key === "Escape" && !presets.prompt) {
      e.preventDefault();
      close();
    }
  }
</script>

{#if presets.managerOpen}
  <div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && close()}>
    <div
      class="dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="presets-title"
      tabindex="-1"
      bind:this={dialog}
      onkeydown={onKeydown}
    >
      <h2 id="presets-title">Manage Presets</h2>
      <ul class="list">
        {#each presets.list as p (p.id)}
          <li>
            <span class="name" title={p.name}>{p.name}</span>
            <span class="dim">{p.effectCount} {p.effectCount === 1 ? "effect" : "effects"}</span>
            <button disabled={busy} aria-label="Rename {p.name}" onclick={() => rename(p.id, p.name)}>Rename…</button>
            <button disabled={busy} aria-label="Delete {p.name}" onclick={() => void remove(p)}>Delete</button>
          </li>
        {:else}
          <li class="empty dim">
            No presets yet. Save a group as a preset with the save button on its header, or Effects › Save Group as
            Preset….
          </li>
        {/each}
      </ul>
      <div class="buttons">
        <button class="primary" onclick={close}>Done</button>
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
    width: min(480px, calc(100vw - 32px));
    max-height: min(520px, calc(100vh - 48px));
    background: var(--panel-bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 16px 18px;
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.45);
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .dialog:focus {
    outline: none;
  }
  h2 {
    margin: 0;
    font-size: 15px;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
  }
  .list li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--wf-grid);
  }
  .list li:last-child {
    border-bottom: none;
  }
  .name {
    flex: 1;
    min-width: 0;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dim {
    color: var(--text-dim);
    font-size: 12px;
    flex-shrink: 0;
  }
  .empty {
    padding: 12px;
    flex-shrink: 1;
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
  }
</style>
