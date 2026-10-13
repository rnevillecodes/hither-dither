# Spec Delta

## MODIFIED Requirements

### Requirement: Output format
Saved files SHALL be RIFF WAVE files containing exactly a `fmt ` chunk identical to the source's and a `data` chunk, with no other chunks. Sample format, bit depth, valid bits, sample rate, channel count and channel mask SHALL match the source. When the tab has no active effects, samples that no edit, fade or crossfade changed SHALL be written bit-for-bit as in the source. When it has active effects, every sample SHALL be the effects' output, aligned with the input so that the effects' processing delay is removed, and the length SHALL equal the edited length. Changed integer samples, including blended crossfade samples and processed samples, SHALL be rounded to the nearest value and clamped to the format's range.

#### Scenario: Untouched audio is bit-exact
- **WHEN** the user deletes 1.0–2.0 s from a 24-bit file and saves it
- **THEN** every sample of the saved file outside the crossfade at the join equals the source sample at the corresponding position

#### Scenario: Hard join is bit-exact
- **WHEN** the user deletes 1.0–2.0 s from a 24-bit file, drags the join's crossfade to zero and saves it
- **THEN** every sample of the saved file equals the source sample at the corresponding position

#### Scenario: Metadata is not carried over
- **WHEN** the source has `LIST` and `bext` chunks and the user saves an edit
- **THEN** the saved file has only `fmt ` and `data` chunks, and HitherDither reopens it with the same format details

#### Scenario: Result too large for WAV
- **WHEN** the edited audio would make the `data` chunk larger than 4 GiB minus the header
- **THEN** Save and Save As fail with a notice saying the result is too large, and nothing is written

#### Scenario: Effects applied with latency removed
- **WHEN** a 10.0 s 16-bit file has a +6 dB gain effect that reports 64 samples of latency, and the user saves it
- **THEN** the saved file is 10.0 s long, and each sample is the source sample at the same position doubled, rounded and clamped

#### Scenario: Bypassed chain is bit-exact
- **WHEN** a tab has effects but Bypass All is on, and the user saves it with Save As
- **THEN** the saved file's samples equal the source's bit-for-bit

### Requirement: After saving
Saving SHALL render the edited audio through the tab's active effects, first finishing any pending background analysis. Meanwhile the tab SHALL show the delayed progress indicator used for loads, covering effects rendering, and edits on it SHALL be disabled. After a successful Save or Save As, the tab SHALL have no unsaved marker, an empty undo/redo history and an empty chain with its editors closed. Its view, selection and playhead SHALL be unchanged. If effects were applied, the waveform and statistics SHALL show the saved audio. Statistics, clip markers and waveform SHALL be exact, not marked as updating.

#### Scenario: Keep working after save
- **WHEN** the user zooms in, selects 1.0–2.0 s, applies Fade In and saves
- **THEN** the zoom and the 1.0–2.0 s selection are unchanged, and Undo is greyed out

#### Scenario: Save before the analysis finishes
- **WHEN** the user edits a long file and saves it straight away, while the whole-file statistics are still updating
- **THEN** the progress indicator appears if the save takes a moment, and afterwards the statistics are up to date and no longer marked as updating

#### Scenario: Progress while rendering
- **WHEN** the user saves a long edited file
- **THEN** the delayed progress indicator shows how far the save has got, and other tabs stay usable

#### Scenario: Effects are baked in
- **WHEN** a tab has an open EQ editor and an active compressor, and the user saves
- **THEN** the EQ editor closes, the Effects section is empty, and the waveform and statistics now describe the processed audio

#### Scenario: Save with effects fails
- **WHEN** a save with effects fails because the folder is read-only
- **THEN** the chain, its settings and its open editors are kept, along with the edits

## ADDED Requirements

### Requirement: Clamped samples are reported
When saving to an integer format clamps any processed samples, the system SHALL show a notice after the save saying how many samples were clamped. Float formats SHALL NOT be clamped.

#### Scenario: Too much gain
- **WHEN** a 16-bit file peaking at −3 dBFS is saved with a +6 dB gain effect
- **THEN** the file is saved, and a notice says that some samples were clipped to full scale and how many

#### Scenario: Float file keeps overs
- **WHEN** the same file in 32-bit float format is saved with the same effect
- **THEN** samples above full scale are written unclamped and no notice is shown
