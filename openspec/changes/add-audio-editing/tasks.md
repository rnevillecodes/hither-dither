# Tasks

## 1. Go to start button

- [ ] 1.1 Add a "Go to start" button before Play in `src/components/Transport.svelte` (bar plus left-pointing triangle icon, `aria-label="Go to start"`, `title="Go to start (Home)"`). It calls `seek(0)` then `reveal(0)`, and is enabled whenever `app.meta` is set, regardless of `playbackAvailable()`. Make the Home key in `App.svelte` also call `reveal(0)`. Verify `npm run check` passes and manually: stopped at 12 s → playhead at 0 and time shows 0:00:00.000; while playing → continues from 0; zoomed in at 5:00 → view scrolls to the start at the same zoom; works with no audio device.
- [ ] 1.2 Add a `transport-controls` section to `docs/acceptance.md` with one row per scenario in `specs/transport-controls/spec.md`. Verify each scenario has a row.

## 2. Edit model (backend, pure)

- [ ] 2.1 Create `src-tauri/src/edit/mod.rs` with `Segment`, `Ramp` and `EditList` as in design.md. Add `EditList::identity(frames)`, `frames()` and the operations `crop`, `delete`, `silence`, `fade_in` and `fade_out` over a `[start, end)` frame range (splitting segments at the bounds; silence collapses gains to a single zero ramp). Add unit tests: crop/delete lengths and source mapping, delete at the start or end, splitting inside a faded segment, fade-twice gain squaring, silence over a fade, and rejection of empty ranges or ranges that would leave zero frames. Verify `npm run test:rust` passes.

## 3. Read edited audio everywhere

- [ ] 3.1 Add `Audio { source, edits }` (in `edit/audio.rs`) with `frames`, `channels`, `info`, `read` and `read_with_full_scale`. It maps ranges onto segments by binary search, applies ramp products per frame, clears full-scale flags on samples with non-unity gain, and has a fast path for the identity list. Add tests comparing `Audio::read` against a hand-computed reference for crop/delete/silence/fade on a synthetic source, including reads that span segment boundaries and are clamped at the end. Verify `npm run test:rust` passes.
- [ ] 3.2 Switch `peaks.rs`, `stats/` and `playback/` from `SampleSource` to `Audio`, and make `LoadedFile` hold `audio: Audio` (identity edits on load). Update existing tests to wrap their sources. Verify `npm run lint:rust` and `npm run test:rust` pass, and that the load-time and scroll benchmarks in `docs/performance.md` stay within 5% of their recorded values for an unedited file.

## 4. History, re-analysis and edit commands

- [ ] 4.1 Add per-file history to `AppState` (`history`, `pos`, `saved_pos`, `revision`, plus an `editing` flag) with `apply_edit`, `undo` and `redo`. Each re-runs `stats::analyse` with a `PyramidBuilder` over the new `Audio`, reports progress, can be cancelled via the existing cancel map, swaps in the new `LoadedFile`, and rejects a concurrent edit on the same file. Add tests in `src-tauri/tests/state.rs`: edit → undo restores identical stats and frames; a new edit after undo clears redo; dirty is false after undoing to the saved position; edits on one id don't affect another; close during re-analysis cancels it. Verify `npm run test:rust` passes.
- [ ] 4.2 Add and register the `apply_edit`, `undo_edit` and `redo_edit` commands in `commands.rs`/`lib.rs`. They stop playback if it is using the file and return `{ metadata, editState }`. Document them in `docs/ipc.md`. Verify `npm run lint:rust` passes and every documented command name matches `generate_handler!`.

## 5. WAV writer and saving (backend)

- [ ] 5.1 Record `fmt_range` in `WavInfo` (`wav/parser.rs`) and add a parser test that it covers the `fmt ` chunk for both plain and extensible headers. Verify `npm run test:rust` passes.
- [ ] 5.2 Create `src-tauri/src/wav/writer.rs`: RIFF header, the verbatim `fmt ` chunk and a `data` chunk; raw copy for gain-free segments; native-domain gain for the others (integers rounded and clamped, valid-bit grid, `f64` without going through `f32`); `0x80` silence for 8-bit; and a size check against the 4 GiB limit before creating anything. Add tests for every sample format: untouched samples are bit-exact after delete; a faded/silenced file re-parses with the same `WavInfo` apart from frames; an `f64` fade matches the reference to 1e-12; LIST/bext in the source are absent in the output; and the oversize check fails without writing. Verify `npm run test:rust` passes.
- [ ] 5.3 Implement `AppState::save(id, path, overwrite)` with the temp-file + fsync + rename flow and the mapped-original swap from design.md, then add the `save_file` command. After a save the new `LoadedFile` reuses the pyramid/analysis, the history resets and dirty clears. Add tests (using `tempfile`): Save As leaves the source unchanged and the new file matches; Save over the original replaces it and the file stays readable; Save refuses when the original changed on disk; a failed write (target dir removed) leaves the original intact and the history unchanged. Document `save_file` in `docs/ipc.md`. Verify `npm run lint:rust` and `npm run test:rust` pass.

