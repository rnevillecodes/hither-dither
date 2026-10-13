# Proposal

## Why

Save still overwrites the WAV with the rendered edits and effects, then clears the chain and the undo history. Now that a session file can hold a tab's whole state, that is the wrong default: an everyday Ctrl/Cmd+S destroys the original recording and throws away the effects and their settings. Writing audio should be a deliberate step, separate from saving work in progress.

## What Changes

- **BREAKING: Save saves the session.** File › Save (Ctrl/Cmd+S) writes the active tab's session file, asking where the first time (as Save Session did). Save As… (Ctrl/Cmd+Shift+S) saves it to a new session file. The separate Save Session / Save Session As items and Ctrl/Cmd+Alt+S go away. Save never writes a WAV.
- **Export writes the audio.** A new File › Export… (Ctrl/Cmd+E) renders the tab's edits and effects to a new WAV, with the same output format and progress as Save today. The tab is left exactly as it was: same WAV, edits, chain, plugin settings, undo history and session. Exporting over the tab's own WAV, or over a WAV open in another tab, is refused.
- **BREAKING: Unsaved means "not in the session".** A tab is unsaved when anything changed since its session was last saved or opened, or since the WAV was opened for a tab without a session: edits, undo/redo, effect changes, bypass switches, and plugin setting changes (from an editor or the generic controls). Saving the session clears the marker; undoing back to the saved state clears it too, unless a setting changed meanwhile.
- **Close and quit prompts save the session.** "Save" in the unsaved-changes prompt saves the session (asking where if the tab has none), never the WAV.
- **History survives saving.** Saving a session keeps the undo history; it resets only when a tab is opened (from a WAV or a session).
- **Launch restore reopens sessions.** The last-session restore remembers a tab's session file when it has one, and reopens that instead of the bare WAV.
- **Out of scope:**
  - exporting to formats other than WAV, or with a different bit depth or sample rate;
  - exporting several tabs at once;
  - writing the rendered audio back over the original WAV (users can export and replace it outside the app);
  - autosave.

Assumptions recorded from the request and the user's answers:
- "Exporting the session" means rendering the current edits and effects to a WAV file; the session file itself is unchanged by an export.
- Plugin setting changes count as unsaved changes.
- The default export name is the WAV's name with `-export` (`take.wav` → `take-export.wav`), in the WAV's folder.

## Capabilities

### New Capabilities
<!-- None. -->

### Modified Capabilities
- `wav-export`: Save and Save As are replaced by Export, which leaves the tab unchanged and never overwrites the source; the unsaved marker, safe writing, and the close and quit prompts now refer to the session.
- `audio-editing`: the undo history lasts from opening the tab, not from the last save, and the original file is never modified by the app.
- `clip-fades`: clip settings are written by Export and stay in place afterwards.
- `live-editing`: audio is rendered only by Export.
- `session-files`: Save Session becomes File › Save / Save As, and the last-session restore reopens session files. This capability is still in the unarchived `add-session-files` change, so that change must be archived before this one (see design.md).

## Impact

- **Rust (`src-tauri`):**
  - `state.rs`: `save` and `save_inner` become `export`, which renders to a new file and leaves the tab, history and chain alone; the reopen-and-reset path goes. The history gets a saved marker (snapshot id plus a settings counter) and `dirty` is computed against it. Bypass and plugin setting changes bump the counter; `save_session` marks the history saved.
  - `commands.rs`/`lib.rs`: `save_file` is replaced by `export_file`; `save_session` returns the new edit state.
  - `tests/state.rs`: save tests are ported to export, and post-save expectations (empty chain, reset history) are rewritten for the new model.
- **Frontend (`src`):** `editActions.ts` (save → session, new export, close/quit prompts), `menu.ts` (Save/Save As/Export items and shortcuts), `App.svelte` (shortcuts, launch restore of session files), `session.ts` (remembers session paths), `tabs.svelte.ts` (drop `overwriteConfirmed`), frontend dirty marking on `fx-param-changed` / `fx-params-reset`.
- **Docs:** `docs/ipc.md` (`export_file`, `save_session` result), `README.md` (Save vs Export), `docs/acceptance.md` (rows for wav-export and session-files).
- **Specs not yet published:** the unarchived `add-vst3-effect-chain` change says a save clears the chain and that an active chain marks a tab unsaved; this change supersedes both. See design.md for the archive order.
