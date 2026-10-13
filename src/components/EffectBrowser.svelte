<script lang="ts">
  import { addEffect } from "../lib/editActions";
  import { browse, scanText, type BrowserEntry } from "../lib/effectBrowser";
  import { effectsUi, showControls } from "../lib/effects.svelte";
  import { plugins, refreshPlugins } from "../lib/plugins.svelte";
  import { activeTab } from "../lib/tabs.svelte";

  let query = $state("");
  let dialog = $state<HTMLDivElement>();
  let search = $state<HTMLInputElement>();
  /** classId of the highlighted effect, chosen with Enter. */
  let current = $state<string | null>(null);

  const tab = $derived(activeTab());
  const groups = $derived(tab?.meta ? browse(plugins.list, query, tab.meta) : []);
  const usable = $derived(groups.flatMap((g) => g.entries).filter((e) => e.usable));
  const scanning = $derived(scanText(plugins.progress));

  $effect(() => {
    if (!effectsUi.browserOpen) return;
    query = "";
    current = null;
    void refreshPlugins();
    queueMicrotask(() => search?.focus());
  });

  // Keep the highlight on an effect that is listed.
  $effect(() => {
    if (!usable.some((e) => e.classId === current)) current = usable[0]?.classId ?? null;
  });

  function close() {
    effectsUi.browserOpen = false;
  }

  async function choose(entry: BrowserEntry) {
    const target = activeTab();
    if (!entry.usable || !target) return;
    close();
    const fx = await addEffect(entry.classId);
    if (fx) await showControls(target.id, fx);
  }

  function step(by: 1 | -1) {
    if (usable.length === 0) return;
    const i = usable.findIndex((e) => e.classId === current);
    current = usable[(i + by + usable.length) % usable.length].classId;
    dialog?.querySelector(`[data-class-id="${current}"]`)?.scrollIntoView({ block: "nearest" });
  }

  function onKeydown(e: KeyboardEvent) {
    // Keep app shortcuts (Space, Delete…) from acting behind the dialog.
    e.stopPropagation();
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      step(e.key === "ArrowDown" ? 1 : -1);
    } else if (e.key === "Enter" && (e.target as HTMLElement).tagName !== "BUTTON") {
      e.preventDefault();
      const entry = usable.find((x) => x.classId === current);
      if (entry) void choose(entry);
    }
  }

  function manage() {
    close();
    effectsUi.managerOpen = true;
  }
</script>

{#if effectsUi.browserOpen && tab?.meta}
  <div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && close()}>
    <div
      class="dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="browser-title"
      tabindex="-1"
      bind:this={dialog}
      onkeydown={onKeydown}
    >
      <h2 id="browser-title">Add Effect</h2>
      <input
        type="search"
        placeholder="Search by name or vendor"
        aria-label="Search effects"
        aria-controls="effect-list"
        bind:value={query}
        bind:this={search}
      />
      {#if scanning}
        <p class="scanning" role="status"><span class="spinner" aria-hidden="true"></span>{scanning}</p>
      {/if}
      <div class="list" id="effect-list" role="listbox" aria-label="Effects">
        {#each groups as group (group.vendor)}
          <div role="group" aria-label={group.vendor}>
            <h3>{group.vendor}</h3>
            {#each group.entries as entry (entry.classId)}
              <!-- The dialog handles the keys: arrows move the highlight, Enter adds it. -->
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <div
                class="entry"
                class:current={entry.classId === current}
                class:unusable={!entry.usable}
                role="option"
                aria-selected={entry.classId === current}
                aria-disabled={!entry.usable}
                tabindex="-1"
                data-class-id={entry.classId}
                title={entry.reason ?? undefined}
                onclick={() => void choose(entry)}
                onpointermove={() => entry.usable && (current = entry.classId)}
              >
                <span class="name">{entry.name}</span>
                {#if entry.reason}<span class="reason">{entry.reason}</span>{/if}
              </div>
            {/each}
          </div>
        {:else}
          <p class="empty">
            {#if query.trim()}
              No effects match “{query.trim()}”.
            {:else if scanning}
              Effects appear here as they are found.
            {:else}
              No effects were found in the plugin folders.
            {/if}
          </p>
        {/each}
      </div>
      <div class="buttons">
        <button onclick={manage}>Manage Plugins…</button>
        <span class="spacer"></span>
        <button onclick={close}>Cancel</button>
        <button
          class="primary"
          disabled={!current}
          onclick={() => {
            const entry = usable.find((x) => x.classId === current);
            if (entry) void choose(entry);
          }}>Add</button
        >
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
    width: min(460px, calc(100vw - 32px));
    max-height: min(560px, calc(100vh - 48px));
    background: var(--panel-bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 16px 18px;
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.45);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .dialog:focus {
    outline: none;
  }
  h2 {
    margin: 0;
    font-size: 15px;
  }
  input {
    font: inherit;
    color: var(--text);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 6px 8px;
  }
  .scanning {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    color: var(--text-dim);
    font-size: 12px;
  }
  .spinner {
    width: 10px;
    height: 10px;
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
  .list {
    flex: 1;
    min-height: 120px;
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    padding: 4px 0;
  }
  h3 {
    margin: 6px 10px 2px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-dim);
  }
  .entry {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 4px 10px 4px 18px;
    cursor: pointer;
  }
  .entry.current {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
  }
  .entry.unusable {
    cursor: default;
    color: var(--text-dim);
    opacity: 0.6;
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .reason {
    font-size: 12px;
    white-space: nowrap;
  }
  .empty {
    margin: 12px;
    color: var(--text-dim);
  }
  .buttons {
    display: flex;
    gap: 8px;
  }
  .spacer {
    flex: 1;
  }
</style>
