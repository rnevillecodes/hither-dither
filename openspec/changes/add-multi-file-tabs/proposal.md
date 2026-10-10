# Proposal

## Why

hither-dither holds only one WAV file at a time. Opening a second file replaces the first, and dropping several files loads just one. Comparing takes (levels, loudness, clipping) means reopening files over and over. The app also still uses the default Tauri logo, so it's hard to spot in the taskbar or dock and looks unfinished.

## What Changes

- Several files can be open at once, each in its own tab. A tab strip between the toolbar and the waveform shows one tab per file. Each tab has its file name and a close button, and the active tab is highlighted.
- Each tab keeps its own waveform view: zoom and scroll position, selection, statistics scope and playhead. Switching tabs brings that state back.
- **BREAKING** (behaviour): opening a file adds a new tab instead of replacing the current file. If the file is already open, its tab is activated instead.
- The Open dialog lets the user pick several files. Every valid file opens in its own tab.
- **BREAKING** (behaviour): dropping several files opens all of them, one tab each, instead of only the first plus a "one file at a time" notice.
- If a file fails to load, the user gets an error and no tab is left behind. Tabs that are already open aren't affected.
- Tabs can be closed with the tab's close button, a middle-click, File › Close Tab, or Ctrl/Cmd+W. Closing the last tab brings back the empty "Open a WAV file" screen. Ctrl+Tab / Ctrl+Shift+Tab switch to the next and previous tab.
- Playback applies to the active tab only. Switching or closing the playing tab stops playback.
- The toolbar no longer has an **Open…** button. Files are opened from **File › Open…** (Ctrl/Cmd+O), by dropping them, or with the Open button on the empty screen.
- The details panel (metadata and statistics) is hidden while no file is open. With files open, a toolbar button shows or hides it, and the choice is remembered.
- A new, purpose-designed app icon replaces the default Tauri logo in the window, taskbar/dock, installers and favicon.

## Capabilities

### New Capabilities
- `file-tabs`: multiple open files shown as tabs, including switching, closing, per-tab view state, keyboard shortcuts and how playback behaves across tabs.
- `app-branding`: the application icon and how it appears across platforms and sizes.

### Modified Capabilities
- `wav-import`: the Open dialog allows multiple selection and opens files in tabs. Drag-and-drop opens every dropped file. A failed load keeps all open tabs. The toolbar has no Open button.
- `audio-stats`: the details panel is hidden when no file is open and can be toggled from the toolbar.

Note: `wav-import` is still defined only in the unarchived change `add-wav-analyser-app` (`openspec/specs/` is empty). Archive that change first so this delta has a main spec to modify.

## Impact

- **Rust backend** (`src-tauri/src/state.rs`, `commands.rs`, `lib.rs`): the single-file `AppState` becomes a map of open files keyed by an ID. File commands (`get_metadata`, `get_peaks`, `compute_stats`, `get_clip_runs`, `play`) take a file ID. New `close_file` and `cancel_open` commands. A new open no longer cancels other opens in progress. `docs/ipc.md` is updated.
- **Frontend** (`src/lib/app.svelte.ts`, `actions.ts`, `api.ts`, `menu.ts`, `transport.svelte.ts`, `view.svelte.ts`, `App.svelte`, `Toolbar.svelte`, `Waveform.svelte`): per-tab document state, a new `TabBar` component, multi-file open/drop, a Close Tab menu item, keyboard shortcuts, and a details-panel toggle.
- **Assets**: new icon source (SVG) in the repo. `src-tauri/icons/*` and `public/favicon.png` are regenerated with `tauri icon`.
- **Docs**: `docs/acceptance.md` gets rows for the new and changed scenarios.
- No new runtime dependencies.
