# Proposal

## Why

Groups (see `add-effect-groups`) make a set of effects into one named job, like "Vocal cleanup" or "Master bus". The same job is often wanted on many recordings, but today the only way to reuse it is to rebuild the group, effect by effect and setting by setting, in every tab. Saving a group as a named preset that can be inserted into any tab turns that into one click.

## What Changes

- **Save as preset:** a group can be saved as a named preset. This is done with a button on the group's header, or with **Effects › Save Group as Preset…** on the highlighted group.
  - The preset stores each effect, in order, with its plugin's complete settings and its own bypass switch.
  - The name defaults to the group's name. Saving under a name that already exists asks whether to replace that preset.
- **Preset library:** presets are kept in the app's data folder, not in files the user picks, and are listed by name, sorted alphabetically, in **Effects › Insert Preset**.
- **Insert a preset:** choosing a preset adds its effects, with their saved settings and bypass switches, as a new group named after the preset, at the end of the active tab's chain.
  - This is one undo step.
  - Effects that aren't installed, are disabled, can't process the file's channels, or fail to load are left out, with one notice naming them.
  - If none of them can be added, the chain is unchanged and a notice says why.
- **Manage Presets:** **Effects › Manage Presets…** opens a dialog that lists the presets. They can be renamed there, and deleted after asking first.
- Inserting a preset doesn't link it to the group: changing the group later doesn't change the preset, and the other way round.

## Capabilities

### New Capabilities
- `effect-presets`: saving a group as a named preset, the app's preset library, inserting a preset into a tab's chain, and renaming and deleting presets.

### Modified Capabilities
<!-- none: groups, the chain and session files behave as before -->

## Impact

- **Rust:**
  - a new `src-tauri/src/presets.rs`: the library, kept in `effect-presets.json` in the app data folder and written atomically like the plugin settings;
  - `src-tauri/src/state.rs`: capturing a group's effects, shared with session capture, and inserting a preset as a group;
  - `src-tauri/src/commands.rs` and `lib.rs`: new commands and the shared library state;
  - tests.
- **Frontend:**
  - `src/lib/api.ts` and `src/lib/editActions.ts`: new calls and actions;
  - a new `src/lib/presets.svelte.ts`: the preset list;
  - `src/lib/menu.ts`: the Insert Preset submenu, kept up to date, and the other menu items;
  - a header button in `src/components/EffectsPanel.svelte`;
  - a new `src/components/PresetManager.svelte`.
- **Docs:** `docs/ipc.md`, the README and `docs/acceptance.md`.
- **Depends on** the `add-effect-groups` change, which is committed on this branch.
