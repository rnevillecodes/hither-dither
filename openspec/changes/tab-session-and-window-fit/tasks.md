# Tasks

## 1. Window: launch maximized, no toolbar on the empty screen

- [x] 1.1 Add `"maximized": true` and `"center": true` to the `main` window in `src-tauri/tauri.conf.json`. Verify with `npm run tauri dev`: the window opens maximized with the title bar, menu and taskbar visible and the transport row fully on screen; clicking restore gives a normal, resizable, centred window.
- [x] 1.2 In `src/App.svelte`, render `<Toolbar>` only while `tabs.list.length > 0`, and remove the now-redundant `tabs.list.length > 0` guard around the panel toggle in `src/components/Toolbar.svelte`. Verify `npm run check` passes and manually: no toolbar strip on the empty screen at launch; opening a file shows toolbar + tab strip; closing the last tab hides both again.
- [x] 1.3 Add an `app-window` section to `docs/acceptance.md` with rows for every scenario in `specs/app-window/spec.md` (laptop screen, restore down, empty screen, toolbar returns, last tab closed), and update row I12 so it expects no toolbar at all on the empty screen. Mention this change's specs in the checklist intro. Verify each scenario has exactly one row.

## 2. Tab reordering by drag and drop

- [x] 2.1 Add a pure `move(ids, id, toIndex)` helper to `src/lib/tabOrder.ts` (no-op for an unknown id or same position; `toIndex` is the insertion gap index in the original list, clamped to range). Extend `src/lib/tabOrder.test.ts` with: last-to-front, front-to-back, move to its own gap (unchanged), unknown id (unchanged), and out-of-range index (clamped). Verify `npm test` passes.
- [x] 2.2 Add `moveTab(id, toIndex)` to `src/lib/actions.ts` that reorders `tabs.list` in place using `move`, without calling `activateTab` or touching playback/view state. Verify `npm run check` passes.
- [x] 2.3 Implement pointer-based dragging in `src/components/TabBar.svelte` as described in design.md: primary-button `pointerdown` on a tab (not on `.close`), pointer capture, 4px horizontal threshold, target gap from tab midpoints, insertion-bar indicator and dimmed dragged tab, edge auto-scroll, `Escape`/`pointercancel`/release far from the strip cancels, and swallow the click that ends a real drag. Verify manually against every scenario in `specs/tab-reorder/spec.md`: move `c` before `a`; indicator visible; reorder while playing (playback continues, active unchanged); reorder an inactive tab past the active one; small jitter activates without reordering; close button and middle-click still close; Escape cancels; reorder with 15 tabs scrolls the strip.
- [x] 2.4 Verify order-dependent behaviour manually: after dragging `c` to the front, Ctrl+Tab from `c` goes to `a`, and closing active `a` in `c, a, b` activates `b`.
- [x] 2.5 Add a `tab-reorder` section to `docs/acceptance.md` with one row per scenario in `specs/tab-reorder/spec.md`, citing `tabOrder.test.ts` as Auto evidence where it applies. Verify each scenario has a row.

## 3. Session restore

- [x] 3.1 Create `src/lib/session.ts` with `readSession()` and `writeSession(paths, active)` storing `{ v: 1, paths, active }` under `hither-dither.session`, all `localStorage` access in try/catch, and malformed JSON / unknown `v` / non-string entries reading as an empty session. Add `src/lib/session.test.ts` (stub `localStorage`) covering round-trip, missing key, malformed JSON, wrong version, and a throwing `localStorage`. Verify `npm test` passes.
- [x] 3.2 In `src/App.svelte`, read the session at startup and, if it has paths, call `openPaths(paths)`; when that resolves, `activateTab` the tab whose path matches `active` (via `samePath`) if it is still open. Then add a root `$effect` that writes the current tab paths (in order) and active path whenever they change. Make sure the read and `openPaths` call happen before the effect first runs. Verify `npm run check` passes.
- [x] 3.3 Verify manually against every scenario in `specs/session-restore/spec.md`: open `a`, `b`, make `a` active, quit and relaunch (both restored in order, `a` active); close a tab and kill the process from Task Manager, relaunch (closed tab not restored); open `not-a-wav.wav`, relaunch (not restored); rename a remembered file between runs (one "Could not open file" notice, others restored, renamed file gone from the next session); close a large file while it is restoring (no error, not restored next time); reordered tabs restore in the new order; restored files open zoomed to fit with no selection; first launch with no session shows the empty screen.
- [x] 3.4 Add a `session-restore` section to `docs/acceptance.md` with one row per scenario in `specs/session-restore/spec.md`, citing `session.test.ts` where it applies. Verify each scenario has a row.

## 4. Integration checks

- [x] 4.1 Run `npm run check`, `npm test`, `npm run lint:rust` and `npm run test:rust`, and verify all pass.
- [x] 4.2 Build a release bundle with `npm run tauri build` and, in the installed app, verify: it launches maximized; a session from a previous run of the installed app is restored; dragging tabs and OS file drops onto the window both still work (drag-reorder must not break `dragDropEnabled` file drops).
