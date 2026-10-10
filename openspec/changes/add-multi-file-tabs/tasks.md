# Tasks

## 1. Backend: multiple loaded files

- [x] 1.1 Replace the single file and `loading` token in `AppState` (`src-tauri/src/state.rs`) with maps keyed by `FileId = u32`. Add `open(id, path, progress)` (no longer cancels other loads), `get(id)`, `close(id)` and `cancel_open(id)`, and make `compute_stats` / `clip_runs` take an ID. Update `src-tauri/tests/state.rs` and add tests: two files stay loaded side by side, a failed open under a new ID leaves the other IDs intact, `cancel_open` makes that open return `Cancelled` without touching other files, and `close` removes only its ID. Verify `npm run test:rust` passes.
- [x] 1.2 Add a `fileId` argument to `open_file`, `get_metadata`, `get_peaks`, `compute_stats`, `get_clip_runs` and `play` in `commands.rs`. Add and register the `close_file` and `cancel_open` commands in `lib.rs`. `close_file` stops the player first if it is playing that file's source (`Arc::ptr_eq`). Verify `npm run lint:rust` and `npm run test:rust` pass.
- [x] 1.3 Update `docs/ipc.md`: the `fileId` argument on each command, the new `close_file` / `cancel_open` entries, and the removal of the "newer open cancels older" note. Verify every documented command name matches the `generate_handler!` list.

## 2. Frontend: tab store and active-document facade

- [x] 2.1 Add `fileId` to the wrappers in `src/lib/api.ts`, and add `closeFile` and `cancelOpen`. Verify `npm run check` passes.
- [x] 2.2 Create `src/lib/tabs.svelte.ts` with the `Tab` type, the `tabs` list and `activeId`. Put the pure helpers in a plain `src/lib/tabOrder.ts` so they can be unit-tested: `neighbourAfterClose`, `cycle(next/prev)` with wrap-around, and `samePath` (separator-normalised, case-insensitive on Windows). Add `src/lib/tabOrder.test.ts` covering close-middle/close-last/close-only, wrap-around in both directions, and path matching. Verify `npm test` passes.
- [x] 2.3 Turn `app` in `app.svelte.ts` into a facade that forwards `meta`, `clipRuns`, `wholeStats`, `selection`, `stats` and `statsPending` to the active tab (null or false when there is none). Make `setSelection` write its async result to the tab that started the request, not to whichever tab is active. Remove `generation` and the single `loading` field. Verify `npm run check` passes and the existing vitest suites still pass.
- [ ] 2.4 Implement `activateTab(id)`: save `view.vp`, `view.userScrolled`, `transport.playhead` and `transport.playStart` into the outgoing tab, stop playback if it is playing, then restore those values from the incoming tab. Replace the `app.generation` effect in `App.svelte` with this. Verify manually: select 1.0–2.0 s and zoom in on file A, switch to B and back, and A's zoom, scroll, selection and "Selection" stats are unchanged.

## 3. Opening and closing files

- [ ] 3.1 Implement `openPaths(paths)` in `actions.ts`. For each path, activate its tab if one with a matching path already exists. Otherwise create a `loading` tab and queue the load. Loads run one at a time; a finished load becomes active, a failed one removes its tab and calls `notify("error", …)`. Switch `openDialog` to `multiple: true`, make `dropPaths` open every path, and delete the "one file at a time" notice. Verify manually: choose three fixtures in the dialog, drop `u8-mono.wav` and `float64-mono.wav` together, re-open an already open file (no duplicate tab), and drop `not-a-wav.wav` with two valid files (one error, two tabs).
- [ ] 3.2 Implement `closeTab(id)` (stop playback if needed, `cancelOpen` for a loading tab or `closeFile` otherwise, activate the neighbour via `neighbourAfterClose`) and `closeActiveTab()` with the 100 ms guard against a double fire. Verify manually: close a middle tab, close the last tab (the empty screen returns), close a loading 1 GB tab (the load stops with no error), and press Ctrl/Cmd+W once with two tabs open (only one closes).
- [ ] 3.3 Show `ProgressOverlay` only when the active tab is loading and has passed the 500 ms delay. Verify manually: start loading `large-1gb.wav`, switch to another tab and use its waveform while the load continues, then switch back and see the progress.

