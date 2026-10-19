# Proposal

## Why

A tab's effect chain is capped at 8 effects. Nothing in the code needs that limit: the chain is a list processed in order, with no fixed-size buffers. The real cost of a long chain is CPU, which depends on which plugins are used far more than on how many. The app already warns when playback can't keep up, and Save renders offline, so the cap only blocks chains that would work fine.

## What Changes

- Remove the effect count limit. Add Effect is never disabled because the chain is long, and the "chain is full" tooltip goes away.
- Opening a session restores every effect in it, however many there are. It no longer leaves effects out with the "The effect chain is full" notice.
- Remove the `MAX_EFFECTS` constant from the Rust backend and the frontend, along with the test that checks the cap.
- README: drop "up to 8" from the VST3 effects section.
- Website (`site/index.html`):
  - Drop "up to eight" from the effect chains feature card.
  - Change the wording to "with VST3 plugin support" in the meta description (which now says "with VST3 effect chains") and in the hero tagline (which now says "with editing and VST3 effects when you need them").

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `effect-chain`: the "Add an effect" requirement drops the 8-effect limit and its "Full chain" scenario.

## Impact

- Rust: `src-tauri/src/fx/chain.rs` (constant), `src-tauri/src/state.rs` (`add_effect` check, session restore check), `src-tauri/tests/state.rs` (`a_full_chain_refuses_another_effect`).
- Frontend: `src/lib/api.ts` (constant), `src/lib/editActions.ts` (`canAddEffect`), `src/components/EffectsPanel.svelte` (the "full" state and tooltip).
- Docs: `README.md`, `site/index.html`.
- The `effect-chain` capability exists only as a delta in the unarchived `add-vst3-effect-chain` change. That change must be archived before this one, so that this change's MODIFIED requirement has a main spec to apply to.
- No IPC, session-file format or dependency changes. A session saved with more than 8 effects can't be opened fully by an older build, which leaves out the extras with a notice.
