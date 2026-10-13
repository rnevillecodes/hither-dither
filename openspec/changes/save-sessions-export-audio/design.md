# Design

## Context

- **Save today** (`AppState::save` → `save_inner` in `state.rs`): renders the edited audio (through `render_file` when the chain is active, else `write_file` after finishing the analysis), renames the temp file over the target, reopens the target as the tab's new source with a fresh `History`, and empties the chain. A failed rename over the original reopens the old mapping. Save refuses to overwrite a WAV that `changed_on_disk()`.
- **Dirtiness today:** `History::state()` computes `dirty = !edits.is_identity() || chain.is_active()`, "differs from the file on disk". Bypass and plugin parameters are outside the history.
- **Sessions** (`add-session-files`, committed): `save_session(fileId, path, replace) -> bool` captures and writes a session; `restore_session` replaces the history with a single restored snapshot. The frontend has separate Save Session / Save Session As items (Ctrl/Cmd+Alt+S) and `Tab.sessionPath`.
- **Parameter changes** reach the backend through `set_fx_param` (generic controls), `HandlerEvent::Edited` from a native editor (`watch_slot`, then `ParamSync::edited`, which emits the throttled `fx-param-changed` event), and `restartComponent(kParamValuesChanged)` (`ParamSync::reset`, which emits `fx-params-reset`).
- **Launch restore** (`App.svelte`, `session.ts`): remembers `tabs.list.map(t => t.path)` in `localStorage` and calls `openPaths` at startup. Its comment relies on `openPaths` pushing the tabs synchronously, so the effect that writes the remembered list never sees an empty list first. Since `add-session-files`, `openPaths` is asynchronous when the list contains session paths.

## Goals / Non-Goals

**Goals:**
- One saved-state marker in the backend that every change kind (history steps, bypass, parameters) is measured against, so the frontend never has to reason about what counts as unsaved.
- Export reuses the existing render paths (`render_file`, `write_file`, temp file plus rename) without any of the reopen or reset logic.
- No code path left that writes over a WAV the app has open.

**Non-Goals:**
- Comparing plugin states byte for byte to decide dirtiness: any reported parameter change counts, even if a later change sets it back.
- Keeping the analysis of an exported file: nothing shows the exported audio, so it isn't analysed.

## Decisions

### Saved marker: snapshot id plus a settings counter
- `Snapshot` gains `id: u64`, unique per history (a counter in `History`).
- `History` gains `settings: u64`, bumped by every change outside the history: per-effect bypass, Bypass All, and any parameter change. It also gains `saved: (u64, u64)`, the `(snapshot id, settings)` pair at the last session save, or when the tab was opened.
- `dirty = (current().id, settings) != saved`. Undoing back to the saved snapshot is clean unless a setting changed meanwhile. That is the spec's rule, because parameter values are not part of the history and can't be undone.
- `History::new` and `History::restored` set `saved` to their first state, so a freshly opened WAV and a freshly restored session are both clean.
- `save_session` captures the session while holding the pair it captured (taken under the history lock with the edits and chain). After the file is written, it sets `saved` to that pair, so a change that lands between capture and write still leaves the tab dirty. It returns `{ written, editState }` instead of a bare bool.

*Alternative:* hashing each plugin's `getState` on every check. That is expensive for large plugins and calls into plugins on every edit-state query.

### Where settings changes are counted
`AppState::touch(id)` bumps `settings` for file `id`. It is called from:
- `change_bypass` (both bypass commands);
- `set_fx_param`;
- the `watch_slot` listener, for `HandlerEvent::Edited` and for `kParamValuesChanged` restarts. The listener already knows the file id, and gets a weak handle to the history map for this.

The frontend mirrors this without a new event. `fx-param-changed`, `fx-params-reset` and a successful `setFxParam` set `tab.dirty = true` for that file's tab, and bypass outcomes already carry `edit_state`. `save_session`'s returned `editState` is authoritative and resets the frontend's view.

*Alternative:* a new `edit-state-changed` event from the backend. It is cleaner in principle, but the existing events already fire exactly when a parameter changes.

