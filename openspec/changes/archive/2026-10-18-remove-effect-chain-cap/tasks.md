# Tasks

## 1. Backend

- [x] 1.1 Remove `MAX_EFFECTS` from `src-tauri/src/fx/chain.rs` and the full-chain check (and its doc-comment mention) from `AppState::add_effect` in `src-tauri/src/state.rs`. Replace `a_full_chain_refuses_another_effect` in `src-tauri/tests/state.rs` with a test that adds 10 HD Test Gain effects, then checks that all 10 are in the chain and that one undo leaves 9 (spec "Long chain"). Verify `npm run test:rust` passes.
- [x] 1.2 Remove the "The effect chain is full" check from session restore in `src-tauri/src/state.rs`. Add a test that saves a session with 12 effects, restores it in another tab with no warnings, and gets the same effect names in the same order (spec "Long chain restored from a session"). Verify `npm run test:rust` and `npm run lint:rust` pass, and that `grep -rn MAX_EFFECTS src-tauri` finds nothing.

## 2. Frontend

- [x] 2.1 Remove `MAX_EFFECTS` from `src/lib/api.ts`. In `src/lib/editActions.ts`, make `canAddEffect` return whether the active tab has a chain. In `src/components/EffectsPanel.svelte`, drop the `full` derived value so the tooltip is always "Add Effect…". Verify `npm run check` and `npm test` pass, that `grep -rn "MAX_EFFECTS\|chain is full" src` finds nothing, and, by hand in `npm run tauri dev`, that a 9th effect can be added from both the panel button and Effects › Add Effect….

## 3. Docs and website

- [x] 3.1 In `README.md` ("VST3 effects" section), change "a chain of up to 8 VST3 effect plugins" to "a chain of VST3 effect plugins". Verify `grep -n "up to 8" README.md` finds nothing.
- [x] 3.2 In `site/index.html`, make these changes:
  - Meta description: "with VST3 effect chains" becomes "with VST3 plugin support".
  - Hero tagline: "with editing and VST3 effects when you need them" becomes "with editing and VST3 plugin support when you need them".
  - Effect chains card: "Run each tab through up to eight VST3 effects" becomes "Run each tab through a chain of VST3 effects".

  Verify by opening `site/index.html` in a browser at desktop and phone widths, and check that `grep -niE "up to eight|effect chains\\." site/index.html` finds nothing.

## Workflow follow-up

- Archive `add-vst3-effect-chain` before this change, so the `effect-chain` main spec exists for this change's MODIFIED requirement to apply to.
- Open a PR from `feat/remove-effect-cap` into `main` (main is PR-protected).
