# Design

## Context

The cap is the constant `MAX_EFFECTS = 8`, which is defined twice: in `src-tauri/src/fx/chain.rs` and, copied by hand, in `src/lib/api.ts`. The code uses it in four places:

- `AppState::add_effect` refuses with "The effect chain is full".
- Session restore in `state.rs` leaves out any effect past the 8th, with a warning.
- `canAddEffect` in `editActions.ts` uses it, and that function drives both the Effects panel button and the enabled state of the native menu item.
- `EffectsPanel.svelte` derives a `full` flag from it to pick the button's tooltip.

The chain is a `Vec` of slots, and the playback feeder and the Save render walk it in order. Nothing is sized by the cap.

## Goals / Non-Goals

**Goals:**
- Remove the constant and every check that depends on it, so the backend and frontend can't disagree.

**Non-Goals:**
- Any replacement limit, whether a larger count or a CPU or latency budget. The existing overload notice (the feeder can't keep up during playback) remains the only feedback for heavy chains.
- Changes to the Effects panel layout for long chains. The panel already scrolls inside the sidebar.

## Decisions

- **Delete rather than raise.** Raising the cap to 16 or 32 would still be arbitrary, because the cost of a chain depends on which plugins are in it. Removing it leaves the overload warning, which measures the real problem, to do the job. *Alternative:* have the backend report a limit through `ChainInfo` so the frontend stops hardcoding it. That isn't needed once there is no limit.
- **`canAddEffect` keeps its other conditions.** It still requires an active tab with a chain. Only the length comparison goes.
- **Replace the cap test with a long-chain test.** `a_full_chain_refuses_another_effect` becomes a test that adds more than 8 effects. It checks that all of them are in the chain and that undo removes the last one. A session test checks that a saved chain with more than 8 effects restores in full. Both tests use the existing HD Test Gain plugin.

## Risks / Trade-offs

- [Very long chains can make live playback drop out] → The existing overload notice already covers this, and Save is unaffected because it renders offline. The README already notes this kind of behaviour.
- [Total latency grows with chain length, so playback takes longer to start and the chain takes longer to drain] → Accepted. The latency is the sum of what the plugins report, and it is already compensated.
- [Older builds open such a session only partly] → An older build leaves out the effects past the 8th, with its existing notice. Nothing is lost silently.
- [The `effect-chain` main spec doesn't exist yet] → This change's delta modifies a requirement that `add-vst3-effect-chain` adds. That change must be archived first. See the workflow follow-up in tasks.md.
