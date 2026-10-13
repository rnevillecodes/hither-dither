# Tasks

## 1. Backend restore modes

- [ ] 1.1 Factor `restore_session` into `restore(id, &SessionFile, mode, plugins)` with the `Exact`, `Relink` and `EffectsOnly` modes from design.md. `RestoreOutcome` gains `applied` and `editsDropped`, and `EditState` gains `edited`. Make `read_session` resolve `{ audioPath: null, audioName }` for a missing WAV, and give the `restore_session` command a `relink` argument. Update the existing session tests in `tests/state.rs` (a mismatch is now `applied: false` with the file left as opened), and add cases:
  - "Renamed WAV" (relink with matching format keeps the edits and is dirty);
  - "Different recording" (relink with another length drops the edits, keeps the effects with their settings, sets `editsDropped`, is dirty);
  - `edited` is true for a restored session with edits and false after undoing every edit;
  - `read_session` of a session whose WAV is gone gives `audioPath: null` and the WAV's name.

  Update `docs/ipc.md`. Verify `npm run lint:rust` and `npm run test:rust` pass.

## 2. Replace Audio in the backend

- [ ] 2.1 Add `AppState::replace_audio` (refuse the current path and exports, capture in memory, `open` under the same id, `restore` with `EffectsOnly`, touch) and the `replace_audio` command (stop playback, close the old editors). Register it in `lib.rs`. Add `tests/state.rs` cases with HD Test Gain and HD Test Stereo:
  - "Replace with another take" (chain order, settings and bypass kept; new metadata);
  - "Edits are dropped" (identity edits, nothing to undo);
  - "Stereo-only effect, mono take" (left out with a warning);
  - "Marked unsaved";
  - "Not a WAV" (the old file, edits and chain are untouched);
  - choosing the current path is refused;
  - the exported audio of the replaced tab goes through the kept effects (+6 dB).

  Document the command in `docs/ipc.md`. Verify `npm run lint:rust` and `npm run test:rust` pass.

## 3. Frontend

- [ ] 3.1 Update `api.ts` (`readSession` result, `restoreSession` `relink` and outcome fields, `replaceAudio`, `EditState.edited`) and add `Tab.relink`. Read sessions one at a time in `openPaths`. Show the missing-WAV dialog with Choose Another WAV…; on a `SessionMismatch` from `loadIntoTab`, remove the tab quietly and show the mismatch dialog. Choosing a file opens the session in relink mode; `editsDropped` adds its notice. Extend `planOpens` and its vitest tests for `relink` requests and a chosen WAV that is already open. Verify `npm run check` and `npm test` pass, and check by hand: "WAV missing", "WAV re-rendered shorter", "Renamed WAV", "Different recording", "Saving points the session at the new WAV" and "Chosen WAV already open".
- [ ] 3.2 Factor the fresh-load reset out of `loadIntoTab` into `showLoaded`. Add `replaceAudio` to `editActions.ts`, following design.md: dialog, refusals, confirmation when `edited || canUndo`, loading state, reset, warnings, and the failure notice. Add File › Replace Audio… to `menu.ts` with a `replace-audio` toggle wired in `App.svelte`. Put the "needs confirmation" rule in a pure helper with a vitest test. Verify `npm run check` and `npm test` pass, and check by hand: "Replace with another take" (with a native editor open, which closes), "Cancel the dialog", "Confirmation shown", "No confirmation without edits", "Open in another tab", "Not a WAV", "Session follows the new audio" and "Stereo-only effect, mono take".

## 4. Docs and end-to-end check

- [ ] 4.1 Add Replace Audio and Choose Another WAV… to the README's "Saving and exporting" section, and a replace-audio section plus updated session-files rows (SF13, SF14 and new relink rows) to `docs/acceptance.md`. Verify by running those rows by hand in `npm run tauri dev`.

## Workflow follow-up

- Archive the change after review; it syncs `replace-audio` (new) and `session-files`.
