# Spec Delta

## Purpose

Lets users trim and clean up a WAV file in place, using the waveform selection as the target for crop, delete, silence and fade operations, with undo and redo. The rest of the app always shows the edited audio.

## ADDED Requirements

### Requirement: Edit operations need a selection
Crop to Selection, Delete, Silence, Fade In and Fade Out SHALL act on the active tab's current time selection across all channels. Each SHALL be available from the Edit menu only while the active tab has finished loading and has a non-empty selection; otherwise the items SHALL be disabled and their shortcuts SHALL do nothing.

#### Scenario: No selection
- **WHEN** a file is open with no selection
- **THEN** Crop to Selection, Delete, Silence, Fade In and Fade Out are greyed out in the Edit menu

#### Scenario: Nothing open
- **WHEN** no file is open
- **THEN** every Edit menu item is greyed out

### Requirement: Crop to selection
Crop to Selection SHALL keep only the selected range, removing the audio before and after it. Afterwards the file SHALL start at what was the selection start, the selection SHALL be cleared, the view SHALL zoom to fit and the playhead SHALL be at the start. It SHALL be disabled when the selection already covers the whole file.

#### Scenario: Trim leading and trailing silence
- **WHEN** a 10.0 s file has 2.0–7.5 s selected and the user chooses Edit › Crop to Selection
- **THEN** the file is 5.5 s long, its first sample is the one that was at 2.0 s, and there is no selection

### Requirement: Delete selection
Delete SHALL remove the selected range and join the audio before and after it. Afterwards the selection SHALL be cleared and the playhead SHALL be placed at the join. Delete SHALL be disabled when the selection covers the whole file, because a file can't be left with no audio.

#### Scenario: Remove a section
- **WHEN** a 10.0 s file has 4.0–5.0 s selected and the user presses Delete
- **THEN** the file is 9.0 s long, the audio that was at 5.0 s now plays at 4.0 s, and the playhead is at 4.0 s

#### Scenario: Whole file selected
- **WHEN** the whole file is selected
- **THEN** Delete is greyed out and pressing the Delete key changes nothing

### Requirement: Silence selection
Silence SHALL set every sample in the selected range to digital silence (zero), keeping the file length and the selection unchanged.

#### Scenario: Silence a click
- **WHEN** the user selects a 5 ms click and chooses Edit › Silence
- **THEN** the waveform is flat across that range, the length is unchanged, and the stats for the selection report silence (peak −∞ dBFS)

### Requirement: Fade in and fade out
Fade In SHALL scale the selected range by a gain rising linearly from 0 at the selection start to 1 at its end. Fade Out SHALL do the reverse. Audio outside the range SHALL be unchanged, and the length and selection SHALL stay the same. Fading a range that was already faded or silenced SHALL multiply the gains.

#### Scenario: Fade out the ending
- **WHEN** the last 2.0 s of a file are selected and the user chooses Edit › Fade Out
- **THEN** the waveform tapers to silence at the end of the file and the audio before the selection is unchanged

#### Scenario: Fade twice
- **WHEN** the user applies Fade Out to a range and then Fade Out again to the same range
- **THEN** each sample in the range is scaled by the square of the single-fade gain

### Requirement: The app shows the edited audio
After each edit, undo or redo, the waveform, overview, ruler, duration, frame count, whole-file and selection statistics, clip markers and playback SHALL all reflect the edited audio. While the edited audio is being re-analysed, the tab SHALL show the same delayed progress indicator as a load, and further edits on that tab SHALL be disabled until it finishes. Other tabs SHALL stay usable meanwhile.

#### Scenario: Clipping removed
- **WHEN** a file has clipped samples, the user selects the clipped region and chooses Silence
- **THEN** the clip markers in that region disappear and the whole-file stats no longer count those clipped samples

#### Scenario: Playback of an edited file
- **WHEN** the user deletes 4.0–5.0 s and plays from 3.5 s
- **THEN** playback goes straight from the audio that was at 4.0 s to the audio that was at 5.0 s, with no gap

### Requirement: Editing stops playback
Applying an edit, undo or redo to the tab that is playing SHALL stop playback first.

#### Scenario: Edit during playback
- **WHEN** a file is playing and the user applies Fade In to a selection
- **THEN** playback stops and the fade is applied

### Requirement: Undo and redo
The system SHALL keep a per-tab history of edits since the file was opened or last saved. Undo (Ctrl/Cmd+Z, Edit › Undo) SHALL revert the most recent edit, and Redo (Ctrl/Cmd+Shift+Z, Ctrl+Y, Edit › Redo) SHALL reapply the most recently undone edit. Making a new edit after undoing SHALL discard the redo history. Undo and Redo SHALL be disabled when there is nothing to undo or redo.

#### Scenario: Undo a delete
- **WHEN** the user deletes 4.0–5.0 s of a 10.0 s file and presses Ctrl/Cmd+Z
- **THEN** the file is 10.0 s long again with the original audio, and Redo is enabled

#### Scenario: Several undos
- **WHEN** the user applies Crop, then Silence, then Fade In, and presses Undo three times
- **THEN** the audio is identical to the file as opened, and Undo is greyed out

#### Scenario: New edit clears redo
- **WHEN** the user undoes an edit and then applies a different edit
- **THEN** Redo is greyed out

#### Scenario: History is per tab
- **WHEN** the user edits `a.wav`, switches to `b.wav` and presses Ctrl/Cmd+Z
- **THEN** nothing in either file changes, because `b.wav` has no edits

### Requirement: Selection after undo and redo
After an undo or redo, the selection SHALL be cleared if the file length changed, and kept otherwise.

#### Scenario: Undo a fade
- **WHEN** the user applies Fade In to 1.0–2.0 s and presses Undo
- **THEN** the fade is gone and 1.0–2.0 s is still selected

### Requirement: Original file untouched until saved
Edits, undo and redo SHALL NOT modify the file on disk. Only Save SHALL change the original file.

#### Scenario: Close without saving
- **WHEN** the user crops a file, closes its tab and chooses Discard, then reopens the file
- **THEN** the file has its original length and content