## 4. Tab strip, toolbar and menu UI

- [ ] 4.1 Create `src/components/TabBar.svelte` (tablist/tab roles, `aria-selected`, file name with the full-path `title`, a "×" close button with `aria-label`, middle-click to close, a spinner on loading tabs, horizontal scrolling that keeps the active tab visible). Render it in `App.svelte` between `Toolbar` and `<main>` only when there are tabs. Verify manually against the "Tab per open file", "Switching tabs" (click) and "Closing tabs" scenarios, and with 15 tabs open (the strip scrolls, the active tab stays visible).
- [ ] 4.2 Wrap `<Waveform>` in `{#key tabs.activeId}`. On mount, restore the tab's saved `vp` (resized to the current width) or fit the file. Cache the overview tile on the tab. Replace `app.generation` guards with a component-local token. Verify manually: switching between two long files shows each one's correct waveform and overview at once, with no flash of the other file.
- [ ] 4.3 Remove the Open… button from the toolbar (the empty screen keeps its own). Add Ctrl+Tab / Ctrl+Shift+Tab and Ctrl/Cmd+W to the keydown handler in `App.svelte`. Verify manually: the toolbar has no Open… button with or without files open, the empty-screen Open… button opens the dialog, and Ctrl+Tab wraps from the last tab to the first.
- [ ] 4.4 Add File › Close Tab (CmdOrCtrl+W) to `menu.ts`, disabled when there are no tabs (update its enabled state when the tab count changes). Verify manually that the menu item closes the active tab and is greyed out when nothing is open.
- [ ] 4.5 Check that transport actions affect only the active tab. Verify manually: play A to about 12 s, switch to B (playback stops), press Space (B plays from its own playhead), switch back to A (playhead at about 12 s). Also close A while it is playing (playback stops, no error).
- [ ] 4.6 Add `src/lib/layout.svelte.ts` (`detailsOpen`, saved to `localStorage` with try/catch). Render the details `<aside>` only when tabs exist and the panel is open, using a single-column layout otherwise. Add the panel toggle button (`aria-pressed`, `aria-controls`) at the right end of the toolbar, shown only while tabs exist. Verify manually against the audio-stats "Details panel visibility" scenarios: no panel or toggle with nothing open; hiding widens the waveform; the choice survives a tab switch and an app restart; on a narrow window the toggle hides the bottom panel.

## 5. App icon

- [x] 5.1 Write `src-tauri/icons/app-icon.svg` following design Decision 7. Render a preview sheet (16, 32, 128 and 512 px on white and black backgrounds) and show it to the user for sign-off. Verify that the user approves the design and that the waveform is still recognisable at 16 and 32 px.
- [ ] 5.2 Run `npx tauri icon src-tauri/icons/app-icon.svg` to regenerate `src-tauri/icons/*`, delete any generated `android/` and `ios/` folders, and replace `public/favicon.png` with the generated 128×128 PNG. Make `src-tauri/build.rs` watch `icons/` (tauri-build only reruns on `tauri.conf.json` changes, so new icons were otherwise not embedded). Verify `npm run tauri dev` shows the new icon in the window title bar and taskbar, and `git status` shows only the expected icon files changed.
- [ ] 5.3 Build with `npm run tauri build` and install the result. Verify the installed shortcut or launcher shows the new icon.
- [ ] 5.4 Move the icon generator into `scripts/gen-icons.mjs` (writes `src-tauri/icons/app-icon.svg` and the single-colour imprint `src/assets/logo-shadow.svg` (bars and dots only); the colour SVG must come out unchanged). Show the imprint above the empty-screen message as a theme-tinted CSS mask, hidden from assistive technology. Verify that regenerating leaves `app-icon.svg` unchanged in git, that `npm run check` and `npm run build` pass, and manually against the "Empty-screen watermark" scenarios in light and dark themes.

## 6. Acceptance

- [ ] 6.1 Add rows to `docs/acceptance.md` for every scenario in this change's `file-tabs`, `app-branding` and modified `wav-import` specs. Update rows I3/I4 to the new drag-and-drop behaviour. Run the manual pass, recording ✅/❌ for each row, and verify all of the new rows pass.

## Workflow follow-up

- Archive `add-wav-analyser-app` before archiving this change, so the `wav-import` MODIFIED delta has a main spec to apply to.
