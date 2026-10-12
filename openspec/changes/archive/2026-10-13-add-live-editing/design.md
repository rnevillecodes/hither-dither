# Design

## Context

- **Edit model:** `EditList` and `Audio` (`src-tauri/src/edit/`) already render edits lazily. Every read goes through the segment list, trim window, clip fades and crossfades. Nothing is written until `save` calls the writer. `Audio::spans()` splits the output into verbatim source copies and changed runs.
- **What blocks today** is in `state.rs`: `change` and `step` call `analyse_edits`, which does a full pass of `stats::analyse` with a `PyramidBuilder` before committing. While it runs:
  - `begin_analysis` rejects further edits;
  - the frontend `run()` calls `haltPlayback()`, shows the progress box and sets `tab.editing`;
  - `showOutcome` bumps `tab.revision`, which remounts the waveform.
- **Peaks:** `PeakPyramid::query(&Audio, …)` takes whole buckets from the pyramid and reads partial blocks from `Audio`, so the pyramid has to match the audio being shown. That's why each edit rebuilds it.
- **Playback:** the feeder thread in `playback/engine.rs` reads a fixed `Arc<Audio>` and a fixed `PlayRequest` in 1024-frame chunks into a 0.2 s ring buffer. Position markers map output samples back to file frames.
- **Specs:** see `specs/live-editing`, plus the MODIFIED requirements in `audio-editing`, `clip-fades` and `wav-export`.

## Goals / Non-Goals

**Goals:**
- No full pass over the audio between a user action and its visible and audible result, except Save.
- One edit state per file is "current". Analysis is a cache keyed by revision, never a gate.
- Previews (drags, curve dialog) reuse the edit model. There is no second rendering path.

