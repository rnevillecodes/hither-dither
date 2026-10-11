# Proposal

## Why

Every launch starts from scratch: files that were open last time have to be found and reopened one by one, and their tabs can only ever sit in the order they were opened. The window also opens at a fixed 1200×760, which is taller than many laptop screens once the title bar, menu and taskbar are counted, so the bottom of the app (transport and waveform edge) is cut off. And the empty screen still shows an empty toolbar strip that has nothing in it.

## What Changes

- **Restore the last session on launch**: remember which files are open (in tab order) and which tab is active, and reopen them automatically the next time the app starts. Files that no longer exist or fail to load are skipped with the usual "Could not open file" notice; the others still open.
- **Reorder tabs by drag and drop**: the user can drag a tab left or right along the tab strip to move it. The new order is used for Ctrl+Tab cycling, for choosing the neighbour after a close, and is what the session remembers.
- **Hide the toolbar on the empty screen**: when no files are open or loading, the top toolbar is not rendered, so the empty "Open a WAV file" screen fills the window below the native menu.
- **Launch maximized**: the main window opens maximized so it fits the screen on any display size; the regular window controls (restore, resize) still work.

Assumptions recorded from the request:
- "Remember opened files" means restoring the previous session, not a recent-files menu.
- "Full screen" means maximized (title bar, menu and taskbar stay visible), not borderless fullscreen.
- Only the file list, order and active tab are restored; per-tab zoom, scroll, selection and playhead are not.

Non-goals: File › Open Recent; restoring window size/position between runs; dragging tabs out into new windows; keyboard shortcuts for moving tabs.

## Capabilities

### New Capabilities
- `session-restore`: remembering the open files, their tab order and the active tab across app restarts, and reopening them on launch.
- `tab-reorder`: moving tabs within the tab strip by drag and drop, and how the resulting order affects other tab behaviour.
- `app-window`: how the main window is presented — initial window state on launch and which chrome (the toolbar) is shown on the empty screen.

### Modified Capabilities
<!-- None: openspec/specs/ has no published specs yet (earlier changes such as add-multi-file-tabs are not archived), so all requirements here are added as new capabilities. -->

## Impact

- `src-tauri/tauri.conf.json`: main window config (`maximized`, `center`).
- `src/App.svelte`: render `<Toolbar>` only while at least one tab exists; restore the session at startup.
- `src/components/TabBar.svelte`: pointer-based drag-to-reorder with a drop indicator.
- `src/lib/tabOrder.ts` (+ tests): pure `move` helper for reordering ids.
- `src/lib/actions.ts`: `moveTab` action; `openPaths` reused for restoring.
- New `src/lib/session.ts` (+ tests): read/write the saved session in `localStorage`, alongside the existing `layout.svelte.ts` preference.
- No new dependencies, no new Tauri commands or capabilities.
