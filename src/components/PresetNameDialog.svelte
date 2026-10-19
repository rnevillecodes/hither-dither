<script lang="ts">
  import { MAX_PRESET_NAME, validPresetName } from "../lib/api";
  import {
    type PresetSaveResult,
    confirmReplacePreset,
    renamePreset,
    saveGroupPreset,
  } from "../lib/editActions";
  import { type NamePrompt, presetNamed, presets } from "../lib/presets.svelte";

  let input = $state<HTMLInputElement>();
  let name = $state("");
  let busy = $state(false);

  const prompt = $derived(presets.prompt);
  const valid = $derived(validPresetName(name));

  $effect(() => {
    if (!prompt) return;
    name = prompt.initial;
    busy = false;
    queueMicrotask(() => input?.select());
  });

  function close() {
    presets.prompt = null;
  }

  function call(p: NamePrompt, name: string, replace: boolean): Promise<PresetSaveResult> {
    return p.kind === "save"
      ? saveGroupPreset(p.fileId, p.groupId, name, replace)
      : renamePreset(p.presetId, name, replace);
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    const p = prompt;
    if (!p || !valid || busy) return;
    busy = true;
    try {
      let result = await call(p, valid, false);
      if (result === "taken") {
        const existing = presetNamed(valid, p.kind === "rename" ? p.presetId : undefined);
        // Declining keeps the prompt open, so another name can be chosen.
        if (await confirmReplacePreset(existing?.name ?? valid)) result = await call(p, valid, true);
      }
      if (result === "saved" && presets.prompt === p) close();
    } finally {
      busy = false;
      if (presets.prompt === p) queueMicrotask(() => input?.focus());
    }
  }

  function onKeydown(e: KeyboardEvent) {
    // Keep app shortcuts (Space, Delete…) from acting behind the dialog.
    e.stopPropagation();
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    }
  }
</script>

{#if prompt}
  <div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && !busy && close()}>
    <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="preset-name-title" tabindex="-1" onkeydown={onKeydown}>
      <form onsubmit={(e) => void submit(e)}>
        <h2 id="preset-name-title">{prompt.kind === "save" ? "Save as Preset" : "Rename Preset"}</h2>
        <label>
          <span>Preset name</span>
          <input
            type="text"
            bind:this={input}
            bind:value={name}
            maxlength={MAX_PRESET_NAME * 2}
            aria-invalid={!valid}
            aria-describedby="preset-name-hint"
            disabled={busy}
          />
        </label>
        <p id="preset-name-hint" class="hint" class:error={!valid}>
          {valid ? "Presets are kept by HitherDither and can be inserted into any tab." : `A name must be 1 to ${MAX_PRESET_NAME} characters.`}
        </p>
        <div class="buttons">
          <button type="button" onclick={close} disabled={busy}>Cancel</button>
          <button type="submit" class="primary" disabled={!valid || busy}>{prompt.kind === "save" ? "Save" : "Rename"}</button>
        </div>
      </form>
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
    /* Above Manage Presets, which opens it for renaming. */
    z-index: 50;
  }
  .dialog {
    width: min(380px, calc(100vw - 32px));
    background: var(--panel-bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 16px 18px;
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.45);
  }
  form {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  h2 {
    margin: 0;
    font-size: 15px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  label span {
    font-size: 12px;
    color: var(--text-dim);
  }
  input {
    font: inherit;
    color: var(--text);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 5px 8px;
  }
  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--text-dim);
  }
  .hint.error {
    color: var(--warning);
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
