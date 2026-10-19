# Design

## Context

- **History** (`src-tauri/src/state.rs`): each `Snapshot` holds `chain: Vec<SlotId>` in processing order. `History.slots` keeps every slot any state refers to, so undo can bring an effect back with its settings, and `release_unreferenced` drops the rest. Bypass All lives on `History` outside the snapshots. A change outside the history (a bypass switch) goes through `change_bypass`, which calls `touch()` and then `sync_bypass`.
- **Chain operations:** `add_effect`, `remove_effect` and `move_effect` each push one snapshot through `change_chain`. The Tauri commands wrap them in `chain_change` (`commands.rs`), which hands the new chain to playback and calls `close_removed_editors`.
- **Processing** (`fx/chain.rs`): `Chain::new(slots, bypass_all)` sets each slot's `wanted_bypass` to `bypass_all || slot.bypassed()`. The feeder and the export render only ever see a flat `Vec<Arc<Slot>>`.
- **Frontend:** `ChainInfo { effects, bypassAll, latency }` reaches the panel through `EditOutcome.chain`. `EffectsPanel.svelte` draws one row per effect, with drag reordering through the pure helpers in `src/lib/chainOrder.ts` (`dropGap`, `dropIndex`, `stepIndex`, all tested), and Alt+Up/Down on a row.
- **Sessions** (`session.rs`): `StoredChain { bypassAll, effects }` is in format `VERSION = 1`. A reader rejects a higher version with the "made by a newer version" notice and ignores unknown fields.

## Goals / Non-Goals

**Goals:**
- Groups change only how the chain is organised and bypassed. Playback and export still process a flat list of slots, so no audio code changes beyond how each slot's wanted bypass is worked out.
- The data can't break the "groups are contiguous and one level deep" rule.

**Non-Goals:**
- Parallel routing, nested groups, and group presets that can be reused across tabs.
- Remembering collapsed groups across sessions or restarts.
- Group-level parameters such as a group gain.

## Decisions

### Chain as a list of items
`Snapshot.chain` becomes `Vec<Item>`:

```
enum Item { Effect(SlotId), Group { id: GroupId, name: String, slots: Vec<SlotId> } }
```

- `GroupId` is a `u32` that is unique for the life of the app, like `SlotId`.
- A helper `slot_order()` flattens the items into processing order. Everything that today reads `chain: Vec<SlotId>` (`History::chain()`, `release_unreferenced`, `capture`, the bypass checks) uses that instead.
- Groups being contiguous and one level deep is a property of the type, so no rule has to be checked. A group with no slots is removed by the operation that emptied it.

*Alternative:* keep `Vec<SlotId>` and add a separate `groups: Vec<{id, name, slots}>` with a "members are contiguous" check. That makes every move and remove maintain the rule by hand. Rejected.

### Group bypass lives beside Bypass All
`History.group_bypass: HashMap<GroupId, bool>` sits outside the snapshots, like `bypass_all`. It is touched through `change_bypass`, so it isn't an undo step but does mark the tab unsaved. Entries for groups that no state refers to are dropped in `release_unreferenced`. Because group ids are never reused, undoing the removal of a bypassed group brings back its bypass along with it, which is what the "Undo removing a group" scenario needs.

`Chain::new` takes the slots paired with each slot's group-bypass flag, and sets `wanted_bypass = bypass_all || group_bypassed || slot.bypassed()`. `skipped()`, `apply_bypass` and the feeder are unchanged.

### Operations
Each operation is a method on `AppState` that goes through `change_chain` (one undo step), plus a Tauri command wrapped in `chain_change`, so playback updates and removed editors close the same way as today:

- `group_effects(file, slot_ids)`: works out the placement and naming rules from the spec on the item list. It returns an error if `slot_ids` is empty or names a slot that isn't in the chain.
- `ungroup(file, group_id)`: splices the group's slots in at its position.
- `rename_group(file, group_id, name)`: trims the name and rejects 0 or more than 64 characters. Renaming to the same name adds no step.
- `move_group(file, group_id, index)`: `index` is a top-level item position.
- `move_effect(file, slot, target)` replaces today's `(slot, index)`. `target` is `{ group: Option<GroupId>, index }`, where the index counts within that group or the top level, after the slot is taken out. Moving an effect out of a group may leave the group empty, and then the group goes.
- `remove_group(file, group_id)`: removes the group and its slots in one step.
- `set_group_bypass(file, group_id, on)`: through `change_bypass`.
- `add_effect` still appends a top-level item.
- `remove_effect` takes the slot out of its group and drops the group if it is now empty.

### What the frontend receives
`ChainInfo` keeps `effects` flat in processing order, so the effect-count badge, the generic controls and the latency code don't change. It gains `groups: GroupInfo[]` with `{ groupId, name, bypassed, slotIds }`. The panel builds its rows with a new pure helper `chainRows(chain)`. Because groups are contiguous, that is a single pass.

*Alternative:* send the item tree as it is. Rejected, because every existing consumer of `effects` would have to flatten it.

### Panel: selection, collapsing, drag and keyboard
- **Selection** is held per tab in `effects.svelte.ts` as a set of slot ids. It isn't saved, and it is cleared of ids that left the chain after each outcome. Clicking a header selects its members.
- **Collapsed groups** are a per-tab set of group ids in the same module. New groups and restored sessions start expanded.
- **Drag and drop:** the pure functions in `chainOrder.ts` grow to work over the visible rows.
  - `dropTarget(rows, from, y)` returns a `move_effect` target, or a `move_group` index for a header.
  - A header can only drop between top-level items.
  - An effect dropped between two members of the same group joins that group. On a group's edge it goes inside if the pointer is in the indented area and outside otherwise.
  - Each rule gets vitest cases.
- **Keyboard:** Alt+Up/Down uses a `stepTarget` that leaves the group at its edges. Ctrl/Cmd+G and Ctrl/Cmd+Shift+G are menu accelerators on the new Effects › Group Effects and Effects › Ungroup items. Their enabled state follows `canGroup()` and `canUngroup()`, like Add Effect does today.
- **Renaming** happens inline: double-click or F2 turns the name into an input. Enter commits through `rename_group`, and Escape cancels.

### Session format
`StoredChain` gains `groups: Vec<StoredGroup { name, bypassed, first, count }>`, where `first` and `count` index into `effects`.

- **Writing:** the field is left out when empty. `version` is 2 when there are groups and 1 otherwise.
- **Reading:** versions up to 2 are accepted. A file is invalid if a group's range is out of bounds, overlaps another group, is empty, or has a name of 0 or more than 64 characters after trimming. Such a file gets the existing "not a valid session file" notice.
- **Restoring:** the stored effect index of each restored slot is mapped, so a group keeps whichever members loaded. A group with none left is dropped, and its effects' warnings come through as today.

*Alternative:* always write version 2. Rejected, because then any session saved by this build would be refused by older builds even when it has no groups.

## Risks / Trade-offs

- **[`move_effect`'s signature changes]** Its only caller is `editActions.moveEffect`, and both change together. The `move_effect` tests in `tests/state.rs` move to the new target form.
- **[Drag rules at a group's edges can feel ambiguous]** → The indented area decides, and the drop marker is drawn indented when the drop will join the group. This is covered by `chainOrder` tests and a hand check.
- **[A grouped session can't be opened by older builds]** → This is intended, so that a bypassed group never plays unbypassed there. Sessions without groups are unaffected.
- **[Menu accelerators fire while typing a group name]** → The rename input stops propagation of the keys it handles. Ctrl/Cmd+G isn't one of them, but grouping with the rename input focused only commits the edit first, which is harmless.
