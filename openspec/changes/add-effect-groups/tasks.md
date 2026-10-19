# Tasks

## 1. Chain model and group bypass (Rust)

- [x] 1.1 In `src-tauri/src/state.rs`, change `Snapshot.chain` to `Vec<Item>` (`Effect(SlotId)` or `Group { id, name, slots }`), add a `GroupId` counter and a `slot_order()` flattener, and switch `History::chain()`, `release_unreferenced`, `capture`, `add_effect`, `remove_effect` (dropping a group it empties) and the bypass checks over to it. Verify `npm run test:rust` passes with the existing chain tests unchanged.
- [x] 1.2 Add `History.group_bypass` (outside the snapshots, pruned in `release_unreferenced`). Make `Chain::new` in `src-tauri/src/fx/chain.rs` take each slot's group-bypass flag, so that `wanted_bypass = bypass_all || group || own`. Add `groups: Vec<GroupInfo>` to `ChainInfo` (camelCase `groupId`, `name`, `bypassed`, `slotIds`). Verify `npm run test:rust` passes and that a chain with no groups serializes `groups: []`.

## 2. Group operations (Rust)

- [x] 2.1 Add `group_effects`, `ungroup`, `rename_group`, `move_group` and `remove_group` to `AppState`, each one undo step through `change_chain`, following the effect-groups spec: "Group N" naming, placement, regrouping members of other groups, the 1–64 character trimmed name, and removing emptied groups. Add tests to `src-tauri/tests/state.rs` for the "Group neighbours", "Group effects that aren't neighbours", "Group across groups", "Whole group regrouped", "Ungroup in place", "Rename"/"Empty name", "Move a group", "Remove a group", "Undo removing a group" and "Undo grouping" scenarios. Verify `npm run test:rust` passes.
- [x] 2.2 Change `move_effect` to take a `{ group: Option<GroupId>, index }` target, and update the existing `move_effect` tests in `src-tauri/tests/state.rs`. Add tests for "Drag into a group", "Keyboard out of a group", moving the last member out (the group goes), and moving into a group that doesn't exist (an error, nothing changes). Verify `npm run test:rust` passes.
- [x] 2.3 Add `set_group_bypass` through `change_bypass`. Test that it marks the tab unsaved without adding an undo step, and that in the "Bypass a group, then restore" scenario the slots' processed and skipped states match, with the member's own bypass kept. Add an export test for "Export with a bypassed group" using HD Test Gain. Verify `npm run test:rust` passes.
- [x] 2.4 Add the Tauri commands for 2.1 to 2.3 in `src-tauri/src/commands.rs`, wrapped in `chain_change` (`set_group_bypass` like `set_effect_bypass`), register them in `lib.rs`, and document them and the new `move_effect` target in `docs/ipc.md`. Verify `npm run lint:rust` passes.

## 3. Session format (Rust)

- [x] 3.1 In `src-tauri/src/session.rs`, add `StoredGroup { name, bypassed, first, count }` and `StoredChain.groups` (left out when empty). Raise `VERSION` to 2, and write version 2 only when there are groups. When reading, reject group ranges that are out of bounds, overlap, are empty, or have a bad name as not a valid session. Add unit tests for both versions, each rejection, and a version 1 file reading unchanged. Verify `npm run test:rust` passes.
- [x] 3.2 Capture groups in `capture`, and restore them in `restore_session` through the saved-index-to-restored-slot map, dropping groups with no restored members. Add tests to `src-tauri/tests/state.rs` for "Round trip with a group" (name, bypass, members and settings), "Group member missing", "Groups captured" (format version 2) and "No groups, old format" (version 1). Verify `npm run test:rust` passes.

## 4. Frontend logic

- [x] 4.1 In `src/lib/api.ts`, add `GroupInfo`, `ChainInfo.groups`, the new command wrappers and the new `moveEffect` target. In `src/lib/editActions.ts`, add `groupEffects`, `ungroup`, `renameGroup`, `moveGroup`, `removeGroup`, `setGroupBypass`, `canGroup` and `canUngroup`. In `src/lib/effects.svelte.ts`, add per-tab selection and collapsed sets, cleared of ids that left the chain after each outcome. Verify `npm run check` passes.
- [x] 4.2 In `src/lib/chainOrder.ts`, add `chainRows(chain, collapsed)`, `dropTarget` (headers only between top-level items; effects join a group when dropped between members or in its indented area) and `stepTarget` (Alt+Up/Down leaving a group at its edges and moving a header past a whole item). Also add selection helpers for click, Ctrl/Cmd+click, Shift+click and header click. Cover them in `src/lib/chainOrder.test.ts`, including the "Range selection" and "Toggle one" scenarios. Verify `npm test` passes.

## 5. Effects panel and menu

- [x] 5.1 Update `src/components/EffectsPanel.svelte`:
  - selectable rows with a visible selected state;
  - group headers (collapse arrow, name, count, bypass, remove) with their effects indented;
  - collapsing;
  - inline rename (double-click or F2, Enter, Escape);
  - group dragging and the drop marker indented when the drop joins a group;
  - Alt+Up/Down through `stepTarget`;
  - a Group button that is enabled while effects are selected.

  Verify `npm run check` passes, then check by hand in `npm run tauri dev` that each effect-groups scenario that the tests don't cover works: "Collapse", "Rename", "Move a group", "Drag into a group", "Keyboard out of a group" and "Bypass a group, then restore", by ear with a real plugin.
- [x] 5.2 Add Effects › Group Effects (CmdOrCtrl+G) and Effects › Ungroup (CmdOrCtrl+Shift+G) to `src/lib/menu.ts`, with enabled states from `canGroup()` and `canUngroup()`. Verify `npm run check` passes, and check by hand that both are greyed out with nothing selected ("Nothing selected") and that the shortcuts work.

## 6. Docs and acceptance

- [x] 6.1 Add an "Effect groups" paragraph to the README's VST3 effects section (grouping, ungrouping, group bypass, and that grouped sessions need this version or later), and an `effect-groups` section to `docs/acceptance.md` listing the hand checks from 5.1 and 5.2. Verify both read correctly in the Markdown preview.
- [x] 6.2 Run the full set of checks (`npm run check`, `npm test`, `npm run test:rust`, `npm run lint:rust`) and `openspec validate add-effect-groups --strict`. Verify all pass.

## Workflow follow-up

- Archive `add-effect-groups` after review, so `effect-groups` becomes a main spec and the `session-files` change is merged.
