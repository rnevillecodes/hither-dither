<script lang="ts">
  import CurveDialog from "./components/CurveDialog.svelte";
  import EffectBrowser from "./components/EffectBrowser.svelte";
  import EffectsPanel from "./components/EffectsPanel.svelte";
  import GenericControls from "./components/GenericControls.svelte";
  import MetadataPanel from "./components/MetadataPanel.svelte";
  import Notices from "./components/Notices.svelte";
  import PluginManager from "./components/PluginManager.svelte";
  import ProgressOverlay from "./components/ProgressOverlay.svelte";
  import StatsPanel from "./components/StatsPanel.svelte";
  import TabBar from "./components/TabBar.svelte";
  import Toolbar from "./components/Toolbar.svelte";
  import Transport from "./components/Transport.svelte";
  import Waveform from "./components/Waveform.svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { activateTab, closeActiveTab, confirmQuit, cycleTab, openDialog, openPaths } from "./lib/actions";
  import { app, setSelection } from "./lib/app.svelte";
  import logoShadow from "./assets/logo-shadow.svg";
  import { dropHover } from "./lib/dropHover.svelte";
  import { layout } from "./lib/layout.svelte";
  import {
    applyEdit,
    bypassAllOn,
    canAddEffect,
    canBypassAll,
    canEdit,
    canRedo,
    canSave,
    canSaveAs,
    canSaveSession,
    canUndo,
    redo,
    save,
    saveAs,
    saveSession,
    undo,
  } from "./lib/editActions";
  import { takeStartupNotices } from "./lib/api";
  import { initEffects } from "./lib/effects.svelte";
  import { onQuit, setMenuChecked, setMenuEnabled } from "./lib/menu";
  import { notify } from "./lib/notices.svelte";
  import { initPlugins } from "./lib/plugins.svelte";
  import { runScrollBenchmark } from "./lib/perf";
  import { readSession, writeSession } from "./lib/session";
  import { samePath } from "./lib/tabOrder";
  import { activeTab, tabs } from "./lib/tabs.svelte";
  import { goToStart, initTransport, seek, togglePlay, transport } from "./lib/transport.svelte";
  import { zoomIn, zoomOut, zoomToFit } from "./lib/view.svelte";

  void initTransport();
  initEffects();
  initPlugins();

  // Plugins disabled because the app closed unexpectedly while one was loading.
  void takeStartupNotices()
    .then((list) => list.forEach((n) => notify("warning", `${n.name} was disabled`, n.message)))
    .catch(() => {});

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
    void setMenuEnabled({
      "close-tab": tabs.list.length > 0,
      save: canSave(),
      "save-as": canSaveAs(),
      "save-session": canSaveSession(),
      "save-session-as": canSaveSession(),
      undo: canUndo(),
      redo: canRedo(),
      crop: canEdit("crop"),
      delete: canEdit("delete"),
      silence: canEdit("silence"),
      "fade-in": canEdit("fadeIn"),
      "fade-out": canEdit("fadeOut"),
      "add-effect": canAddEffect(),
      "bypass-all": canBypassAll(),
    });
    void setMenuChecked({ "bypass-all": bypassAllOn() });
  });

  // Closing the window (or File › Quit) asks about unsaved changes first. Tauri closes
  // the window after this handler unless it prevents the close.
  const appWindow = getCurrentWindow();
  void appWindow.onCloseRequested(async (event) => {
    if (!(await confirmQuit())) event.preventDefault();
  });
  onQuit(() => void appWindow.close());

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
    if (mod && e.key.toLowerCase() === "z") {
      e.preventDefault();
      void (e.shiftKey ? redo() : undo());
      return;
    }
    if (e.ctrlKey && !e.shiftKey && e.key.toLowerCase() === "y") {
      e.preventDefault();
      void redo();
      return;
    }
    if (mod && e.altKey && !e.shiftKey && (e.key.toLowerCase() === "s" || e.code === "KeyS")) {
      e.preventDefault();
      void saveSession();
      return;
    }
    if (mod && !e.altKey && e.key.toLowerCase() === "s") {
      e.preventDefault();
      void (e.shiftKey ? saveAs() : save());
      return;
    }
    if (mod && !e.shiftKey && e.key.toLowerCase() === "t") {
      e.preventDefault();
      void applyEdit("crop");
      return;
    }
    if (mod && !e.shiftKey && e.key.toLowerCase() === "l") {
      e.preventDefault();
      void applyEdit("silence");
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
      case "Delete":
        void applyEdit("delete");
        break;
      case "Home":
        goToStart();
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
        <!-- Remount per file; edits refetch peaks in place (keyed by peaksVersion). -->
        {#key active.id}
          <Waveform
            fileId={active.id}
            meta={app.meta}
            peaksVersion={active.peaksVersion}
            playhead={transport.playhead}
            onseek={seek}
          />
        {/key}
        {#if active.editing && active.progress.visible}
          <ProgressOverlay name={active.name} fraction={active.progress.fraction} verb="Saving" />
        {/if}
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
        {#if active && app.meta}
          <EffectsPanel tab={active} />
          <MetadataPanel meta={app.meta} />
          <StatsPanel
            stats={app.stats}
            labels={app.meta.channel_labels}
            scope={app.selection ? "Selection" : "Whole file"}
            pending={app.statsPending}
            updating={!app.selection && app.analysing}
          />
        {:else}
          <p class="dim">File details and level statistics appear here.</p>
        {/if}
      </aside>
    {/if}
  </main>

  {#if dropHover.active}
    <div class="drop-hint">Drop WAV or session files to open them</div>
  {/if}
  <Notices />
  <CurveDialog />
  <EffectBrowser />
  <PluginManager />
  <GenericControls />
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
