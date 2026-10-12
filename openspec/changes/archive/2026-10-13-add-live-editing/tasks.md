# Tasks

## 1. Position mapping in the edit model

- [x] 1.1 Add `EditList::source_at(out)` and `EditList::output_of(src) -> Located::{At, Removed { next }}` in `src-tauri/src/edit/mod.rs`. Cover trims, deletes, crops, crossfade spans (each segment maps by its own frames) and frames outside the trim window. Add unit tests for the spec scenarios "Delete before the playhead" (30.0 s → 20.0 s), "Delete the audio being played" (15.0 s → join at 10.0 s) and an end trim before a position (Removed, no next). Verify `npm run test:rust` passes.
- [x] 1.2 Cache `Audio::spans()` on `Audio` (computed once in `with_edits`) and make the writer and callers use the cached spans. Verify the existing writer and audio tests still pass with `npm run test:rust`.

## 2. Peaks and clip runs without re-analysis

- [x] 2.1 Add `peaks::query_edited(source_pyramid, audio, start, end, columns)` as in design.md: verbatim spans from the source pyramid, changed parts of at most 4096 frames read exactly, and longer changed parts bounded by the gain range (silence 0, crossfade = sum of both sides). Add tests against brute force over random edit lists (silence, curved fades, clip fades, trims, long and short crossfades) asserting: exact for verbatim columns and for columns of ≤ 4096 changed frames; otherwise `got.max ≥ true.max` and `got.min ≤ true.min`; and a transient inside a long fade-out is never below its faded height ("No peak hidden"). Verify `npm run test:rust` passes.
- [x] 2.2 Add mapping of source clip runs through verbatim spans (cut to spans, drop runs under 3 frames, shift to output positions) with tests: runs in a silenced or faded region vanish, runs after a delete move left by the deleted length. Verify `npm run test:rust` passes.

## 3. Background analysis in `AppState`

- [x] 3.1 Restructure `LoadedFile` to keep `audio`, `source_pyramid`, `source_runs` and `analysis: Option<EditAnalysis { revision, pyramid, whole }>`. Make `change`/`step` swap the audio and bump the revision without analysing, and drop their `progress` parameter. Seed the identity state with the open-time analysis. Route `get_peaks`, `compute_stats(null)` (returns `None` while stale) and `clip_runs` (mapped runs plus `pending`) through the current analysis or the fallbacks from group 2. Update `tests/state.rs`: edits succeed back to back with no "still being analysed" error; `compute_stats(null)` is `None` straight after an edit; peaks after a delete match a fresh file containing the same audio within the bound rules. Verify `npm run lint:rust` and `npm run test:rust` pass.
- [x] 3.2 Add the per-file analyser: 300 ms debounce, cancel on change or close, install only if the revision is still current, and `wait_analysis(id, revision) -> bool`. Add `tests/state.rs` cases: five rapid edits run one completed analysis for the last revision ("Rapid changes"); `wait_analysis` for a superseded revision returns `false`; after waiting, whole stats equal those of an unedited file with the same audio; closing the file during analysis resolves waiters with `false`. Verify `npm run test:rust` passes.
- [x] 3.3 Make `save` finish the current revision's analysis synchronously if it isn't installed, then render with progress over a new `onProgress`, carrying the analysis over to the saved file. Add `tests/state.rs` cases: save immediately after an edit leaves exact stats and peaks ("Save before the analysis finishes"), and progress reaches the total. Verify `npm run test:rust` passes.

## 4. Playback that follows edits

