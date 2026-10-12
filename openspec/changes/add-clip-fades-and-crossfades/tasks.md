# Tasks

## 1. Curve

- [x] 1.1 Add `src-tauri/src/edit/curve.rs` (`Curve { bend }`, `gain(dir, t)` with `p = 4^(−bend)`, bend clamped to [−1, 1]) and `src/lib/curve.ts` with the same formula, plus `bendFromPoint(height)` for dragging the bend point. Add the shared test vector (bends −1, −0.5, 0, 0.5, 1 × t = 0, ¼, ½, ¾, 1) to a Rust unit test and `src/lib/curve.test.ts`. Verify `npm run test:rust` and `npm test` pass with identical expected values.

## 2. Edit model: curved ramps, trim, clip fades, crossfades

- [x] 2.1 Change `Ramp` to `{ t0, t1, dir, curve }` and give `fade_in`/`fade_out` a `Curve` argument. Update `Segment::gain`/`split` and the existing tests, and add tests that a curved fade split by a delete keeps its gains and that two linear fades still square. Verify `npm run test:rust` passes.
- [x] 2.2 Add `trim`, `fade_in`, `fade_out` to `EditList` and `xfade` to `Segment`, with `set_trim`, `set_clip_fades`, `set_crossfade(join, …)`, `max_crossfade(join)` and `joins()`. Translate menu operations by `trim.start`; make Delete adjust `trim.end` and add a default 10 ms linear crossfade (clamped); make Crop reset `trim`; clamp clip fades when the audio gets shorter. Add tests for: trim then delete then restore the trim (spec "Restore trimmed audio"); trim can't go below 1 frame; fades clamp to the length and follow trims; Delete's default crossfade and its clamping near the ends; Crop resetting the trim. Verify `npm run test:rust` passes.

## 3. Rendering edited audio

- [x] 3.1 Extend `Audio::read`/`read_with_full_scale` with the trim window, crossfade mixing (`A_ext`/`B_ext` as in design.md) and clip fade gains, and add `render_native` and `plain_spans`. Extend the reference renderer in `edit/audio.rs` tests to cover trims, clip fades and crossfades (including length 0 = hard join, and crossfades over faded or silent neighbours), and compare whole-file and 7-frame-piece reads. Verify `npm run test:rust` passes.
- [x] 3.2 Make the writer copy `plain_spans` raw and encode everything else from `render_native`. Update `untouched_samples_are_bit_exact_after_delete` to exclude the crossfade span, and add tests that a delete with a zero-length crossfade is bit-exact, that crossfaded `f64` samples match the reference to 1e-12, and that 24-bit crossfade samples stay on the valid-bit grid. Verify `npm run test:rust` passes.

## 4. Commands

- [x] 4.1 Add `AppState::set_clip`, `set_crossfade` and `clip(id)` (re-analysed history entries like `apply_edit`), pass `bend` through `apply_edit`, include the clip data in `EditOutcome`, and add the `set_clip`, `set_crossfade` and `get_clip` commands. Add `tests/state.rs` cases: set a fade, adjust it, undo restores the first setting; Save bakes the trim and fades in and leaves an empty clip state; `set_crossfade` beyond `maxLen` is clamped. Document the commands and the new `EditOutcome` field in `docs/ipc.md`. Verify `npm run lint:rust` and `npm run test:rust` pass.

## 5. Handles and live preview

- [x] 5.1 Create `src/lib/clipHandles.ts` (handle geometry from clip data and viewport, 6 px hit-testing, drag maths for trim, fade length, crossfade length and bend, all clamped like the backend) with vitest cases for hit priority, clamping (spec "Can't trim to nothing", "Fades can't overlap") and bend from pointer height. Verify `npm test` passes.
- [x] 5.2 Add the `setClip`/`setCrossfade`/`getClip` wrappers and `bend` on `applyEdit` to `api.ts`, keep the clip data on the `Tab`, and route handle commits through `run()` in `editActions.ts`. Verify `npm run check` passes.
- [ ] 5.3 In `renderer.ts`, draw the trimmed-region shading, the envelope line, the envelope-scaled waveform, the handles (fade squares, bend circles, crossfade markers) and the drag label. In `Waveform.svelte`, hit-test handles before selection, drive a local preview during drags, commit on release, cancel on Escape, reset the bend on double-click, and set cursors and tooltips. Verify manually against every scenario in `specs/clip-fades/spec.md` using `cd-16bit-stereo-10s.wav` and `beep-every-second-mono-60s.wav`.

## 6. Curve dialog for Fade In / Fade Out

- [ ] 6.1 Create `src/components/CurveDialog.svelte` (modal, curve graph with draggable bend point, Apply/Cancel, Escape cancels, focus trapped) and make Edit › Fade In/Out open it and call `applyEdit(op, bend)`. Verify manually against the MODIFIED "Fade in and fade out" scenarios in `specs/audio-editing/spec.md` (including Cancel and the curved fade).

## 7. Docs

- [x] 7.1 Add a `clip-fades` section to `docs/acceptance.md` with one row per scenario in `specs/clip-fades/spec.md`, and update the audio-editing (Delete, Fade In/Out) and wav-export (Output format) rows to match the MODIFIED requirements, citing the new automated tests. Verify each scenario has a row.

## 8. Integration checks

- [x] 8.1 Run `npm run check`, `npm test`, `npm run lint:rust` and `npm run test:rust`, and verify all pass.
- [ ] 8.2 End-to-end in `npm run tauri dev`: trim both ends of a take, add curved fades, delete a section and lengthen its crossfade, undo/redo through the history, Save As, reopen the saved file and confirm its duration and waveform match what was shown, and play it in another audio tool to confirm the fades and the crossfade sound smooth.
