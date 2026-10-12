# Design

## Context

- **Source access:** each open file is a `LoadedFile` (`src-tauri/src/state.rs`) holding an `Arc<SampleSource>` (a read-only memory map, decoded to `f32` on demand), a `PeakPyramid` and a whole-file `Analysis` (stats and clip runs). All three are built in one pass by `stats::analyse(src, …, Some(&mut PyramidBuilder))` at load.
- **Consumers:**
  - Peaks (`peaks.rs` `query`/`raw`), stats (`stats::analyse`, `StatsRunner::spawn`) and playback (`Player::play`, `engine.rs`) all take a `SampleSource`.
  - They only use `frames()`, `channels()`, `info()`, `read(start, count, out)` and `read_with_full_scale(…)`.
  - Playback refuses to play if `changed_on_disk()`, because the file is memory-mapped.
- **Frontend:**
  - Tabs (`tabs.svelte.ts`) hold `meta`, `wholeStats`, `clipRuns`, `selection`, `stats` and saved view state.
  - `Waveform` is `{#key active.id}`. `PeakLoader` caches tiles per mounted waveform, and `overviewCache` caches the overview per tab id.
  - The selection is in frames (`selection.ts`), and `view.reveal(frame)` scrolls a frame into view.
- **Saving:** there is no writer yet. The parser (`wav/parser.rs`) records `data_offset` but not where the `fmt ` chunk is.
- **Dialogs:** `tauri-plugin-dialog` is installed, but only `dialog:allow-open` is granted.

## Goals / Non-Goals

**Goals:**
- Non-destructive edits that cost O(number of edits) memory, not O(file size), so a 1 GB file can be edited as easily as a 1 MB one.
- One code path for peaks, stats and playback over original or edited audio.
- Bit-exact output for untouched samples.

**Non-Goals:**
- Incremental peak/stats updates (we re-analyse after each edit; see Risks).
- Persisting edit history across restarts.

## Decisions

### Edit model: a segment list over the source (an edit decision list)
New module `src-tauri/src/edit/`:

```text
Segment { src_start: u64, len: u64, gains: Vec<Ramp> }   // frames of the original source
Ramp    { from: u64, to: u64, g0: f32, g1: f32 }        // segment-local, linear, multiplied together
EditList { segments: Vec<Segment> }                      // the edited audio, in order
```

- A freshly opened file is one segment covering the whole source with no gains.
- **Crop** and **Delete** split segments at the selection bounds and drop the segments outside (or inside) the range.
- **Silence** splits at the bounds and marks the covered segments silent: their gains become a single `0` ramp, short-circuited on read.
- **Fade** splits at the bounds and appends a `Ramp` to each covered segment, with that segment's share of the 0→1 (or 1→0) line.
- Because ramps are multiplied, fading twice squares the gain, as the spec requires.

Each operation is a pure function `EditList -> EditList` with no I/O, so it is unit-testable with small synthetic sources.

*Alternatives:*
- *Materialise the edited audio in memory or a temp file:* simple reads, but O(file size) per edit and per undo step. That rules it out for the 1 GB files the app already supports.
- *A single gain envelope per segment:* can't represent repeated fades exactly.

### `Audio`: one read interface for original and edited audio
Introduce `Audio { source: Arc<SampleSource>, edits: Arc<EditList> }`. It exposes the same methods the consumers use (`frames`, `channels`, `info`, `read`, `read_with_full_scale`):

- `read` maps the requested frame range onto segments, decodes each piece from the source, and applies the product of ramps per frame.
- The full-scale flag is kept only for samples with unity gain. A sample scaled by g < 1 can't be at full scale.

`peaks.rs`, `stats/` and `playback/` change their parameter type from `SampleSource` to `Audio`. `LoadedFile.source` becomes `audio: Audio`.

Unedited files (a single segment, no gains) take a fast path straight to `SampleSource::read`, so analysis and playback of unedited files cost no more than today. The existing benchmarks in `docs/performance.md` are the check.