## 6. Editing UI

- [ ] 6.1 Add `applyEdit`, `undoEdit`, `redoEdit` and `saveFile` wrappers in `src/lib/api.ts`, and add `dirty`, `canUndo`, `canRedo`, `revision`, `editing` and `overwriteConfirmed` to `Tab` in `tabs.svelte.ts`. Verify `npm run check` passes.
- [ ] 6.2 Create `src/lib/editActions.ts` with `applyEdit(op)`, `undo()` and `redo()`. Each stops playback, sets `editing` and shows the delayed progress overlay, updates `meta`, `wholeStats`, `clipRuns`, the selection (cleared for crop/delete and for undo/redo that changes the length, kept otherwise), the playhead (0 for crop, the join for delete) and the viewport (fit for crop, clamped otherwise), and bumps `revision`. Re-key `<Waveform>` on `id:revision` and invalidate `overviewCache` for that tab. Put the pure selection/playhead rules in a testable helper with vitest cases. Verify `npm test` and `npm run check` pass.
- [ ] 6.3 Add the Edit submenu (Undo, Redo, separator, Crop to Selection, Delete, Silence, Fade In, Fade Out) and File › Save / Save As to `menu.ts`, with enabled states driven from an `$effect` in `App.svelte` (no tab / loading / editing / no selection / whole-file selection / nothing to undo/redo). Add the keyboard shortcuts from design.md to `App.svelte` with a double-fire guard. Verify manually against every scenario in `specs/audio-editing/spec.md` using `cd-16bit-stereo-10s.wav`, `two-clip-runs.wav` and `large-1gb.wav` (progress overlay during re-analysis; other tabs usable).
- [ ] 6.4 Add an `audio-editing` section to `docs/acceptance.md` with one row per scenario in `specs/audio-editing/spec.md`, citing the Rust and vitest tests as Auto evidence where they apply. Verify each scenario has a row.

## 7. Saving UI and unsaved-changes protection

- [ ] 7.1 Add `dialog:allow-save`, `dialog:allow-ask`, `dialog:allow-message` and `core:window:allow-destroy` to `src-tauri/capabilities/default.json`. Implement `save()` (first-time overwrite `ask`, disabled when not dirty) and `saveAs()` (native save dialog with a WAV filter, rejects a path open in another tab with a notice, then updates the tab's path/name so session restore follows it) in `editActions.ts`, and show backend errors as notices. Verify manually: the Save and Save As scenarios in `specs/wav-export/spec.md`, including a read-only folder and a file modified externally after opening.
- [ ] 7.2 Show the unsaved marker in `TabBar.svelte` (a dot in place of ×, × on hover, tooltip "Unsaved changes"). Verify manually: an edit marks the tab; undoing back to the original clears it; saving clears it.
- [ ] 7.3 Make `closeTab`/`closeActiveTab` go through `confirmDiscard` (Save / Don't Save / Cancel via `message`; Save skips the overwrite prompt and keeps the tab open if saving fails). Handle `onCloseRequested` to prompt for each dirty tab in order before `destroy()`, and replace the predefined Quit menu item with a custom one using the same path. Verify manually: the closing and quitting scenarios in `specs/wav-export/spec.md` (close button, middle-click, Ctrl/Cmd+W, window ×, File › Quit), and that quitting with no edits doesn't prompt.
- [ ] 7.4 Add a `wav-export` section to `docs/acceptance.md` with one row per scenario in `specs/wav-export/spec.md`, citing the writer and save tests where they apply. Verify each scenario has a row.

## 8. Integration checks

- [ ] 8.1 Run `npm run check`, `npm test`, `npm run lint:rust` and `npm run test:rust`, and verify all pass.
- [ ] 8.2 End-to-end in `npm run tauri dev`: open `large-1gb.wav`, crop it, fade both ends, undo/redo several times, Save As, then reopen the saved file in a fresh tab and verify its duration, waveform and stats match what was shown before saving. Also open the saved file in another audio tool and verify it plays.
- [ ] 8.3 Build with `npm run tauri build` and verify in the installed app that Save over the original works on Windows while the file is open (the mapped-file replace path).
