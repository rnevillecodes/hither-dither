<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import * as api from "../lib/api";
  import { matches, scanText } from "../lib/effectBrowser";
  import { effectsUi } from "../lib/effects.svelte";
  import { notify } from "../lib/notices.svelte";
  import { plugins, refreshPlugins } from "../lib/plugins.svelte";

  const STATUS: Record<api.PluginStatus, string> = {
    ready: "Ready",
    failed: "Failed",
    incompatible: "Incompatible",
    disabled: "Disabled",
    noEffects: "No usable effects",
  };

  let dialog = $state<HTMLDivElement>();
  let folders = $state<api.PluginFolders>({ standard: [], extra: [] });
  let query = $state("");

  const scanning = $derived(scanText(plugins.progress));
  const shown = $derived(plugins.list.filter((p) => matches(p, query)));

  $effect(() => {
    if (!effectsUi.managerOpen) return;
    query = "";
    void refreshPlugins();
    void api
      .pluginFolders()
      .then((f) => (folders = f))
      .catch(() => {});
    queueMicrotask(() => dialog?.focus());
  });

  function close() {
    effectsUi.managerOpen = false;
  }

  async function saveFolders(extra: string[]) {
    try {
      folders = await api.setPluginFolders(extra);
    } catch (e) {
      notify("error", "Could not change the plugin folders", String(e));
    }
  }

  async function addFolder() {
    const path = await open({ directory: true, multiple: false, title: "Add a VST3 folder" });
    if (typeof path === "string") await saveFolders([...folders.extra, path]);
  }

  async function setEnabled(p: api.PluginEntry, enabled: boolean) {
    if (!p.classId) return;
    try {
      await api.setPluginEnabled(p.classId, enabled);
    } catch (e) {
      notify("error", `Could not ${enabled ? "enable" : "disable"} ${p.name}`, String(e));
    }
    await refreshPlugins();
  }

  function rescanAll() {
    void api.rescanPlugins(true).catch((e) => notify("error", "Could not rescan the plugins", String(e)));
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

{#if effectsUi.managerOpen}
  <div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && close()}>
    <div
      class="dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="manager-title"
      tabindex="-1"
      bind:this={dialog}
      onkeydown={onKeydown}
    >
      <h2 id="manager-title">Manage Plugins</h2>

      <section aria-labelledby="folders-heading">
        <h3 id="folders-heading">Folders</h3>
        <ul class="folders">
          {#each folders.standard as path (path)}
            <li><span class="path" title={path}>{path}</span><span class="tag">Standard</span></li>
          {/each}
          {#each folders.extra as path (path)}
            <li>
              <span class="path" title={path}>{path}</span>
              <button
                class="link"
                aria-label="Remove {path}"
                onclick={() => void saveFolders(folders.extra.filter((f) => f !== path))}>Remove</button
              >
            </li>
          {/each}
        </ul>
        <button onclick={() => void addFolder()}>Add Folder…</button>
      </section>

      <section class="plugins" aria-labelledby="plugins-heading">
        <div class="row">
          <h3 id="plugins-heading">Plugins</h3>
          {#if scanning}
            <span class="scanning" role="status"><span class="spinner" aria-hidden="true"></span>{scanning}</span>
          {/if}
          <span class="spacer"></span>
          <button disabled={!!scanning} onclick={rescanAll}>Rescan All</button>
        </div>
        <input type="search" placeholder="Filter by name or vendor" aria-label="Filter plugins" bind:value={query} />
        <ul class="list">
          {#each shown as p (`${p.path}|${p.classId}`)}
            {@const toggleable = !!p.classId && (p.status === "ready" || p.status === "disabled")}
            <li class:off={p.status !== "ready"}>
              <input
                type="checkbox"
                checked={p.status === "ready"}
                disabled={!toggleable}
                aria-label="Enable {p.name}"
                title={toggleable ? (p.status === "ready" ? "Disable" : "Enable") : undefined}
                onchange={(e) => void setEnabled(p, e.currentTarget.checked)}
              />
              <div class="info">
                <div class="line">
                  <span class="name">{p.name}</span>
                  {#if p.version}<span class="dim mono">{p.version}</span>{/if}
                  <span class="status {p.status}">{STATUS[p.status]}</span>
                </div>
                <div class="line dim">
                  {#if p.vendor}<span class="vendor">{p.vendor}</span>{/if}
                  <span class="path" title={p.path}>{p.path}</span>
                </div>
                {#if p.reason && p.status !== "noEffects"}<div class="reason">{p.reason}</div>{/if}
              </div>
            </li>
          {:else}
            <li class="empty dim">
              {#if query.trim()}
                No plugins match “{query.trim()}”.
              {:else if scanning}
                Plugins appear here as they are found.
              {:else}
                No plugins were found in these folders.
              {/if}
            </li>
          {/each}
        </ul>
      </section>

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
    width: min(640px, calc(100vw - 32px));
    height: min(640px, calc(100vh - 48px));
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
  h3 {
    margin: 0;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-dim);
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 6px;
    align-items: flex-start;
  }
  .plugins {
    flex: 1;
    min-height: 0;
    align-items: stretch;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .spacer {
    flex: 1;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .folders {
    align-self: stretch;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .folders li {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tag {
    font-size: 11px;
    color: var(--text-dim);
  }
  .link {
    border: none;
    background: none;
    padding: 0 2px;
    color: var(--accent);
  }
  .link:hover:not(:disabled) {
    background: none;
    text-decoration: underline;
  }
  input[type="search"] {
    font: inherit;
    color: var(--text);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 5px 8px;
  }
  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
  }
  .list li {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 6px 10px;
    border-bottom: 1px solid var(--wf-grid);
  }
  .list li:last-child {
    border-bottom: none;
  }
  .list input {
    margin-top: 3px;
  }
  .info {
    flex: 1;
    min-width: 0;
  }
  .line {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
  }
  .name {
    font-weight: 600;
  }
  li.off .name {
    color: var(--text-dim);
  }
  .vendor {
    flex-shrink: 0;
  }
  .status {
    margin-left: auto;
    flex-shrink: 0;
    font-size: 12px;
  }
  .status.ready {
    color: var(--accent);
  }
  .status.failed,
  .status.incompatible {
    color: var(--warning);
  }
  .status.disabled,
  .status.noEffects {
    color: var(--text-dim);
  }
  .reason {
    color: var(--warning);
    font-size: 12px;
  }
  .dim {
    color: var(--text-dim);
    font-size: 12px;
  }
  .empty {
    padding: 12px;
  }
  .scanning {
    display: flex;
    align-items: center;
    gap: 8px;
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
  .buttons {
    display: flex;
    justify-content: flex-end;
  }
</style>
