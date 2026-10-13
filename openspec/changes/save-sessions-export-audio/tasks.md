# Tasks

## 1. Saved state in the backend

- [x] 1.1 Give `Snapshot` an `id`, and give `History` a `settings` counter and a `saved: (snapshot id, settings)` pair set by `History::new` and `History::restored`. Compute `dirty` against `saved` as design.md describes. Add `AppState::touch(id)` and call it from `change_bypass`, `set_fx_param`, and the `watch_slot` listener for `HandlerEvent::Edited` and `kParamValuesChanged` restarts. Rewrite the `tests/state.rs` cases whose expectations change (`everything_bypassed_is_not_unsaved`, `an_effect_alone_marks_the_file_unsaved`, the session restore dirty checks) and add cases:
  - "Edit marks the tab";
  - "Undo back to the original";
  - "Plugin setting changed" (`set_fx_param` and an editor `performEdit` both mark dirty, and undo doesn't clear it);
  - a bypass toggle marks dirty;
  - a freshly opened WAV and a freshly restored session are clean ("Nothing to undo after opening", "Change after opening").

  Verify `npm run test:rust` passes.
- [x] 1.2 Make `save_session` take the `(snapshot id, settings)` pair with the capture, mark it saved once written, and return `{ written, editState }`. Add `tests/state.rs` cases:
  - "Saving the session clears it";
  - "Undo after saving the session" (undo makes it dirty, redo back makes it clean);
  - a change between capture and write leaves the tab dirty (simulated by touching from the capture's UI-thread call in a test hook, or by checking that the saved pair is the captured one);
  - `replace: false` on an existing file leaves the edit state dirty.

  Update `docs/ipc.md` for the new result. Verify `npm run lint:rust` and `npm run test:rust` pass.

## 2. Export instead of Save

- [x] 2.1 Replace `AppState::save`/`save_inner` with `AppState::export` and the `save_file` command with `export_file`, as design.md describes: refuse the source path, render with or without the chain, always return the chain to real-time, temp file plus rename, and leave the file, history and chain untouched. Remove the code that only Save used. Port the save tests in `tests/state.rs` to `export`, keeping the output checks (bit-exact untouched audio, hard join, latency removed, bypassed chain bit-exact, clamped count, float overs, too large), and replace the post-save expectations with the new ones:
  - "Keep working after export" (same chain, slots, settings, history and edit state after export);
  - "Export over the source" (refused, nothing written);
  - "Read-only target" (fails, temp file removed, tab unchanged);
  - "Replacing an earlier export";
  - an export failure with effects leaves them playable in real time.

  Verify `npm run lint:rust` and `npm run test:rust` pass.
- [x] 2.2 Replace `save_file` with `export_file` in `docs/ipc.md`, including its refusals, that the tab is unchanged, and that playback stops. Verify every command in `generate_handler!` appears there and `save_file` no longer does.

## 3. Frontend

- [x] 3.1 In `api.ts`, replace `saveFile` with `exportFile` and update `saveSession` to its `{ written, editState }` result. Remove `overwriteConfirmed` from `Tab`. Add `defaultExportPath` to `sessionFiles.ts`, with vitest tests: `take.wav` → `take-export.wav` in the same folder, `Take.WAVE` keeps its stem, and a path without an extension gets `-export.wav`. Verify `npm run check` and `npm test` pass.
- [x] 3.2 In `editActions.ts`:
  - Save and Save As save the session; drop the old Save Session functions and `canSaveSession`.
  - Add `exportAudio` with its dialog, its refusals of the tab's own path and other open tabs' paths, progress, and the clamped notice. It makes no tab updates afterwards.
  - Make `canSave` follow "When Save is available".
  - Make `confirmClose` save the session, with a cancelled dialog keeping the tab open and aborting a quit.

  Update `menu.ts` (Save, Save As…, Export… with Ctrl/Cmd+E; remove the Save Session items) and the `App.svelte` key handler. Mark the tab dirty on `fx-param-changed`, `fx-params-reset` and successful `setFxParam` calls for that file. Verify `npm run check` and `npm test` pass, and check by hand: "First save asks", "Later saves overwrite", "Save As switches session", "Nothing new to save", "Export a trimmed copy", "Export over the source", "Name of another open file", "Save on close", "Save on close without a session", "Cancel quitting", and "Plugin setting changed" with a native editor.
- [x] 3.3 Make the launch restore remember `sessionPath ?? path` for each tab and for the active tab, and skip writing the remembered list while the launch `openPaths` is still running. Put the mapping in a pure function in `session.ts` with vitest tests (session tabs stored by session path, plain tabs by WAV path, and the active entry the same way). Verify `npm run check` and `npm test` pass, and check by hand: "Session tab after a restart" and "Unsaved changes are not kept by the restart".

## 4. Docs and end-to-end check

- [x] 4.1 Update `README.md`:
  - Save now saves the session and Export writes the audio;
  - the WAV is never overwritten;
  - plugin setting changes count as unsaved, including plugins that report changes when their editor opens.

  Update `docs/acceptance.md`:
  - rewrite the wav-export rows for Export;
  - update the session-files rows that used Save Session / Ctrl/Cmd+Alt+S;
  - add rows for "Plugin setting changed", "Keep working after export", "Session tab after a restart" and "Shaped take in a session".

  Verify by running those rows by hand in `npm run tauri dev`.

## Workflow follow-up

- Archive `add-vst3-effect-chain`, then `add-session-files`, then this change, rebasing this change's `wav-export` deltas onto the VST3 change's text first (see design.md).
- After archiving, update the `wav-export` main spec's Purpose, which still says Save replaces the original.