### Export replaces Save in the backend
`AppState::export(id, target, progress) -> Result<ExportOutcome { clamped_samples }, String>`:
1. It refuses when `same_file(target, source path)`. The frontend also refuses targets open in another tab, as Save As did.
2. It holds the per-file `loading` token for the duration, as Save did, so edits are refused meanwhile (`check_not_saving`). The message becomes "The file is being exported".
3. With an active chain it calls `render_file` and then always `fx::render::realtime`, on success as well as failure, so the chain keeps playing. Otherwise it calls `write_file(&*file.audio, …)` directly, with no analysis first.
4. It writes to `temp_path(target)` and calls `rename_with_retry`. On failure it removes the temp file. The `LoadedFile`, the history and the chain are never touched.

`save` and `save_inner` are deleted, together with the reopen-after-rename path, `changed_on_disk` refusals on save, and the `seeded` install of the written file's analysis (`seeded` is still used by `install`). The `save_file` command becomes `export_file(fileId, path, onProgress)`. It still stops playback first, because offline rendering takes the slots' processing.

*Alternative:* keeping `save` for a hidden "Export and replace". Out of scope, and it is the riskiest code in `state.rs`.

### Frontend actions
- `editActions.ts`:
  - `save()` becomes the old `saveSession()`: write `sessionPath`, or ask.
  - `saveAs()` becomes the old `saveSessionAs()`.
  - The old `saveSession`, `saveSessionAs` and `canSaveSession` are removed.
  - `canSave()` = editable && (dirty || !sessionPath); `canSaveAs()` = editable.
  - `exportAudio()` is the old `saveTab` render flow: progress, `tab.editing`, the clamped-samples notice. It has no metadata, path, chain or history updates afterwards.
  - The export dialog defaults to `<stem>-export.wav` in the WAV's folder (pure helper `defaultExportPath` in `sessionFiles.ts`, with tests). It refuses the tab's own path and paths open in other tabs before calling the backend.
  - `confirmClose(tab)`: "Save" calls the session save for that tab, which may ask for a path. Cancelling that dialog resolves false, so the tab stays open and a quit is aborted.
  - `overwriteConfirmed` is removed from `Tab`.
- `menu.ts`: File › Save (Ctrl/Cmd+S), Save As… (Ctrl/Cmd+Shift+S), Export… (Ctrl/Cmd+E). The `save-session` and `save-session-as` toggles are replaced by `export`.
- `App.svelte` key handler: drop the Alt case; add Ctrl/Cmd+E → export.

### Launch restore remembers session files
- `writeSession` stores `t.sessionPath ?? t.path` per tab, and the same for `active`. `readSession` is unchanged; `openPaths` already routes session paths.
- Because restoring session paths is asynchronous, `App.svelte` sets a `restoringLaunch` flag before calling `openPaths(saved.paths)` and clears it when that resolves. The `$effect` that writes the remembered list does nothing while it is set, so an empty or partial list never overwrites the remembered one.
- The active-tab match after restore compares against `t.sessionPath ?? t.path`.

### Archive order and unpublished specs
`session-files` exists only in `add-session-files` (unarchived), so this change's `MODIFIED`/`REMOVED` operations on it can only be archived after `add-session-files` is. The unarchived `add-vst3-effect-chain` change also modifies `wav-export` ("Output format", "After saving") and says a save clears the chain and an active chain marks a tab unsaved. Archive in this order: `add-vst3-effect-chain`, `add-session-files`, then this change. Before archiving, rebase this change's `wav-export` blocks onto the text the VST3 change leaves, keeping the processed-samples wording in "Output format".

## Risks / Trade-offs

- [Some plugins send parameter edits when their editor merely opens, or on mouse-over, so opening an editor marks the tab unsaved] → Accepted: a false "unsaved" is safe, and a missed change is not. Note it in the README.
- [Users who relied on Ctrl/Cmd+S to write their WAV now get a session file] → The README and the first Export make the split clear. The close prompt still saves work, now non-destructively.
- [An export of a long file blocks edits on that tab] → As Save did before; other tabs stay usable.
- [Undo past the saved snapshot then redo back: clean again only if no setting changed] → This is the specified behaviour.

## Migration Plan

- User data: nothing to migrate. Session files from `add-session-files` stay valid (format version 1 is unchanged). A tab that was remembered by its WAV path keeps reopening as that WAV.
- `localStorage` launch list: entries may now be session paths. Older builds would try to open a `.hdsession.json` as a WAV and show "Could not open file", which is harmless.
- Rollback: revert the change. No file format changes.