*Alternative:* a `trait Frames` with generics everywhere. It's more flexible, but `Audio` covers both cases, keeps signatures simple and avoids monomorphising the hot loops twice.

### History and re-analysis
`AppState` stores per file:

- `history: Vec<Arc<EditList>>` with a cursor `pos`.
- `saved_pos: Option<usize>`: the history index that matches the file on disk (`None` after a save once older states are dropped).
- Dirty means `pos != saved_pos`.

Each edit truncates the history after `pos`, pushes the new list and advances. Undo and redo move `pos`.

After any change of `pos`, the backend re-runs `stats::analyse` with a `PyramidBuilder` over the new `Audio`:

- It's the same code and the same progress callback as `load`, and it can be cancelled through the existing per-id cancel flag.
- The new `LoadedFile` replaces the old one atomically when analysis finishes.
- A new edit request for that file while analysis runs is rejected (the frontend disables edits meanwhile, per spec).

The history is unbounded in length; each entry is a few segments.

*Alternative:* cache a pyramid and analysis per history entry to make undo instant. That costs roughly 16 MB per entry for a 1 GB stereo file, so it was rejected for now. Re-analysis cost is listed under Risks.

### IPC
New commands, documented in `docs/ipc.md`:

- **`apply_edit(fileId, op, start, end, onProgress)`**:
  - `op` is one of `crop | delete | silence | fadeIn | fadeOut`.
  - Returns `{ metadata, editState }` after re-analysis.
  - `editState = { canUndo, canRedo, dirty, revision }`.
- **`undo_edit(fileId, onProgress)`** and **`redo_edit(fileId, onProgress)`**: same response.
- **`save_file(fileId, path | null)`**:
  - Writes the file and returns `{ metadata, editState }`.
  - Errors use readable messages (changed on disk, too large, I/O).

`revision` increases on every change of audio. The frontend uses it to invalidate cached peaks and the overview, and to re-key `<Waveform>` (`{#key \`${active.id}:${active.revision}\`}`), keeping the tab's saved viewport (clamped to the new length).

`get_peaks`, `compute_stats` and `get_clip_runs` are unchanged and always serve the current audio. Playback's existing `changed_on_disk` check stays: edits never touch the source file, so it remains valid.

### Writer
New `src-tauri/src/wav/writer.rs`:

1. Write a RIFF header, the source's `fmt ` chunk bytes verbatim, and a `data` chunk.
2. For each segment:
   - **No gains:** copy its raw bytes straight from the map (bit-exact, fast).
   - **Silent:** write zero samples. For 8-bit unsigned, zero is `0x80`.
   - **Otherwise:** decode each sample in its native domain (integer or `f32`/`f64`, not via `f32` for `f64` sources), multiply by the gain in `f64`, then encode back. Integers are rounded to nearest and clamped. Samples with fewer valid bits than the container are rounded on the valid-bit grid.

The parser gains `fmt_range: Range<u64>` so the writer can copy the chunk. If the data size would exceed `u32::MAX` minus the header, the write fails before anything is created.

### Safe save and replacing a mapped file
- Write to a temp file (`<name>.hdtmp-<random>`) in the target's directory, `fsync`, then rename it over the target. Any failure deletes the temp file and leaves the target untouched.
- **Save over the original** needs extra steps on Windows, where a memory-mapped file can't be replaced:
  1. Check `changed_on_disk()` and refuse if it's true.
  2. Stop playback if it's using this file, and cancel any running stats.
  3. Write the temp file and fsync it.
  4. Open the temp file as a new `SampleSource`.
  5. Swap the tab's `LoadedFile` so nothing references the old map, and drop it.
  6. Rename the temp file over the original. If the rename fails, keep serving from the temp file's map, delete nothing, and report the error.
  7. Re-stamp `disk_stamp`.
- After a successful save, the new `LoadedFile` reuses the current pyramid and analysis, because the audio is identical. It gets a fresh single-segment `EditList` over the new source, and the history is reset to `[that]` with `saved_pos = 0`. That's how the spec's "no reload, same stats" is met.
- **Save As** writes to the new path the same way, then swaps to the new source and path.
- The frontend rejects a Save As target that matches another open tab (`samePath`) before calling the backend.

