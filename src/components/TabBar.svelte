<script lang="ts">
  import { activateTab, closeTab } from "../lib/actions";
  import { tabs } from "../lib/tabs.svelte";

  let strip: HTMLDivElement;

  // Keep the active tab in view when it changes (e.g. a newly opened file at the end).
  $effect(() => {
    const id = tabs.activeId;
    strip?.querySelector(`[data-tab-id="${id}"]`)?.scrollIntoView({ block: "nearest", inline: "nearest" });
  });

  function onAuxClick(e: MouseEvent, id: number) {
    if (e.button === 1) {
      e.preventDefault();
      closeTab(id);
    }
  }
</script>

<div class="tabs" role="tablist" aria-label="Open files" bind:this={strip}>
  {#each tabs.list as tab (tab.id)}
    {@const active = tab.id === tabs.activeId}
    <div
      class="tab"
      class:active
      role="presentation"
      data-tab-id={tab.id}
      onauxclick={(e) => onAuxClick(e, tab.id)}
      onmousedown={(e) => e.button === 1 && e.preventDefault()}
    >
      <button
        class="name"
        role="tab"
        aria-selected={active}
        title={tab.path}
        onclick={() => activateTab(tab.id)}
      >
        {#if tab.status === "loading"}
          <span class="spinner" aria-label="Loading"></span>
        {/if}
        <span class="label">{tab.name}</span>
      </button>
      <button class="close" aria-label="Close {tab.name}" title="Close (Ctrl/Cmd+W)" onclick={() => closeTab(tab.id)}>
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M2 2l6 6M8 2l-6 6" /></svg>
      </button>
    </div>
  {/each}
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
