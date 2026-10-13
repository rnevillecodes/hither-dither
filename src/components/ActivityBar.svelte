<script lang="ts">
  import { clickSidebarView, layout } from "../lib/layout.svelte";
  import { SIDEBAR_VIEWS, type SidebarView } from "../lib/sidebar";
  import { activeTab } from "../lib/tabs.svelte";

  /** Below this width the sidebar sits under the waveform and the bar runs across (matches App.svelte). */
  const NARROW = "(max-width: 820px)";

  const names: Record<SidebarView, string> = { metrics: "Metrics", effects: "Effects" };

  const effectCount = $derived(activeTab()?.chain?.effects.length ?? 0);

  let horizontal = $state(window.matchMedia(NARROW).matches);
  $effect(() => {
    const query = window.matchMedia(NARROW);
    const update = () => (horizontal = query.matches);
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  });

  let buttons: Partial<Record<SidebarView, HTMLButtonElement>> = $state({});

  function onKeydown(e: KeyboardEvent, view: SidebarView) {
    const back = horizontal ? "ArrowLeft" : "ArrowUp";
    const forward = horizontal ? "ArrowRight" : "ArrowDown";
    if (e.key !== back && e.key !== forward) return;
    e.preventDefault();
    const i = SIDEBAR_VIEWS.indexOf(view) + (e.key === forward ? 1 : -1);
    buttons[SIDEBAR_VIEWS[(i + SIDEBAR_VIEWS.length) % SIDEBAR_VIEWS.length]]?.focus();
  }
</script>

<div class="activity-bar" role="tablist" aria-label="Sidebar views" aria-orientation={horizontal ? "horizontal" : "vertical"}>
  {#each SIDEBAR_VIEWS as view (view)}
    {@const shown = layout.sidebarOpen && layout.sidebarView === view}
    <button
      bind:this={buttons[view]}
      role="tab"
      class:active={shown}
      aria-selected={layout.sidebarView === view}
      aria-expanded={layout.sidebarView === view ? layout.sidebarOpen : undefined}
      aria-controls="sidebar"
      aria-label={names[view]}
      title={shown ? `Hide ${names[view]}` : names[view]}
      tabindex={layout.sidebarView === view ? 0 : -1}
      onclick={() => clickSidebarView(view)}
      onkeydown={(e) => onKeydown(e, view)}
    >
      {#if view === "metrics"}
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <path d="M1.5 14h13" />
          <rect x="2.5" y="8" width="2.6" height="6" rx="0.5" />
          <rect x="6.7" y="3" width="2.6" height="11" rx="0.5" />
          <rect x="10.9" y="6" width="2.6" height="8" rx="0.5" />
        </svg>
      {:else}
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <path d="M6 1.5v3M10 1.5v3" />
          <path d="M4 4.5h8v2.5a4 4 0 0 1-8 0z" />
          <path d="M8 11v3.5" />
        </svg>
        {#if effectCount > 0}
          <span class="badge" aria-hidden="true">{effectCount}</span>
        {/if}
      {/if}
    </button>
  {/each}
</div>

<style>
  .activity-bar {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 2px;
    padding: 0;
    background: var(--panel-bg);
    border-left: 1px solid var(--border);
  }
  button {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    height: 40px;
    padding: 0;
    border: none;
    border-radius: 0;
    background: transparent;
    color: var(--text-dim);
  }
  button:hover {
    color: var(--text);
    background: transparent;
  }
  button.active {
    color: var(--accent);
  }
  svg {
    width: 20px;
    height: 20px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .badge {
    position: absolute;
    right: 6px;
    bottom: 5px;
    min-width: 15px;
    height: 15px;
    padding: 0 4px;
    border-radius: 8px;
    background: var(--accent);
    color: var(--accent-text);
    font-size: 10px;
    font-weight: 600;
    line-height: 15px;
    text-align: center;
  }
  @media (max-width: 820px) {
    .activity-bar {
      flex-direction: row;
      padding: 0 6px;
      border-left: none;
      border-top: 1px solid var(--border);
    }
    button {
      width: 44px;
      height: 36px;
    }
  }
</style>
