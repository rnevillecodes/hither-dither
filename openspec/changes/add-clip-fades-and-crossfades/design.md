# Design

## Context

- **Edit model** (`src-tauri/src/edit/`):
  - `EditList` is an ordered list of `Segment { src_start, len, silent, ramps: Vec<Ramp { g0, g1 }> }` over the memory-mapped source.
  - Each operation (crop/delete/silence/fade) returns a new list.
  - `Audio` reads through the list (fast path when it's the identity).
  - History is a `Vec<Arc<EditList>>` in `AppState`, and every change is fully re-analysed (pyramid, stats, clip runs).
- **Writer** (`wav/writer.rs`): copies plain segments' raw bytes, scales ramped segments sample by sample in their native domain, and writes silence directly.
- **Frontend:**
  - `Waveform.svelte` owns pointer handling: drag to select, click to seek, ruler and overview.
  - `renderer.ts` draws lanes, the selection, clip markers and the playhead on canvases.
  - Edits go through `editActions.ts` (`run` → backend → `showOutcome`, which bumps `revision` and remounts the waveform).
- **Specs:** see `specs/clip-fades`, plus the MODIFIED `audio-editing` (Delete, Fade In/Out) and `wav-export` (Output format) requirements.

## Goals / Non-Goals

**Goals:**
- One curve definition shared by menu fades, clip fades and crossfades, in Rust and TypeScript, so the preview matches the audio.
- Trims, clip fades and crossfades as replaceable settings inside `EditList`, so undo/redo and saving need no new mechanism.
- Interactive dragging with no backend round trip until release.

**Non-Goals:**
- Live audio preview while dragging (the visual preview only).
- Reusing the previous analysis for small changes (every release still re-analyses; see Risks).

## Decisions

### Curve: a power curve with one bend value
`Curve { bend: f32 }` with bend in [−1, 1]:
- fade-in gain at position t ∈ [0, 1] is `t^p`;
- fade-out gain is `(1 − t)^p`;
- `p = 4^(−bend)`, so bend 0 is linear, +1 gives p = ¼ (bowed up, louder) and −1 gives p = 4 (bowed down).

It is a cheap, monotonic, single-parameter family that covers log-like to exp-like shapes and maps cleanly onto one vertical drag. The bend point is drawn at t = ½, at height `0.5^p`, and dragging it solves for `p` from the pointer's height.

The same formula lives in `edit/curve.rs` and `src/lib/curve.ts`. A shared test vector (gain at t = 0, ¼, ½, ¾, 1 for bends −1, −0.5, 0, 0.5, 1) is checked in both test suites.

*Alternatives:*
- Equal-power sine/cosine presets: only one shape, not continuously editable.
- Bezier: more than one parameter; the user chose a single bend point.

### `Ramp` becomes a section of a curve
`Ramp { t0: f64, t1: f64, dir: In | Out, curve: Curve }`. Segment-local frame i of a segment of length L has gain `curve.gain(dir, t0 + (t1 − t0) · i / L)`.

Splitting a segment splits `[t0, t1]` proportionally, so a curved fade survives later cuts exactly. The old linear `{g0, g1}` fades are the bend-0 case, and `fade_in`/`fade_out` gain a `curve` argument. Gains still multiply, so "fade twice squares the gain" holds.

### Clip settings live in `EditList`
```text
EditList {
  source_frames, segments,
  trim: Range<u64>,                    // window over the untrimmed timeline (segments' concatenation)
  fade_in: ClipFade, fade_out: ClipFade, // ClipFade { len: u64, curve: Curve }; len 0 = none
}
Segment { …, xfade: Option<Crossfade> }  // crossfade with the previous segment; Crossfade { len: u64, curve: Curve }
```

- **Output:** output frames = `trim.len()`. Output frame f reads untrimmed frame `trim.start + f`.
  - The fade-in gain applies over output `[0, fade_in.len)`.
  - The fade-out gain applies over `[frames − fade_out.len, frames)`.
- **Setting operations** (`set_trim`, `set_clip_fades`, `set_crossfade(join, …)`) return a new list with the setting replaced and clamped, as the spec's limits require:
  - At least 1 frame.
  - `fade_in.len + fade_out.len ≤ frames`.
  - A crossfade no longer than the available audio.
- **Interaction with segment edits:**
  - Menu edits take output positions and translate them by `trim.start`.
  - Delete keeps the trim window valid by taking the deleted length off `trim.end`. Silence and Fade don't change the length.
  - Crop replaces the segments with the cropped range and resets `trim` to the whole new list. That's why trimmed audio is only recoverable "until saved or cropped".
  - Clip fades are attached to the output ends, so they follow trims automatically and are clamped if the audio gets shorter.
- **Delete** sets `xfade = Some(Crossfade { len: 10 ms in frames, curve: linear })` on the segment after the new join, clamped by `max_crossfade`.
- **Joins:** segment boundaries where `xfade.is_some()` (only Delete creates them). Joins outside the trim window are not shown.

*Alternatives:*
- Bake trims into segments: dragging back out would then need Undo, which Ableton-like handles shouldn't.
- Store clip settings outside `EditList`, in the history entry: undo/redo and saving would need a second code path.

### Crossfade rendering
A join between segment A (ending at source frame `a_end`) and segment B (starting at `b_start`) with crossfade length L covers output `[J − L/2, J + L/2)`, where J is the join's output position. For an output offset d within it, with t = (d + L/2) / L:

`out = A_ext(d) · gain(Out, t) + B_ext(d) · gain(In, t)`

- `A_ext` continues A past its end with source frames `a_end + d` for d ≥ 0, so it plays into the deleted audio. For d < 0 it is A's own audio.
- `B_ext` plays B's own audio for d ≥ 0, and source frames `b_start + d` before B starts for d < 0.
- Each side keeps its segment's own gain (ramps or silence) from its boundary frame.
- `max_crossfade` is limited by:
  - the source frames available past `a_end` and before `b_start`;
  - the lengths of A and B, so a crossfade never reaches another join.

### Rendering in `Audio` and the writer
- `Audio::read` gains three stages: it maps through the trim window, renders crossfade spans by mixing the two sides as above, and multiplies by the clip fade gains.
- A new `Audio::render_native(start, count, &mut Vec<f64>)` produces the same samples in the source's native units:
  - integer values for PCM;
  - f64 for float (so `f64` sources keep their precision).
- `Audio::plain_spans()` yields output ranges whose samples are an unmodified copy of a contiguous source range.
- The writer copies those raw and encodes every other range from `render_native`, rounding and clamping integers, on the valid-bit grid as before.
- Full-scale flags are kept only for samples that are plain copies.

### IPC
- `set_clip(fileId, { trimStart, trimEnd, fadeIn: { len, bend }, fadeOut: { len, bend } }, onProgress)` replaces the clip settings. Trim positions are frames of the untrimmed timeline.
- `set_crossfade(fileId, join, { len, bend }, onProgress)` sets one crossfade, where `join` is the index of the segment after the join.
- Both are history entries like `apply_edit`, return `EditOutcome`, and stop playback.
- `apply_edit` gains an optional `bend` (default 0) used by `fadeIn`/`fadeOut`.
- `get_clip(fileId)` returns the data the handles need:
  - the untrimmed length;
  - the trim window;
  - the clip fades;
  - for each visible join: its output position, crossfade and `maxLen`.

  `EditOutcome` includes it too, so the frontend needn't call it after an edit.

### Frontend: handles, drag and preview
- **Hit-testing:** a new `src/lib/clipHandles.ts` (pure, unit-tested) computes handle positions from the clip data and viewport, and hit-tests within 6 px.
  - The trim handles are the full-height left/right edges of the audio.
  - Each fade handle is a small square at the top, at its fade length.
  - Each bend point is a circle on its fade line at t = ½.
  - Crossfade markers sit at joins, with length handles at J ± L/2 and a bend point.
- **Pointer handling:** `Waveform.svelte` checks for a handle first on `pointerdown`. If none is hit, it falls back to the existing selection logic.
  - While dragging, a local `preview` clip state drives drawing. Escape restores it.
  - On release the action calls `setClip`/`setCrossfade` through the existing `run()` path, so progress, errors, undo state and remount are shared.
  - Double-click on a bend point sends bend 0.
- **Drawing:** `renderer.ts` gains an envelope pass.
  - It shades the trimmed regions.
  - It draws the gain envelope as a line, using `curve.ts`.
  - It scales each column's min/max by the envelope gain at that column. This is exact for fades. For crossfades it is an approximation that is drawn only while dragging; after release the re-analysed peaks are exact.
  - Handles are drawn on top.
  - A small label near the pointer shows the length or fade time.
- **Curve dialog:** `CurveDialog.svelte` is a modal with a 160×100 graph of the curve and a draggable bend point, plus Apply and Cancel. Edit › Fade In/Out open it, then call `applyEdit(op, bend)`.

## Risks / Trade-offs

- [Every handle release re-analyses the whole file, which takes seconds for 1 GB] → The preview is instant and the existing delayed progress box covers the commit. Incremental re-analysis of just the changed ranges is the obvious follow-up and is deliberately deferred.
- [Default crossfades change Delete's output (no longer bit-exact near joins)] → This is spec'd as BREAKING in the proposal. Dragging a crossfade to 0 restores a bit-exact hard join.
- [The approximate crossfade preview (scaled peaks) can look slightly different from the final waveform] → This only shows while dragging. Fade previews are exact.
- [Hit-testing conflicts with selection near the edges] → The hit radius is small (6 px), the cursor changes over handles, and handles are only active where drawn.
- [The curve formula is duplicated in Rust and TS] → A shared test vector in both suites keeps them in sync.
- [`EditList` grows more complex (trim translation in every operation)] → Segment operations stay in untrimmed coordinates internally. Translation happens in one place, at the public operation entry points, and property tests compare `Audio::read` with a straightforward reference renderer.

## Migration Plan

None. History is in memory only, and files open with an empty clip state. Saving bakes everything into an ordinary WAV.
