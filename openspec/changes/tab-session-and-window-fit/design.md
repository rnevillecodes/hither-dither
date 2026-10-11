# Design

## Context

- Tabs live in `src/lib/tabs.svelte.ts` (`tabs.list`, `tabs.activeId`). All order-dependent logic (`cycleTab`, `removeTab` → `neighbourAfterClose`) already reads `tabs.list` order, so reordering the array is enough for those to follow.
- `openPaths()` in `src/lib/actions.ts` pushes one tab per path synchronously, then loads them one at a time through a queue; failed loads are removed and reported with `notify("error", "Could not open file", …)`, and closes during load are handled. It is exactly the behaviour the restore spec asks for.
- The only persisted preference today is `layout.detailsOpen` in `localStorage` (`src/lib/layout.svelte.ts`), wrapped in try/catch.
- The main window is configured statically in `src-tauri/tauri.conf.json` (1200×760, min 640×420, `dragDropEnabled: true` for OS file drops).
- `Toolbar.svelte` is always rendered by `App.svelte`; with no tabs it renders an empty 46px strip.

## Goals / Non-Goals

**Goals:**
- Reuse `openPaths` for restoring so restore gets queueing, progress, error notices and close-during-load for free.
- Keep reordering a pure array operation, unit-tested like the rest of `tabOrder.ts`.
- No new Tauri plugins, commands or capabilities.

**Non-Goals:**
- Persisting per-tab view state or window geometry.
- Keyboard-driven tab moving.

## Decisions

### Session storage: `localStorage`, JSON, written on every change
New `src/lib/session.ts` exposes `readSession(): { paths: string[]; active: string | null }` and `writeSession(paths, active)`, storing `{ "v": 1, "paths": [...], "active": "..." }` under `hither-dither.session`. Unknown versions or malformed JSON read as an empty session. All access is in try/catch, mirroring `layout.svelte.ts`.

Saving happens in a root `$effect` in `App.svelte` that reads `tabs.list.map(t => t.path)` and the active tab's path and writes whenever they change. Writing on change (rather than on window close) keeps the session correct after crashes or forced termination and avoids a Tauri close-request hook.

Loading tabs are included (so quitting mid-restore doesn't lose files); failed loads drop out naturally because `openPaths` removes their tabs.

*Alternatives:* `tauri-plugin-store` or a Rust-side JSON file in the app data dir — more robust against webview data being cleared, but adds a dependency, a capability and IPC for a few hundred bytes. Not worth it now; `session.ts` is the only place to change if we switch later.

### Restore order of operations
At startup (`App.svelte` script, before the save effect first runs) read the session, and if it has paths call `openPaths(paths)`. Because `openPaths` pushes all tabs synchronously, the save effect's first run already sees them and never overwrites the stored session with an empty list. When the returned promise resolves, if a tab whose path matches `active` (via `samePath`) is still open, `activateTab` it.

*Alternative:* restore from `main.ts` after mount — works too, but keeping read, restore and save side by side in `App.svelte` makes the "read before first save" ordering obvious.

### Tab dragging: pointer events, not HTML5 drag-and-drop
`TabBar.svelte` handles `pointerdown` on a tab (primary button only, ignoring presses on `.close`), calls `setPointerCapture`, and only enters drag mode after the pointer moves more than 4px horizontally. While dragging:
- the target index is computed from the pointer x against the midpoints of the other tabs' bounding boxes;
- an absolutely positioned insertion bar is drawn at that gap, and the dragged tab is dimmed;
- near the strip's left/right edges the strip auto-scrolls so off-screen positions are reachable.

On `pointerup` within the strip's vertical band (with some slack) the tab moves via a new `moveTab(id, toIndex)` action; `Escape`, `pointercancel` or release far outside the strip cancels. A click that ends a real drag is swallowed so it doesn't activate the tab; a sub-threshold press falls through to the existing `onclick`.

`moveTab` uses a pure `move(ids, id, toIndex)` helper in `tabOrder.ts` to compute the new order, then reorders `tabs.list` in place. It touches nothing else — no `activateTab`, no playback calls — so view state and playback are untouched.

*Why not HTML5 DnD:* with `dragDropEnabled: true` (needed for OS file drops), Tauri's WebView2 on Windows intercepts drag events and HTML5 `dragover`/`drop` inside the page don't fire reliably. Pointer events also give us the threshold/click distinction and Escape handling directly.

### Launch maximized via window config
Set `"maximized": true` and `"center": true` on the `main` window in `tauri.conf.json`. Maximized respects the OS work area (taskbar stays visible), which is what fixes the bottom cut-off. `center` makes "restore down" land on-screen.

*Alternative:* `fullscreen: true` — rejected by the user (hides title bar/taskbar). Computing a size from `currentMonitor()` at startup — more code and a visible resize; the OS maximize already does it right.

### Hide the toolbar when there are no tabs
Wrap `<Toolbar>` in `App.svelte` with the same `{#if tabs.list.length > 0}` guard the `<TabBar>` uses, and drop the now-redundant `tabs.list.length > 0` check around the panel toggle inside `Toolbar.svelte`. The empty screen's `main` is already `flex: 1`, so it grows to fill the space.

## Risks / Trade-offs

- [Webview storage cleared or origin changes (dev `localhost:1420` vs bundled app) → sessions don't carry across] → Acceptable: dev and release are separate sessions by design; worst case the app starts empty.
- [Restoring many or very large files makes startup slow] → Loads are queued and the app stays usable; each tab can be closed to cancel. No cap for now.
- [A remembered file sits on a disconnected network/removable drive and the open call is slow to fail] → Same as opening it manually; the tab shows its loading state and can be closed.
- [Restore-down size (1200×760) is still taller than a small laptop's work area] → The user can resize; the window is no longer launched in that state. Revisit default size if it comes up.
- [Some Linux window managers ignore the maximized hint] → Falls back to the existing default size; no worse than today.
- [Pointer capture and the strip's horizontal scroll conflict on touchpads] → Only horizontal movement beyond the threshold starts a drag; plain wheel/touchpad scrolling of the strip is untouched.

## Migration Plan

No data migration: the first launch after upgrade finds no session and starts empty, as today. Rollback is reverting the change; a leftover `hither-dither.session` key is harmless.
