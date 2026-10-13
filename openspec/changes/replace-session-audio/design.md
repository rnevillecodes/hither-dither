# Design

## Context

- **Capturing a chain:** `AppState::capture(id, session_path)` already turns a tab's current edits and chain into a `SessionFile` in memory. It gets each plugin's state on the UI thread, plus parameter changes not yet processed. `save_session` writes that to disk; nothing requires it to be written.
- **Restoring a chain:** `AppState::restore_session(id, path, plugins)`:
  1. reads the session file;
  2. checks the format and length against the loaded file, rejecting with an error string on a mismatch;
  3. rebuilds the edits;
  4. loads each effect with `load_slot`, collecting `EffectWarning`s for effects that can't be found, are disabled, don't fit the layout, or fail to load;
  5. refuses unless the history is fresh;
  6. installs `History::restored`.
- **Opening:** `AppState::open(id, path)` loads first and installs only on success, replacing that id's file and history. A failed load leaves the previous file under the id untouched.
- **Saved marker:** `History::restored` is clean. `touch()` bumps the settings counter, which makes the tab dirty.
- **Editors:** `commands::close_removed_editors` closes editor windows for slots that are no longer in a file's chain.
- **Frontend open flow:** `openPaths` reads every session (`read_session`, in parallel), plans tabs with `planOpens`, and loads them in a queue. `loadIntoTab` calls `restore_session` before the tab is ready; a failure closes the backend file and removes the tab with a notice. `read_session` rejects when the WAV is missing.
- **Edit state:** `EditState` has `can_undo`, `can_redo`, `dirty` and `revision`, but nothing says whether the current audio is edited. A restored session with edits has nothing to undo.

## Goals / Non-Goals

**Goals:**
- One chain-transfer path, the session capture plus restore code, for saving, opening, replacing and relinking, so effects come across with exactly the settings a saved session would give them.
- Replacing never leaves a tab half-replaced: either the new WAV is loaded with the chain rebuilt, or the old WAV, edits and effects are untouched.

**Non-Goals:**
- Keeping the old WAV's plugin instances. They are recreated from their captured state on the new file, because their bus arrangement, sample rate and latency are set up for the old audio.

## Decisions

### Restore takes a session value and a mode
`restore_session`'s body becomes `restore(id, session: &SessionFile, mode, plugins)`. `mode` is one of:
- `Exact`: today's checks. A format mismatch returns `RestoreOutcome { applied: false, … }` instead of an error, so the frontend can offer to choose another WAV. Other failures stay errors.
- `Relink`: for a WAV the user picked. If `format_matches`, the edits are restored. Otherwise the edits are replaced by the identity list and `editsDropped: true` is reported. Either way the history is touched afterwards, so the tab is unsaved.
- `EffectsOnly`: used by Replace Audio. Identity edits and no format check, and the history is touched afterwards.

The `restore_session` command takes `relink: bool` (choosing `Exact` or `Relink`). It reads the file and calls `restore`. `RestoreOutcome` gains `applied` and `editsDropped`.

*Alternative:* separate functions per case. Rejected, because the effect-loading loop, warnings, fresh-history check and install are identical.

### Replace Audio is one backend command
`AppState::replace_audio(id, path, plugins, progress) -> Result<ReplaceOutcome { metadata, edit_state, warnings }, String>`:
1. Refuses if `same_file(path, current path)`, or the file is exporting.
2. `capture(id, path)` takes the chain in memory. The relative path is irrelevant here.
3. `open(id, path, progress)`. On failure it returns the error. Nothing was installed, so the tab still has its old WAV, edits and effects.
4. `restore(id, &captured, EffectsOnly, plugins)` rebuilds the chain on the new file. The old history and its slots were dropped by `open`, and their instances are released on the UI thread as usual.

The `replace_audio` command stops playback first and calls `close_removed_editors` afterwards, so every old editor window closes.

*Alternative:* the frontend opens the new WAV in the tab and calls `restore_session` with an in-memory session. Rejected: the captured chain would have to cross IPC as base64 JSON, and the old file would already be gone if the capture failed.

### Knowing whether a tab is edited
`EditState` gains `edited: bool` (`!current().edits.is_identity()`). Replace Audio confirms when `edited || can_undo`. This covers a restored session with edits, which has nothing to undo.

### Missing WAV is a result, not an error
`read_session` returns `{ audioPath: string | null, audioName: string }`. `null` means neither path exists. Invalid and newer-version sessions still reject.

### Frontend flows
- **Opening sessions:**
  - `openPaths` reads session paths one at a time instead of in parallel, so at most one "Choose Another WAV…" dialog is open at once. For `audioPath: null` it shows the missing-WAV dialog. Choosing a file makes the request `{ kind: "session", sessionPath, audioPath: chosen, relink: true }`; Cancel drops the request.
  - `planOpens` passes `relink` through, and its "already open" handling covers the chosen WAV.
  - `Tab` gains `relink: boolean`, which `loadIntoTab` passes to `restore_session`.
  - When `restore_session` resolves `applied: false`, `loadIntoTab` closes the backend file and throws a `SessionMismatch` carrying the session path and WAV name. The load queue catches it, removes the tab without the usual error notice, shows the mismatch dialog, and on Choose Another WAV… calls `openPaths`'s relink path for that session.
  - `editsDropped` adds a notice that the edits were left out because the audio is different.
- **Replace Audio** (`editActions.ts`):
  1. Check the tab is editable.
  2. Open dialog with the WAV filter.
  3. Refuse the tab's own path, or a path open in another tab.
  4. Confirm when `edited || canUndo`.
  5. Set `status: "loading"` with progress, and call `replaceAudio`.
  6. On success, reset the tab as a fresh load: path, name, metadata, stats, clip runs, clip and chain fetched again; selection, viewport, playhead, overview cache and peaks version reset; edit state from the outcome. Then show the warnings notice. The session path is kept.
  7. On failure, restore `status: "ready"` and show "Could not replace the audio".

  The fresh-load reset is factored out of `loadIntoTab` into a `showLoaded(tab, meta)` helper used by both.
- **Menu:** File › Replace Audio… after Export…, enabled with `canExport()`. No shortcut.

## Risks / Trade-offs

- [A plugin whose state depends on the sample rate (for example a convolution reverb with an impulse response at 44.1 kHz) restored onto a 48 kHz WAV] → It behaves as if the session were opened at the new rate; VST3 plugins are expected to handle `setupProcessing` with a new rate.
- [Replacing loses the edits with no undo] → Confirmation first, and the saved session file is untouched until the user saves.
- [Reading sessions one at a time slows opening many sessions at once] → Reading a session is a small JSON parse; only the dialogs make it wait.
- [A relinked WAV with the right format and length but different audio] → Same as "WAV touched but same format" today: the user chose it, and edits are positions, not content.

## Migration Plan

None: no file format changes. `read_session` and `restore_session` change their results, but the frontend is their only caller.
