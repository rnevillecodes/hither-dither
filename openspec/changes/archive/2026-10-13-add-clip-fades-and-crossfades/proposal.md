# Proposal

## Why

Trimming and fading a recording currently means selecting a range and running a menu command, and every fade is a straight line. Shaping the start and end of a take, or smoothing a cut, takes several tries with Undo in between. DAWs like Ableton make this direct: you drag a clip's edges to trim, drag fade handles to set the fade length, and drag a point on the fade to bend its curve. A cut between two pieces of audio gets a crossfade you can shape the same way. HitherDither already has the non-destructive edit model to support this.

## What Changes

- **Clip handles on the waveform.** The whole file is treated like one Ableton clip:
  - **Trim handles** at the start and end: drag the file's start or end edge inward to trim, or back outward to restore trimmed audio.
  - **Fade handles**: drag a handle at the top of the waveform near each edge to set the fade-in and fade-out length.
  - **Bend point** on each fade line: drag it up or down to bow the curve, from slow-rising through linear to fast-rising.
- **Crossfades at delete joins.** Deleting a selection now blends the audio either side of the cut with a 10 ms crossfade instead of a hard join. The join shows the same handles: drag to set the crossfade length, and drag its bend point to shape it.
- **Adjustable until saved.** Trims, start/end fades and join crossfades are settings, not one-off edits. Dragging a handle again replaces the previous setting, and each release is one undo step. While dragging, the waveform previews the result. The audio, stats and playback update when you let go.
- **Curve editor for Edit › Fade In / Fade Out.** These open a dialog with the same bend-point curve, defaulting to linear. They are still applied once to the selection, as today.
- **BREAKING (audio):** Delete no longer makes a hard join. The few milliseconds either side of the cut are now blended, so saved files are no longer bit-exact there.

Assumptions:
- The bend is a single value: 0 is linear, positive bows the fade line upward (louder through the fade), negative bows it downward.
- New crossfades start linear (bend 0).
- A crossfade is centred on the join. It is limited by the audio available on each side, including the deleted audio and the audio before the next piece.
- Saving bakes trims, fades and crossfades into the file. Afterwards the handles start fresh, as for a newly opened file.

Non-goals: several bend points per curve; fades on arbitrary interior positions by dragging (use Edit › Fade In/Out); crossfades between different files; per-channel fades; incremental re-analysis.

## Capabilities

### New Capabilities
- `clip-fades`: on-waveform trim handles, start/end fades and join crossfades with a draggable length and bend point, their live preview, and how they combine with other edits, undo and saving.

### Modified Capabilities
- `audio-editing`: "Fade in and fade out" gains the curve dialog and bend-shaped gains. "Delete selection" now crossfades the join.
- `wav-export`: "Output format": the bit-exact guarantee excludes samples changed by fades and crossfades, including the default crossfade at delete joins.

## Impact

- **Backend (`src-tauri/src/edit/`):**
  - `EditList` gains a trim window, start/end fades and per-join crossfades.
  - `Ramp` becomes a section of a bent fade curve instead of a straight line.
  - `Audio` renders trims, fades and crossfades (crossfades read source audio beyond the join).
  - The writer encodes every changed span from a native-precision render.
- **Commands:** new `set_clip` and `set_crossfade` (documented in `docs/ipc.md`). `apply_edit` takes a `bend` for fades.
- **Frontend:**
  - Handle hit-testing, dragging and drawing in `Waveform.svelte` and the renderer (envelope overlay, live preview).
  - A new curve dialog component for Fade In/Out.
  - Wiring in `editActions.ts`.
- **Dependencies:** none new.