**Non-Goals:**
- Incremental or partial re-analysis.
- Sharing analysis between undo states (only the latest state's analysis is kept).
- Lower ring-buffer latency in general. Only edits flush it.

## Decisions

### Analysis becomes a background cache per file
`LoadedFile` changes from "audio + pyramid + analysis of that audio" to:

```text
LoadedFile {
  metadata, disk_stamp,
  audio: Arc<Audio>,                 // current edit state, swapped on every change
  source_pyramid: Arc<PeakPyramid>,  // pyramid of the unedited source (built at open)
  source_runs: Arc<Vec<Vec<ClipRun>>>, // clip runs of the unedited source
  analysis: Option<EditAnalysis>,    // { revision, pyramid, whole } for `audio`, when done
}
```

- `change`/`step` build the new `EditList`, swap in `Audio::with_edits` (cheap), bump the revision and return at once. `begin_analysis` is no longer used for edits, so they no longer reject with "still being analysed".
- An `Analyser` per file owns one worker:
  - On each change it cancels any running pass and schedules a new one after a 300 ms debounce.
  - The pass runs `stats::analyse` with a `PyramidBuilder` over that revision's `Audio`.
  - It installs the result only if the revision is still current.
  - The identity state (just opened, or just saved) is seeded with the source analysis, so nothing runs.
- Undo back to a state that was analysed before still re-analyses. Keeping one analysis bounds memory to roughly one extra pyramid per file.
- New command `wait_analysis(fileId, revision)`: resolves `true` when that revision's analysis is installed, and `false` if a newer revision supersedes it or the file closes. The frontend awaits it after each change and then refetches `compute_stats(null)`, `get_clip_runs` and peaks. A command was chosen over a global Tauri event because it matches the existing per-call `Channel` style and needs no listener lifecycle.
- `compute_stats(null)` returns the analysis' whole stats when current. Otherwise it returns `null`, which already means "discard". `get_clip_runs` gains a `pending: boolean` field.

*Alternatives:*
- Keep analysing synchronously but faster (SIMD, parallel channels): still O(file) per edit.
- Incremental re-analysis: the right long-term answer, but stats like loudness integrate over the whole file. Out of scope (see proposal).

### Peaks through the edit list
A new `peaks::query_edited(source_pyramid, audio, start, end, columns)` serves `get_peaks` whenever the current revision isn't analysed yet. When the analysis is current it uses the edited pyramid directly, exactly as today. For each column it walks `audio.spans()` overlapping the column:
- **Verbatim spans** query the source pyramid over the matching source range. This is exact and already O(log n).
- **Changed spans** whose part in this column is at most `EXACT_READ_FRAMES` (4096) are read with `Audio::read` and folded in exactly. This makes zoomed-in views, short crossfades and sample mode exact.
- **Longer changed spans** are bounded:
  - Take the min/max of the contributing source range from the source pyramid.
  - Take the gain range `[g_lo, g_hi]` of the segment gain × clip fade gain over that part. Ramps and fades are monotonic, so the endpoints suffice.
  - The result is `max' = max ≥ 0 ? max·g_hi : max·g_lo` and `min' = min ≤ 0 ? min·g_hi : min·g_lo`.
  - Silent segments contribute 0.
  - A long crossfade contributes the sum of both sides' bounds.

  The true peak is never above the bound, which satisfies "never hides a peak".

`Audio::spans()` is cached on `Audio` (computed once per edit state), since every query and the writer call it. The binary `get_peaks` format is unchanged.

*Alternatives:*
- Rebuild the pyramid lazily per column on demand: O(file) at full zoom-out, the same problem again.
- Draw the envelope approximation in the frontend (as the drag preview does): it can't remove deleted audio or handle stacked menu fades, and it duplicates the edit model.

### Clip markers without analysis
Until the analysis is current, `get_clip_runs` maps `source_runs` through the verbatim spans:
- each run is cut to the span;
- runs shorter than 3 frames after cutting are dropped;
- each run is shifted to its output position.

Runs in changed audio disappear immediately, as the spec requires. Gained audio can't be a raw full-scale copy anyway, and the writer and the analysis apply the same rule. Runs that a hard join would merge are reconciled when the analysis lands.

### Playback follows the current edit state
`Shared` gains a `retarget: Mutex<Option<Retarget>>` slot. The feeder checks it before each chunk, so a newer retarget replaces an older one:

```text
Retarget { audio: Arc<Audio>, start: u64, end: u64, from: u64, generation: u32 }
```

- **`Player::follow(file, new_audio)`**, called by every change, undo, redo and preview of the playing file:
  - Maps the feeder's current read position and the play range from the old `EditList` to the new one through a source anchor. `EditList::source_at(out) -> Option<(segment, src)>` and `EditList::output_of(src) -> Located::{At(out), Removed { next: Option<out> }}`.
  - A source frame appears at most once in an edit list (there is no copy/paste), so the mapping is unambiguous.
  - Range ends that map to `Removed` snap to `next` (or the new end).
  - If the play range was the whole file, it becomes the whole new file.
  - `from` being `Removed { next: None }` stops playback, reported as `ended`.
- **Feeder:** on taking a retarget it resets the resampler, increments `generation`, publishes `flush_to = out_index_of_next_write`, and pushes a marker `{ out_index, file_frame: from }`.
- **Engine:** when it sees a new generation, it drops queued samples up to `flush_to` instead of playing them. It then applies a 5 ms linear fade-in to the first new samples (declick), and position reports switch to the new timeline at once.

  The worst-case delay is one device callback plus refilling one chunk, well under 0.25 s. Accepting up to 0.2 s of stale audio and stale positions instead would be simpler, but it makes the playhead jump back and forth in the edited timeline.
- **Frontend:**
  - `run()` no longer calls `haltPlayback()`.
  - The `position` events now arrive in the new timeline, so the playhead follows naturally.
  - When `afterEdit` clears or changes the selection that defined the play range, the frontend calls a new `set_play_range(start, end)`, which uses the same retarget path without changing the audio.
  - `playStart` (where Stop returns) is remapped by the frontend with the same "same audio or the join" rule, using the `ClipInfo`/outcome frames. If it falls outside the new length, it is clamped.

### Previews are transient edit states
`preview_edits(fileId, preview | null)`, where preview is one of:
- `{ kind: "clip", clip }`
- `{ kind: "crossfade", join, xfade }`
- `{ kind: "fade", op, start, end, bend }`

The backend applies the preview to the current `EditList` exactly as the committing command would (`set_clip`, `set_crossfade`, `fade_*_range`) and calls `Player::follow` with the result. It doesn't touch history, revision, `LoadedFile.audio` or analysis. `null` follows back to the current state.

The call does nothing unless the file is the one playing. The frontend only sends it while `transport.playing` is true for that tab, throttled to one call in flight with latest-wins, like `PeakLoader`.
- **Handle drags:** `Waveform.svelte` sends previews from `pointermove` and `null` on Escape. On release, the normal `set_clip`/`set_crossfade` commits (which follows again).
- **Curve dialog:** `CurveDialog.svelte` sends `{ kind: "fade" }` on each bend change. Apply commits through `applyEdit`, and Cancel sends `null`. The dialog's waveform preview reuses the renderer's envelope pass (built for clip drags) with a temporary ramp over the selection. Peaks are not re-queried.

*Alternatives:*
- Commit every pointer move as a history entry and coalesce later: it pollutes undo and marks the tab dirty mid-drag.
- Render previews in the frontend with Web Audio: a second renderer that could drift from the Rust one.

### Frontend edit flow
- `run()` becomes:
  1. call the command (no progress channel);
  2. `showOutcome` (metadata, clip, edit state, view and selection rules as today);
  3. mark `tab.statsPending = true`;
  4. fire and forget `wait_analysis` → refetch whole stats and clip runs, clear `statsPending`, bump `tab.peaksVersion`.
- `tab.editing` remains only as an in-flight guard for one command (milliseconds), not a UI lock. Edits queue naturally because commands for one file serialize on the history lock.
- **The waveform no longer remounts on `revision`.**
  - `PeakLoader` takes a version. When it changes, `covers()` returns false but the old tile keeps being drawn until the new one arrives.
  - The overview cache is invalidated the same way.
  - The viewport is updated in place by `showOutcome`, so zoom and scroll follow the edit rules without a remount.
- **Stats panel:** shows a small "Updating…" label beside the whole-file header and dims the figures while `statsPending`.

### Saving renders, and completes the analysis
`save_file` gains `onProgress`.
1. If the current revision's analysis isn't installed, the save first runs it synchronously, reporting progress over the first part of the bar (the debounced background pass is cancelled and replaced).
2. The writer then renders, reporting frames written for the rest of the bar.
3. The installed analysis' pyramid and stats carry over to the saved file, as today.
4. Saving keeps `begin_analysis`, so it is still exclusive per file and the frontend keeps the progress box and the editing lock for Save only.

Playback still stops for Save, because on Windows the mapping must be released before overwriting.

## Risks / Trade-offs

- [Background analysis competes with playback for disk and CPU on huge files] → One worker per file, debounced and cancelled on change. The feeder keeps its 0.2 s ring buffer and reads far less data than the analysis.
- [Bounded peaks overstate levels inside long fades when zoomed out] → Specified as allowed, never understated, and exact once analysed or when zoomed in (4096-frame exact-read threshold).
- [The flush on retarget can cause a tiny gap if the feeder is slow to refill] → One 1024-frame chunk from a memory map is microseconds. The 5 ms fade-in hides the join. The engine treats an empty buffer after a flush as silence, not as end of playback.
- [Undo/redo now re-analyses states that were analysed before] → It only costs background CPU. Caching per-history-state analyses was rejected to bound memory.
- [Mapping positions through edits is subtle (trims, crossfades)] → `source_at`/`output_of` are pure functions on `EditList` with unit tests. Crossfade spans map by each segment's own frames.
- [Commands without `onProgress` are an IPC break] → The frontend and backend ship together. `docs/ipc.md` is updated in the same change.

## Migration Plan

None. State is in memory only. Rollback is reverting the change. `add-clip-fades-and-crossfades` must be archived before this change, because its `clip-fades` requirements are modified here.

## Open Questions

- The exact debounce (300 ms) and exact-read threshold (4096 frames) may be tuned after trying them on the 1 GB test file. Neither affects the specs.
