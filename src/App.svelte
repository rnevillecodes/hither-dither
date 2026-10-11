<script lang="ts">
  import MetadataPanel from "./components/MetadataPanel.svelte";
  import Notices from "./components/Notices.svelte";
  import ProgressOverlay from "./components/ProgressOverlay.svelte";
  import StatsPanel from "./components/StatsPanel.svelte";
  import TabBar from "./components/TabBar.svelte";
  import Toolbar from "./components/Toolbar.svelte";
  import Transport from "./components/Transport.svelte";
  import Waveform from "./components/Waveform.svelte";
  import { activateTab, closeActiveTab, cycleTab, openDialog, openPaths } from "./lib/actions";
  import { app, setSelection } from "./lib/app.svelte";
  import logoShadow from "./assets/logo-shadow.svg";
  import { dropHover } from "./lib/dropHover.svelte";
  import { layout } from "./lib/layout.svelte";
  import { setCloseTabEnabled } from "./lib/menu";
  import { runScrollBenchmark } from "./lib/perf";
  import { readSession, writeSession } from "./lib/session";
  import { samePath } from "./lib/tabOrder";
  import { activeTab, tabs } from "./lib/tabs.svelte";
  import { initTransport, seek, togglePlay, transport } from "./lib/transport.svelte";
  import { zoomIn, zoomOut, zoomToFit } from "./lib/view.svelte";

  void initTransport();

  // Reopen the last session. This pushes its tabs synchronously, so the save effect
  // below never sees an empty list first and overwrites the remembered session.
  const saved = readSession();
  if (saved.paths.length > 0) {
    void openPaths(saved.paths).then(() => {
      const { active } = saved;
      const tab = active === null ? undefined : tabs.list.find((t) => samePath(t.path, active));
      if (tab?.status === "ready") activateTab(tab.id);
    });
  }

  $effect(() => {
    writeSession(
      tabs.list.map((t) => t.path),
      activeTab()?.path ?? null,
    );
  });

  const active = $derived(activeTab());
  const showDetails = $derived(tabs.list.length > 0 && layout.detailsOpen);

  $effect(() => {
    void setCloseTabEnabled(tabs.list.length > 0);
  });

  function onKeydown(e: KeyboardEvent) {
    const target = e.target as HTMLElement | null;
    const typing =
      target?.tagName === "TEXTAREA" || (target?.tagName === "INPUT" && (target as HTMLInputElement).type !== "range");
    if (typing) return;
    const mod = e.ctrlKey || e.metaKey;
    if (mod && e.key.toLowerCase() === "o") {
      e.preventDefault();
      void openDialog();
      return;
    }
    if (mod && !e.shiftKey && e.key.toLowerCase() === "w") {
      e.preventDefault();
      closeActiveTab();
      return;
    }
    if (e.ctrlKey && e.key === "Tab") {
      e.preventDefault();
      cycleTab(e.shiftKey ? -1 : 1);
      return;
    }
    if (mod && e.shiftKey && e.key.toLowerCase() === "b" && app.meta) {
      e.preventDefault();
      runScrollBenchmark();
      return;
    }
    if (mod || e.altKey || !app.meta) return;
    switch (e.key) {
      case "+":
      case "=":
        zoomIn();
        break;
      case "-":
      case "_":
        zoomOut();
        break;
      case "f":
      case "F":
        zoomToFit();
        break;
      case "Escape":
        if (app.selection) void setSelection(null);
        break;
      case " ":
        // Always play/pause, rather than activating whichever button has focus.
        togglePlay();
        break;
      case "Home":
        seek(0);
        break;
      case "End":
        seek(app.meta.frames);
        break;
      default:
        return;
    }
    e.preventDefault();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="app">
  {#if tabs.list.length > 0}
    <Toolbar>
      {#snippet transport()}<Transport />{/snippet}
    </Toolbar>
    <TabBar />
  {/if}

  <main class:with-details={showDetails}>
    <section class="wave-area">
      {#if active && app.meta}
        {#key active.id}
          <Waveform fileId={active.id} meta={app.meta} playhead={transport.playhead} onseek={seek} />
        {/key}
      {:else if active}
        <!-- Still loading. The progress box appears once the load has lasted a moment. -->
        {#if active.progress.visible}
          <ProgressOverlay name={active.name} fraction={active.progress.fraction} />
        {/if}
      {:else}
        <div class="empty">
          <div class="watermark" style:--logo={`url("${logoShadow}")`} aria-hidden="true"></div>
          <p class="big">Open a WAV file to inspect it</p>
          <p>Use <b>File › Open…</b> (Ctrl/Cmd+O) or drop files onto this window.</p>
          <button class="primary" onclick={() => void openDialog()}>Open…</button>
        </div>
      {/if}
    </section>

    {#if showDetails}
      <aside id="details-panel" aria-label="Details">
        {#if app.meta}
          <MetadataPanel meta={app.meta} />
          <StatsPanel
            stats={app.stats}
            labels={app.meta.channel_labels}
            scope={app.selection ? "Selection" : "Whole file"}
            pending={app.statsPending}
          />
        {:else}
          <p class="dim">File details and level statistics appear here.</p>
        {/if}
      </aside>
    {/if}
  </main>

  {#if dropHover.active}
    <div class="drop-hint">Drop WAV files to open them</div>
  {/if}
  <Notices />
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100%;
  }
  main {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 1fr;
  }
  main.with-details {
    grid-template-columns: 1fr 340px;
  }
  .wave-area {
    min-width: 0;
    min-height: 0;
    position: relative;
  }
  aside {
    border-left: 1px solid var(--border);
    background: var(--panel-bg);
    padding: 14px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 22px;
  }
  .empty {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    color: var(--text-dim);
    text-align: center;
    padding: 24px;
  }
  /* An imprint of the icon's waveform, tinted with the text colour so it suits light and dark themes. */
  .watermark {
    width: min(320px, 40vh);
    aspect-ratio: 772 / 690;
    margin-bottom: 28px;
    flex-shrink: 0;
    background: var(--text);
    opacity: 0.09;
    -webkit-mask: var(--logo) center / contain no-repeat;
    mask: var(--logo) center / contain no-repeat;
    pointer-events: none;
  }
  .empty .big {
    font-size: 18px;
    color: var(--text);
    margin: 0;
  }
  .empty button {
    margin-top: 12px;
  }
  .dim {
    color: var(--text-dim);
    margin: 0;
  }
  .drop-hint {
    position: fixed;
    inset: 12px;
    border: 3px dashed var(--accent);
    border-radius: 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 18px;
    background: color-mix(in srgb, var(--bg) 80%, transparent);
    pointer-events: none;
    z-index: 30;
  }
  @media (max-width: 820px) {
    main.with-details {
      grid-template-columns: 1fr;
      grid-template-rows: 1fr auto;
    }
    aside {
      border-left: none;
      border-top: 1px solid var(--border);
      max-height: 40vh;
    }
  }
</style>
