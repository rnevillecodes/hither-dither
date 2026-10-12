# Proposal

## Why

Every edit, undo, redo and handle release currently stops playback and re-analyses the whole file before anything updates. On a long recording that takes seconds behind a progress box, and you can't keep editing meanwhile. This makes it slow to try a fade, listen, and adjust. The edit model is already non-destructive: the audio is read through an edit list, and new audio is only written when saving. What's slow is the full analysis pass after each change. Editing should feel live: changes show and play right away, and the expensive work happens in the background or when the file is saved.

## What Changes

- **Edits apply instantly.**
  - Edit menu operations, undo, redo and handle releases update the waveform, overview, ruler, length, handles and playback right away, with no progress box.
  - Editing stays enabled while earlier changes are still being analysed.
- **The waveform comes from the edit list.**
  - The waveform is drawn from the original file's peak data, mapped through the current edits, so no new peak data has to be built after an edit.
  - Where gains vary across a pixel column (fades, crossfades), the drawing never hides a peak but may slightly overstate one when zoomed out.
  - It becomes exact once the background analysis finishes.
- **Whole-file statistics and clip markers update in the background.**
  - A cancellable analysis runs shortly after the last change. Another change restarts it.
  - Until it finishes, the stats panel marks the whole-file figures as updating.
  - Selection statistics are already computed on demand, so they stay live.
- **Playback keeps going through edits.**
  - Editing, undo and redo no longer stop playback. The playing audio switches to the edited version within a fraction of a second.
  - Playback continues from the same point in the audio. If that audio was removed, it continues from the join.
  - **BREAKING (behaviour):** removes the "Editing stops playback" requirement.
- **Live audio preview.**
  - While a trim, fade, crossfade or bend handle is being dragged during playback, you hear the change as you drag, not only after release.
  - The Fade In / Fade Out curve dialog previews its curve on the waveform and in playback while it is open.
  - Release, or Apply in the dialog, commits one undo step. Escape and Cancel restore what was there.
- **New audio is rendered only when saving.**
  - Save, Save As and saving on close/quit are the only steps that render ("bounce") the edited audio. They show the delayed progress box while they write.
  - If the background analysis of the saved state hasn't finished, saving completes it so the tab ends up with exact peaks and statistics.

Assumptions:
- "Saving/exporting/bouncing" refers to the existing Save, Save As and save-on-close/quit paths. No separate Export or Bounce command is added.
- Playback switching to edited audio within about 0.25 s is good enough for live preview, and a short declick ramp at the switch is acceptable.
- Opening a file still analyses it once up front, as it does today.

Non-goals:
- Incremental re-analysis of only the changed ranges. The background pass is still a full pass, just non-blocking.
- Rendering edits to a cache file before saving.
- Previewing menu operations other than Fade In/Out before they are applied.
- Changing the edit model, the curve maths or the saved file format.

Depends on `add-clip-fades-and-crossfades` (same branch). This change modifies its `clip-fades` requirements, so that change must be archived first.

## Capabilities

### New Capabilities
- `live-editing`: how edits take effect without blocking: instant edit/undo/redo, waveform drawn from the edit list, background analysis with an "updating" state, playback that follows edits, live audio preview of handle drags and the fade curve dialog, and rendering only on save.

### Modified Capabilities
- `audio-editing`:
  - "The app shows the edited audio": updates are immediate, with no progress box or editing lock; whole-file stats and clip markers follow in the background.
  - "Editing stops playback": removed.
- `clip-fades` (from `add-clip-fades-and-crossfades`):
  - "Live preview while dragging": now includes audio during playback.
  - "Clip changes are undoable settings": no longer stops playback or blocks for re-analysis.
- `wav-export`: "After saving" now covers the progress shown while rendering, and finishing any pending analysis so the saved tab is exact.

## Impact

- **Backend:**
  - `state.rs`: `LoadedFile` keeps the source's peak pyramid plus an optional analysis of the current edit state. `change`/`step` no longer analyse inline. A background analysis worker per file is debounced, cancellable and tagged with a revision.
  - `peaks.rs`: a query through an `EditList`, with conservative gain scaling and exact reads for short gained spans.
  - Playback (`playback/mod.rs`, `engine.rs`): swap the playing `Audio` mid-stream, with position remapping through the edit list and a short declick. A transient preview edit list for drags and the curve dialog.
  - `save` renders, then reuses or completes the analysis.
- **Commands:**
  - Edits, undo and redo return without re-analysis, and their `onProgress` argument is dropped.
  - New: `wait_analysis(fileId, revision)`, `preview_edits(fileId, preview | null)`.
  - `save_file` gains `onProgress`.
  - `docs/ipc.md` is updated to match.
- **Frontend:**
  - `editActions.ts` no longer halts playback, shows progress or remounts the waveform on edits.
  - `PeakLoader` and the overview refetch on a revision change, keeping the old tile until the new one arrives.
  - The stats panel gets an "updating" state.
  - `Waveform.svelte` drags and `CurveDialog.svelte` send preview updates during playback.
- **Docs:** `docs/acceptance.md` rows for the new and changed scenarios.
- **Dependencies:** none new.
