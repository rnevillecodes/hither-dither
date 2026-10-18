# Tasks

## 1. Preset library (Rust)

- [x] 1.1 Add `src-tauri/src/presets.rs` with `PresetLibrary`, built on the file format in design.md (`effect-presets.json`, format and version, `nextId`, presets of `StoredEffect`).
  - `open(dir)` returns the library plus an optional notice when the file can't be read.
  - Also `list()` (sorted ignoring case), `get(id)`, `save(name, effects, replace)`, `rename(id, name, replace)` and `delete(id)`. Writes go through an atomic JSON write; names are checked with `layout::valid_name`; a name taken, ignoring case, gives `NameTaken` unless `replace` is set.

  Unit tests cover:
  - save, then reopen from disk;
  - sorting;
  - name taken and replacing;
  - rename keeps the id;
  - delete;
  - an unreadable file opens empty, with a notice, and is left unchanged until the next save;
  - a failed write leaves the in-memory list unchanged.

  Verify `npm run test:rust` passes.
- [x] 1.2 Hold the library as a Tauri state opened from the app data folder in `src-tauri/src/lib.rs`, and include its notice in `take_startup_notices`. Verify `npm run lint:rust` passes.

## 2. Saving and inserting (Rust)

- [x] 2.1 In `src-tauri/src/state.rs`, pull `store_effects` out of `capture` and `load_stored` out of `restore`, leaving session behaviour unchanged. Verify the existing session tests in `src-tauri/tests/state.rs` pass unchanged.
- [x] 2.2 Add `AppState::save_group_preset` and `AppState::insert_preset` as designed. Add tests to `src-tauri/tests/state.rs` with HD Test Gain and HD Test Stereo for:
  - "Save a group" (settings and own bypass kept; the tab's unsaved state and history unchanged);
  - "Insert into another tab" (a group named after the preset, settings, one undo step);
  - "Not linked";
  - "Stereo-only effect on a 5.1 file";
  - nothing insertable (an error, chain unchanged).

  Verify `npm run test:rust` passes.
- [x] 2.3 Add the `list_presets`, `save_group_preset`, `rename_preset`, `delete_preset` and `insert_preset` commands. Make `chain_change` generic so `insert_preset` follows the same playback and editor path. Register the commands and document them in `docs/ipc.md`. Verify `npm run lint:rust` and `npm run test:rust` pass.

## 3. Frontend logic

- [x] 3.1 Add to `src/lib/api.ts`:
  - `PresetInfo`, `SavePresetOutcome` and `InsertOutcome`;
  - the command wrappers;
  - a `validPresetName` helper mirroring the backend's name rules, with vitest cases.

  Add `src/lib/presets.svelte.ts` (the list, `refreshPresets`). In `src/lib/editActions.ts`, add `saveGroupPreset`, `insertPreset` (showing warnings with `showEffectWarnings`, and errors as notices), `renamePreset`, `deletePreset` and `canInsertPreset`. Verify `npm run check` and `npm test` pass.

## 4. Menu, dialogs and panel

- [x] 4.1 In `src/lib/menu.ts`, add Effects › Save Group as Preset… (enabled with a highlighted group), an Effects › Insert Preset submenu kept up to date by `setPresetMenu(list, enabled)` from an `$effect` in `src/App.svelte` (a disabled "No presets" item when empty), and Manage Presets…. Verify `npm run check` passes, and check by hand ("No presets", "Available after restarting") that the submenu follows saves, renames and deletes.
- [x] 4.2 Add `src/components/PresetNameDialog.svelte` (name prompt, validation, the "Replace" / "Cancel" message dialog when a name is taken) and a Save as Preset button on group headers in `src/components/EffectsPanel.svelte`. Verify `npm run check` passes, and check "Save a group", "Name taken" and "Effect can't save its settings" by hand (the last with a plugin known to fail, or skip with a note if none is at hand).
- [x] 4.3 Add `src/components/PresetManager.svelte` (list with effect counts, Rename through the name dialog, Delete with confirmation), opened from Manage Presets…. Verify `npm run check` passes, and check "Delete" and a rename by hand.

## 5. Docs

- [x] 5.1 Add a presets paragraph to the README's VST3 effects section, and an `effect-presets` section to `docs/acceptance.md` covering every scenario, with the automated test named where there is one. Verify the docs read correctly, and that `npm run check`, `npm test`, `npm run test:rust`, `npm run lint:rust` and `openspec validate add-group-presets --strict` all pass.
