# Proposal

## Why

Now that a chain can hold any number of effects, a long chain becomes hard to scan and manage one row at a time. Effects usually come in jobs, such as "vocal cleanup" (EQ, de-esser, compressor) or "mastering" (limiter, meter). Users want to name such a job, fold it away, switch it off as one, and move or delete it in one go, without changing how the audio is processed.

## What Changes

- **Groups in the chain:** neighbouring effects can be combined into a named group. Audio still runs through every effect one after another, in the order shown. A group is a way of organising the chain, not a new kind of routing.
- **Selecting effects:** the user can select several effects in the Effects panel with click, Ctrl/Cmd+click and Shift+click.
- **Group and Ungroup:**
  - **Group Effects** (Ctrl/Cmd+G; the panel and the Effects menu) puts the selected effects in a new group named "Group N", placed where the first selected effect was.
  - **Ungroup** (Ctrl/Cmd+Shift+G) removes the group and leaves its effects in place, in the same order.
- **What a group does:**
  - It shows as a header (a collapse arrow, its name, how many effects it holds, a bypass switch and a remove button), with its effects indented below.
  - It can be renamed, collapsed, dragged as a block, or moved with Alt+Up/Down.
  - Removing it removes all of its effects.
  - Effects can be dragged into or out of a group.
- **Group bypass:** a group has its own bypass switch. While it is on, every effect in the group passes audio through unchanged, but each effect keeps its own switch, so turning the group bypass off restores exactly what was on before. It works like Bypass All, but for one group.
- **One level only:** groups can't hold other groups. Grouping a selection that includes a group puts that group's effects into the new group.
- **History:** grouping, ungrouping, renaming, moving a group and removing a group are each one undo step. Like other bypass switches, group bypass is not an undo step, but it marks the tab as unsaved. Collapsing is a view setting: it is not an undo step and is not saved.
- **Sessions:** session files store each group's name, bypass switch and members, and opening a session restores them. A session that has groups is written as format version 2. Older builds then refuse it with the existing "made by a newer version" notice, instead of opening it ungrouped and sounding different when a group was bypassed. A session without groups is still written as version 1.

## Capabilities

### New Capabilities
- `effect-groups`: grouping and ungrouping effects in a tab's chain, and what a group does: naming, collapsing, moving, removing, group bypass, undo, and the unsaved marker.

### Modified Capabilities
- `session-files`: "Session file contents" now includes each group's name, bypass switch and members, and says which format version is written.

## Impact

- **Rust:**
  - `src-tauri/src/state.rs`: the history snapshot gains groups, plus new chain operations (group, ungroup, rename, move and remove group, group bypass) and capturing and restoring sessions.
  - `src-tauri/src/fx/chain.rs`: a slot's bypass now also counts its group's bypass, and `ChainInfo` gains groups.
  - `src-tauri/src/session.rs`: a new groups field and format version 2.
  - `src-tauri/src/commands.rs` and `lib.rs`: new commands.
  - Tests in `src-tauri/tests/state.rs` and `session.rs`.
- **Frontend:**
  - `src/lib/api.ts` and `src/lib/editActions.ts`: new calls and actions.
  - `src/lib/chainOrder.ts`: new drop and move rules that know about groups.
  - `src/components/EffectsPanel.svelte`: selection, group headers, collapsing, renaming and drag behaviour.
  - `src/lib/menu.ts`: Group Effects and Ungroup.
- **Docs:** the README's VST3 effects section.
- **Compatibility:** no new dependencies. Sessions without groups are unchanged. A session with groups can't be opened by older builds, as described above.