- [x] 4.1 Add the retarget slot to `Shared`, retarget handling in the feeder (reset resampler, flush marker, new range and position) and flush plus 5 ms fade-in in `Engine::render`. Add engine tests that drive `render` directly: after a retarget the next rendered samples come from the new audio with no stale samples; positions are reported in the new timeline; the first 5 ms ramp up from 0; an empty buffer right after a flush does not set `finished`. Verify `npm run test:rust` passes.
- [x] 4.2 Add `Player::follow(src, new_audio)` (maps position and range through `source_at`/`output_of`, whole-file ranges become the new whole file, a removed position with nothing after it ends playback) and `Player::set_play_range`. Call `follow` from every change, undo and redo of the playing file instead of stopping. Add tests for the spec scenarios "Fade while listening", "Delete before the playhead", "Delete the audio being played" and "Looping selection" using the test render path. Verify `npm run test:rust` passes.
- [x] 4.3 Add `preview_edits(fileId, preview | null)` (clip, crossfade, fade) that follows a transient edit list for the playing file without touching history, and the `set_play_range` and `wait_analysis` commands. Drop `onProgress` from `apply_edit`, `undo_edit`, `redo_edit`, `set_clip` and `set_crossfade`, and add it to `save_file`. Add `tests/state.rs` cases showing that a preview leaves `edit_state` (undo, dirty, revision) unchanged. Update `docs/ipc.md` (Editing, Clip handles, Playback and the new commands). Verify `npm run lint:rust` and `npm run test:rust` pass.

## 5. Frontend edit flow

- [x] 5.1 Update `api.ts` for the new and changed commands. Rework `run()` in `editActions.ts`: no `haltPlayback`, no progress box or lock for edits, `statsPending` set, and a fire-and-forget `waitAnalysis` that refetches whole stats and clip runs and bumps a peaks version. After `afterEdit`, call `setPlayRange` while playing when the selection that defined the range changed, and remap `playStart` by the "same audio or the join" rule. Keep the progress box and lock for Save, wiring its progress. Verify `npm run check` and `npm test` pass, with unit tests for the `playStart` remapping in `editRules.test.ts`.
- [x] 5.2 Stop remounting the waveform on `revision`: give `PeakLoader` a version that invalidates `covers()` while keeping the old tile drawn, update the viewport in place, and invalidate the overview cache by version. Show "Updating…" and dimmed whole-file figures in `StatsPanel.svelte` while `statsPending`. Verify `npm run check` passes and manually confirm the "Old view kept until redrawn" and "Stats updating" scenarios on `beep-every-second-mono-60s.wav`.
- [x] 5.3 Send `previewEdits` from handle drags in `Waveform.svelte` (on pointer move while this tab is playing, one in flight and latest wins; `null` on Escape) and from `CurveDialog.svelte` (each bend change; `null` on Cancel or Escape). Draw the dialog's curve over the selection with the renderer's envelope pass. Verify `npm run check` passes and manually check the "Shape a fade by ear", "Cancel a heard drag", "Audition a curve", "Cancel the preview" and clip-fades "Hear a trim while playing" scenarios.

## 6. Acceptance docs

- [x] 6.1 Add a `live-editing` section to `docs/acceptance.md` with one row per scenario in `specs/live-editing/spec.md`, update the audio-editing rows ("The app shows the edited audio" scenarios, and remove "Editing stops playback"), the clip-fades rows ("Live preview while dragging", "Clip changes are undoable settings") and the wav-export "After saving" rows, citing the automated tests from groups 1–4. Verify every scenario in this change's specs has a row.

## 7. Integration checks

- [x] 7.1 Run `npm run check`, `npm test`, `npm run lint:rust` and `npm run test:rust`, and verify they all pass.
- [ ] 7.2 End-to-end in `npm run tauri dev` with the 1 GB test file: loop a selection and make a delete, a fade and a crossfade change while it plays, with no stop, no progress box, and the change heard within a moment; hold Ctrl/Cmd+Z down through the history; watch the stats panel go from "Updating…" to fresh figures; drag a fade handle and audition a curve in the dialog during playback; Save and confirm the progress box, exact stats afterwards, and that the saved file matches what was heard when reopened. Confirm no files appear next to the source before saving ("Many edits, no files").

## Workflow follow-up

- Archive `add-clip-fades-and-crossfades` before archiving this change, since this change modifies its `clip-fades` requirements.
