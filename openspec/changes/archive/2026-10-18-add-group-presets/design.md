# Design

## Context

- **Groups** (from `add-effect-groups`): the history's chain is a `Layout` of `Item::Effect(SlotId)` and `Item::Group { id, name, slots }` (`src-tauri/src/fx/layout.rs`). New groups get ids from `AppState.next_group`.
- **Sessions already serialize effects.** `AppState::capture` (`state.rs`) asks each slot's plugin for its state on the UI thread and builds `StoredEffect { class_id, name, vendor, version, bypassed, state, pending_params }`. `AppState::restore` loads stored effects through `load_slot(plugins, class_id, config, Some(saved))`, leaving out the ones that fail and collecting `EffectWarning`s. A preset holds exactly the same data, so presets reuse `StoredEffect` and both code paths.
- **Saved app data:** `PluginCache` keeps `plugin-settings.json` in the app data folder (`app.path().app_data_dir()`), written with `write_json`, which goes through a temporary file and a rename. Startup problems reach the user through `take_startup_notices`.
- **Chain commands** go through `chain_change` (`commands.rs`), which hands the new chain to playback and closes the editors of removed effects.
- **Menu:** `src/lib/menu.ts` builds the native menu once. Items are enabled or disabled through `setMenuEnabled`, which is fed from an `$effect` in `App.svelte`.
- **Dialogs:**
  - the Tauri dialog plugin gives native message dialogs (`dialog:allow-message`), which the close prompt already uses with custom buttons;
  - there is no native text prompt;
  - in-app modal dialogs follow `CurveDialog.svelte` and `PluginManager.svelte`.

## Goals / Non-Goals

**Goals:**
- One serialized form for effects, shared by sessions and presets.
- Inserting a preset behaves like restoring a session's chain: the same leave-out rules, warnings and settings handling.

**Non-Goals:**
- Presets from effects that aren't in a group, or from a whole chain.
- Importing and exporting preset files, or sharing them between machines.
- Presets in the Add Effect browser.
- Remembering a group's bypass switch in its preset: inserted groups always start on.

## Decisions

### A preset library module
A new `src-tauri/src/presets.rs` adds a `PresetLibrary`, held as a Tauri state next to the plugin cache.

The library's file, `effect-presets.json`:

```
{ "format": "hitherdither-presets", "version": 1, "nextId": 3,
  "presets": [ { "id": 1, "name": "Vocal cleanup", "effects": [StoredEffect…] } ] }
```

- **Ids:** preset ids are `u64`s from `nextId`, so a rename keeps the identity that menu items and the dialog refer to.
- **Names:** compared ignoring case, using Unicode lowercase. `list()` returns `PresetInfo { id, name, effectCount }` sorted that way.
- **Operations:** `save(name, effects, replace)`, `rename(id, name, replace)` and `delete(id)`.
  - `save` and `rename` return `NameTaken` instead of saving when the name is in use by another preset and `replace` is false.
  - Each operation writes the file through `write_json`.
- **A file that can't be read:** the library opens empty and queues a startup notice ("Your effect presets could not be read…"). The file is left alone until the next successful save replaces it.

*Alternative:* one file per preset. Rejected, because a single small file is simpler to write atomically and to list. Preset sizes are dominated by plugin state, which sessions already hold in the same way.

### Shared capture and load helpers in `AppState`
- **Saving:** `store_effects(slots) -> Result<Vec<StoredEffect>, String>` is pulled out of `capture` (UI-thread `get_state`, naming the effect that fails). `save_group_preset(id, group, name, replace, library)` reads the group's slots under the history lock, releases the lock, calls `store_effects`, then saves to the library. It never touches the history.
- **Loading:** `load_stored(effects, config, plugins) -> (Vec<Option<Arc<Slot>>>, Vec<EffectWarning>)` is pulled out of `restore`, which keeps using it.
- **Inserting:** `insert_preset(id, preset, plugins)` uses `load_stored` outside the lock, as `add_effect` does.
  - It then goes through `change_chain`. That checks the file wasn't reopened in the meantime, and pushes `Item::Group { id: next_group, name: preset.name, slots }` on the end.
  - With no slots loaded, it returns an error that includes the warnings, and the history isn't touched.
  - It returns `InsertOutcome { outcome: EditOutcome, warnings }`.

### Commands
- `list_presets`
- `save_group_preset { fileId, groupId, name, replace }` returns `{ saved, presets }`; `saved: false` means the name is taken.
- `rename_preset { presetId, name, replace }` returns the same shape.
- `delete_preset { presetId }` returns the new list.
- `insert_preset { fileId, presetId }` returns `{ outcome, warnings }`. `chain_change` becomes generic over a result type that can hand over its `EditOutcome` (a small trait implemented by `EditOutcome` and `InsertOutcome`), so insertion follows the same playback and editor path.

### Frontend
- **`presets.svelte.ts`** holds `presets.list`, refreshed at startup and replaced by the list each preset command returns.
- **Insert Preset submenu:** `menu.ts` keeps the submenu up to date with an `$effect` in `App.svelte` that calls `setPresetMenu(list, enabled)`.
  - It removes the old items and appends one `MenuItem` per preset (action `insertPreset(id)`), or a disabled "No presets" item.
  - Then a separator and Manage Presets….
  - It is rebuilt only when the list or its enabled state changes.
- **Name prompt:** a small modal, `PresetNameDialog.svelte`, asks for the name. It is used by Save as Preset and by rename in the manager.
  - It validates 1 to 64 trimmed characters with `valid_name`'s rules, mirrored in TypeScript.
  - On `saved: false` it shows a native message dialog ("Replace" / "Cancel"), as the close prompt does. "Replace" calls again with `replace: true`, and "Cancel" keeps the prompt open.
- **Effects panel:** the group header gains a Save as Preset button. The Effects menu gains Save Group as Preset…, enabled by `canRenameGroup()` (a highlighted group).
- **`PresetManager.svelte`**, modelled on `PluginManager.svelte`, lists the presets (name and effect count) with Rename and Delete buttons. Delete confirms through a native message dialog.
- **Insert results:** inserting shows warnings through the existing `showEffectWarnings`. When nothing could be inserted it shows an error notice, as other chain errors do.

## Risks / Trade-offs

- **[A preset of large plugins makes a large JSON file that is rewritten on every save]** → Accepted: preset operations are rare, and session files already do the same.
- **[Plugin versions change between saving and inserting]** → This is the same as sessions: the plugin gets its saved state and may reject it, in which case it is inserted with its defaults and a warning.
- **[The native menu can't be built from Svelte state directly]** → The rebuild is done from one `$effect`, and items are matched by preset id.
- **[A failed library write]** → `save`, `rename` and `delete` return the error, which is shown as a notice, and the in-memory list is not changed, so the library on disk and on screen stay the same.