### Frontend
- **Tab state:** `Tab` gains `dirty`, `canUndo`, `canRedo`, `revision`, `editing` (re-analysis in progress) and `overwriteConfirmed`.
- **Actions:** `editActions.ts` holds `applyEdit(op)`, `undo()`, `redo()`, `save()`, `saveAs()` and `confirmDiscard(tab)`. They stop playback, call the backend, then update `meta`, `wholeStats`, `clipRuns`, the selection (per spec) and the playhead (start for crop, join point for delete), and bump `revision`.
- **Menu:**
  - `menu.ts` gains an Edit submenu (Undo, Redo, separator, Crop to Selection, Delete, Silence, Fade In, Fade Out) and File › Save / Save As.
  - Enabled states are pushed from an `$effect` in `App.svelte`, the same way Close Tab is handled today.
  - Shortcuts: Ctrl/Cmd+Z, Ctrl/Cmd+Shift+Z, Ctrl+Y, Delete, Ctrl/Cmd+T (Crop), Ctrl/Cmd+L (Silence), Ctrl/Cmd+S, Ctrl/Cmd+Shift+S. The webview key handler gets the same double-fire guard Close Tab uses. Fade In and Fade Out have no shortcut.
- **Dialogs:**
  - Overwrite confirmation uses `message` with `buttons: { ok: "Overwrite", cancel: "Cancel" }`.
  - Save / Don't Save / Cancel uses `message` with `buttons: { yes: "Save", no: "Don't Save", cancel: "Cancel" }`.
  - Save As uses `save` with a WAV filter.
  - Capabilities add `dialog:allow-save` and `dialog:allow-message`.
- **Closing and quitting:**
  - `closeTab` becomes async and goes through `confirmDiscard`.
  - Quitting is handled by `getCurrentWindow().onCloseRequested`: prevent the close, prompt for each dirty tab, then `destroy()`. This needs `core:window:allow-close` (menu Quit calls `close()`) and `core:window:allow-destroy`.
  - The predefined Quit menu item is replaced by a custom one that triggers the same path.
- **Tab strip:** `TabBar` shows a dot in place of the close icon while the tab is dirty (it turns back into × on hover), and the tooltip adds "Unsaved changes".
- **Go to start:** `Transport.svelte` gets a button that calls `seek(0)` then `reveal(0)`, enabled whenever `app.meta` is set. It isn't tied to `playbackAvailable()`. The Home key gets the same `reveal(0)`.

## Risks / Trade-offs

- [Re-analysis after each edit or undo takes about as long as the original load (seconds for a 1 GB file)] → It shows the existing delayed progress overlay and other tabs stay usable. Crop and delete could later reuse pyramid blocks from unchanged regions; this is deferred.
- [Read path slower once a file has many segments or ramps] → Segment lookup is a binary search per read call, not per frame. Ramps are evaluated incrementally. Expected edit counts are small.
- [Replacing a memory-mapped original on Windows fails if another process has it open] → The rename error is reported and the tab keeps working from the temp copy. The user can retry or use Save As.
- [Rounding faded integer samples without dither adds quantisation distortion at very low levels] → Acceptable for trims and fades; dither is a possible later option.
- [Dropping metadata surprises users with broadcast WAVs (`bext`)] → This was the user's explicit choice. The proposal and the Save As dialog flow don't promise preservation.
- [Unbounded history] → Each entry is tiny (segments only); a cap can be added if it ever matters.
- [Session restore reopens the on-disk file, so unsaved edits are lost after an accepted "Don't Save"] → That's intended. Quitting always asks first.

## Migration Plan

No data migration. Files are only written on explicit Save or Save As. Rolling back the change removes editing; files saved meanwhile are ordinary WAVs.

## Open Questions

- Exact icon for the Go to start button (a bar plus left-pointing triangle is the default plan). This is cosmetic and can be settled during implementation.
