<script lang="ts">
  import MetadataPanel from "./components/MetadataPanel.svelte";
  import Notices from "./components/Notices.svelte";
  import ProgressOverlay from "./components/ProgressOverlay.svelte";
  import StatsPanel from "./components/StatsPanel.svelte";
  import Toolbar from "./components/Toolbar.svelte";
  import Transport from "./components/Transport.svelte";
  import Waveform from "./components/Waveform.svelte";
  import { openDialog } from "./lib/actions";
  import { app, setSelection } from "./lib/app.svelte";
  import { dropHover } from "./lib/dropHover.svelte";
  import { initTransport, resetTransport, seek, togglePlay, transport } from "./lib/transport.svelte";
  import { zoomIn, zoomOut, zoomToFit } from "./lib/view.svelte";

  void initTransport();

  // A new file stops playback (the backend does too) and resets the playhead.
  let lastGeneration = 0;
  $effect(() => {
    if (app.generation !== lastGeneration) {
      lastGeneration = app.generation;
      resetTransport();
    }
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
  <Toolbar>
    {#snippet transport()}<Transport />{/snippet}
  </Toolbar>

  <main>
    <section class="wave-area">
      {#if app.meta}
        <Waveform meta={app.meta} playhead={transport.playhead} onseek={seek} />
      {:else}
        <div class="empty">
          <p class="big">Open a WAV file to inspect it</p>
          <p>Use <b>File › Open…</b> (Ctrl/Cmd+O) or drop a file onto this window.</p>
          <button class="primary" onclick={() => void openDialog()}>Open…</button>
        </div>
      {/if}
    </section>

    <aside>
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
  </main>

  {#if dropHover.active}
    <div class="drop-hint">Drop a WAV file to open it</div>
  {/if}
  {#if app.loading?.visible}
    <ProgressOverlay name={app.loading.name} fraction={app.loading.fraction} />
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
    main {
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
