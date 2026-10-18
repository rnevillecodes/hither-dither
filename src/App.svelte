<script lang="ts">
  import ActivityBar from "./components/ActivityBar.svelte";
  import CurveDialog from "./components/CurveDialog.svelte";
  import EffectBrowser from "./components/EffectBrowser.svelte";
  import EffectsPanel from "./components/EffectsPanel.svelte";
  import GenericControls from "./components/GenericControls.svelte";
  import MetadataPanel from "./components/MetadataPanel.svelte";
  import LevelMeter from "./components/LevelMeter.svelte";
  import Notices from "./components/Notices.svelte";
  import PluginManager from "./components/PluginManager.svelte";
  import PresetManager from "./components/PresetManager.svelte";
  import PresetNameDialog from "./components/PresetNameDialog.svelte";
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
    canGroup,
    canRenameGroup,
    canUngroup,
    canInsertPreset,
    canSaveGroupPreset,
    canEdit,
    canRedo,
    canSave,
    canSaveAs,
    canExport,
    canReplaceAudio,
    canUndo,
    redo,
    save,
    saveAs,
    exportAudio,
    groupSelected,
    startRenameGroup,
    undo,
    ungroup,
  } from "./lib/editActions";
  import { takeStartupNotices } from "./lib/api";
  import { initEffects } from "./lib/effects.svelte";
  import { onQuit, setMenuChecked, setMenuEnabled, setPresetMenu } from "./lib/menu";
  import { presets, refreshPresets } from "./lib/presets.svelte";
  import { notify } from "./lib/notices.svelte";
  import { initPlugins } from "./lib/plugins.svelte";
  import { runScrollBenchmark } from "./lib/perf";
  import { readSession, rememberedPath, rememberedTabs, writeSession } from "./lib/session";
  import { samePath } from "./lib/tabOrder";
  import { activeTab, tabs } from "./lib/tabs.svelte";
  import { goToStart, initTransport, seek, togglePlay, transport } from "./lib/transport.svelte";
  import { zoomIn, zoomOut, zoomToFit } from "./lib/view.svelte";

  void initTransport();
  initEffects();
  initPlugins();
  void refreshPresets();

  // Plugins disabled because the app closed unexpectedly while one was loading, and saved
  // presets that could not be read.
  void takeStartupNotices()
    .then((list) => list.forEach((n) => notify("warning", n.title, n.message)))
    .catch(() => {});

  // Reopen the last session: WAVs and session files, as the tabs were remembered. Session
  // files are read before their tabs appear, so nothing is remembered until this finishes,
  // or an empty or partial list would overwrite the remembered one.
  const saved = readSession();
  let restoring = $state(saved.paths.length > 0);
  if (saved.paths.length > 0) {
    void openPaths(saved.paths).then(() => {
      restoring = false;
      const { active } = saved;
      const tab = active === null ? undefined : tabs.list.find((t) => samePath(rememberedPath(t), active));
      if (tab?.status === "ready") activateTab(tab.id);
    });
  }

  $effect(() => {
    const remembered = rememberedTabs(tabs.list, activeTab());
    if (!restoring) writeSession(remembered.paths, remembered.active);
  });

  const active = $derived(activeTab());
  const showSidebar = $derived(tabs.list.length > 0 && layout.sidebarOpen);

  $effect(() => {
    void setMenuEnabled({
      "close-tab": tabs.list.length > 0,
      save: canSave(),
      "save-as": canSaveAs(),
      export: canExport(),
      "replace-audio": canReplaceAudio(),
      undo: canUndo(),
      redo: canRedo(),
      crop: canEdit("crop"),
      delete: canEdit("delete"),
      silence: canEdit("silence"),
      "fade-in": canEdit("fadeIn"),
      "fade-out": canEdit("fadeOut"),
      "add-effect": canAddEffect(),
      "group-effects": canGroup(),
      ungroup: canUngroup(),
      "rename-group": canRenameGroup(),
      "save-group-preset": canSaveGroupPreset(),
      "bypass-all": canBypassAll(),
    });
    void setMenuChecked({ "bypass-all": bypassAllOn() });
  });

  // Effects › Insert Preset follows saves, renames and deletes.
  $effect(() => {
    void setPresetMenu(presets.list, canInsertPreset());
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
    if (mod && !e.shiftKey && e.key.toLowerCase() === "e") {
      e.preventDefault();
      void exportAudio();
      return;
    }
    if (mod && e.key.toLowerCase() === "s") {
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
    // Also stops the webview's own Ctrl+G (find next) and Ctrl+Shift+G (find previous).
    if (mod && e.key.toLowerCase() === "g") {
      e.preventDefault();
      void (e.shiftKey ? ungroup() : groupSelected());
      return;
    }
    // Also stops the webview reloading the page.
    if (mod && !e.shiftKey && e.key.toLowerCase() === "r") {
      e.preventDefault();
      startRenameGroup();
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
        // Highlighted effects first; the waveform selection with the next Escape.
        if (active?.fxSelected.length) {
          active.fxSelected = [];
          active.fxAnchor = null;
        } else if (app.selection) void setSelection(null);
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

  <main class:with-bar={tabs.list.length > 0} class:with-sidebar={showSidebar}>
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
          <ProgressOverlay name={active.name} fraction={active.progress.fraction} verb="Exporting" />
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

    {#if active && app.meta}
      <!-- Fresh for each file (and its channels): playback stops on switching tabs anyway. -->
      {#key `${active.id}:${app.meta.channel_labels.join()}`}
        <LevelMeter labels={app.meta.channel_labels} />
      {/key}
    {/if}

    {#if showSidebar}
      <aside id="sidebar" aria-labelledby="sidebar-title">
        <h2 id="sidebar-title" class="view-title">{layout.sidebarView === "metrics" ? "Metrics" : "Effects"}</h2>
        <div class="view">
          {#if active && app.meta}
            {#if layout.sidebarView === "effects"}
              <EffectsPanel tab={active} titled={false} />
            {:else}
              <MetadataPanel meta={app.meta} />
              <StatsPanel
                stats={app.stats}
                labels={app.meta.channel_labels}
                scope={app.selection ? "Selection" : "Whole file"}
                pending={app.statsPending}
                updating={!app.selection && app.analysing}
              />
            {/if}
          {:else}
            <p class="dim">
              {layout.sidebarView === "effects"
                ? "The file's effects appear here."
                : "File details and level statistics appear here."}
            </p>
          {/if}
        </div>
      </aside>
    {/if}
    {#if tabs.list.length > 0}
      <ActivityBar />
    {/if}
  </main>

  {#if dropHover.active}
    <div class="drop-hint">Drop WAV or session files to open them</div>
  {/if}
  <Notices />
  <CurveDialog />
  <EffectBrowser />
  <PluginManager />
  <PresetManager />
  <PresetNameDialog />
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
    grid-template-areas: "wave";
  }
  /* The activity bar sits on the window's right edge, with the sidebar between it and the
     waveform, and the level meter right beside the waveform. */
  main.with-bar {
    grid-template-columns: 1fr auto 44px;
    grid-template-areas: "wave meter bar";
  }
  main.with-sidebar {
    grid-template-columns: 1fr auto 340px 44px;
    grid-template-areas: "wave meter side bar";
  }
  .wave-area {
    grid-area: wave;
    min-width: 0;
    min-height: 0;
    position: relative;
  }
  main > :global(.activity-bar) {
    grid-area: bar;
  }
  main > :global(.meter) {
    grid-area: meter;
  }
  aside {
    grid-area: side;
    min-height: 0;
    border-left: 1px solid var(--border);
    background: var(--panel-bg);
    display: flex;
    flex-direction: column;
  }
  .view-title {
    margin: 0;
    padding: 10px 14px 0;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text);
  }
  .view {
    flex: 1;
    min-height: 0;
    padding: 12px 14px 14px;
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
    main.with-bar {
      grid-template-columns: 1fr;
      grid-template-rows: 1fr auto auto;
      grid-template-areas: "wave" "meter" "bar";
    }
    main.with-sidebar {
      grid-template-columns: 1fr;
      grid-template-rows: 1fr auto auto auto;
      grid-template-areas: "wave" "meter" "bar" "side";
    }
    aside {
      border-left: none;
      border-top: 1px solid var(--border);
      max-height: 40vh;
    }
  }
</style>
