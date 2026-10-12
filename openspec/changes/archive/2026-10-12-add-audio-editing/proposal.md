# Proposal

## Why

HitherDither can show exactly where a recording has a problem, such as leading silence, a click or a clipped section. But to fix it the user has to switch to another editor. Basic trimming and clean-up are the natural next step after inspecting a file, and the waveform selection the user already makes for statistics is the right input for them.

## What Changes

- **Edit operations on the waveform selection:**
  - **Crop to selection**: keep only the selected range.
  - **Delete selection**: remove the range and join the audio on either side.
  - **Silence selection**: replace the range with digital silence; the length is unchanged.
  - **Fade in / Fade out**: apply a linear gain ramp from silence to the original level (or the reverse) across the range.
- **Multi-level undo / redo** (Ctrl/Cmd+Z, Ctrl/Cmd+Shift+Z and Ctrl+Y) for every edit since the file was opened or last saved.
- **Edits are non-destructive until saved.** The original file is not touched while editing. The waveform, statistics, clip markers and playback all reflect the edited audio.
- **Save and Save As.** Save overwrites the original file, asking for confirmation the first time for each file. Save As writes a new WAV file and the tab then refers to it. Saved files use the source's sample format, sample rate and channel layout, and contain only the `fmt ` and `data` chunks.
- **Unsaved-changes protection.** A tab with unsaved edits is marked in the tab strip. Closing that tab or quitting the app asks whether to save, discard or cancel.
- **New Edit menu** with Undo, Redo, Crop to Selection, Delete, Silence, Fade In and Fade Out. File gains Save and Save As.
- **Go to start button** in the transport, before Play. It does the same as the Home key and scrolls the view back to the start if needed. This is handy after cropping or trimming.

Assumptions:
- Edits act on all channels together; there are no per-channel edits.
- Fades are linear in amplitude.
- Samples that an edit didn't change are written back bit-for-bit. Changed samples in integer formats are rounded to the nearest value without dither.
- Saved files never contain metadata chunks (LIST, bext, cue, smpl and so on) from the source, even when the source had them.

Non-goals: copy/paste and insert; gain, normalise or other effects; per-channel edits; changing the sample format, rate or bit depth on save; RF64/W64 output for results over 4 GiB; editing while a file is still loading.

## Capabilities

### New Capabilities
- `audio-editing`: the edit operations on a selection, undo/redo, and how the waveform, statistics, selection and playback behave after an edit.
- `wav-export`: writing edited audio to disk (Save, Save As, output format) and protecting unsaved edits when tabs close or the app quits.
- `transport-controls`: transport buttons that move the playhead; starts with Go to start.

### Modified Capabilities
<!-- None: openspec/specs/ has no published specs yet; earlier changes (wav-import, file-tabs, audio-playback, …) are not archived. -->

## Impact

- **Backend (`src-tauri/src/`):**
  - New edit model: a list of segments over the memory-mapped source, each with optional gain ramps, plus an undo history.
  - Peaks, stats and playback read through the edited view instead of `SampleSource` directly.
  - New WAV writer.
  - New commands: `apply_edit`, `undo_edit`, `redo_edit` and `save_file`.
  - `AppState` keeps the edit history per file.
  - `docs/ipc.md` gains the new commands.
- **Frontend (`src/`):**
  - Edit menu and Save/Save As items in `menu.ts`, and the matching shortcuts in `App.svelte`.
  - Tabs track an unsaved flag and an audio revision, so the waveform, overview and stats refresh after each edit.
  - The tab strip shows an unsaved marker.
  - `Transport.svelte` gains the Go to start button.
  - Prompts appear when closing a tab or quitting with unsaved edits.
- **Tauri:** the dialog plugin needs `save`, `ask` and `message` permissions. The window close request and the menu Quit item must go through the unsaved-changes check.
- **Dependencies:** none new.
