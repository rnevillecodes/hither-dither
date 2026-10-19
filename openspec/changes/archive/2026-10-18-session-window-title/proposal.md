# Proposal

## Why

The window title always says "HitherDither", so with several tabs open, or the tab strip scrolled, there is no quick way to see which session is being worked on or whether it has unsaved changes. Desktop apps usually show the open document in the title bar, and the taskbar and window switcher show it too.

## What Changes

- **With a saved session:** while the active tab has a session file, the window title is the session's name without `.hdsession.json`, then " — HitherDither". For example, "mix — HitherDither".
- **Unsaved changes:** while that tab has unsaved changes, a "•" follows the name: "mix • — HitherDither".
- **Otherwise:** the title is "HitherDither" when no tab is open, when the active tab has never been saved as a session, or while it is still loading.
- **Keeping it current:** the title follows switching tabs, saving or Save As (a new name), changes becoming unsaved or saved, opening a session, and closing tabs.

## Capabilities

### New Capabilities
- `window-title`: what the main window's title shows for the active tab's session.

### Modified Capabilities
<!-- none -->

## Impact

- **Frontend:**
  - a new pure helper, `src/lib/windowTitle.ts`, with vitest cases;
  - an `$effect` in `src/App.svelte` that sets the title through the Tauri window API.
- **Permissions:** `src-tauri/capabilities/default.json` gains `core:window:allow-set-title`.
- **Docs:** `docs/acceptance.md`.
- No backend or file format changes.
