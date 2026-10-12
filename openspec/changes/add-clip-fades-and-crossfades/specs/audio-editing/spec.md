# Spec Delta

## MODIFIED Requirements

### Requirement: Delete selection
Delete SHALL remove the selected range and join the audio before and after it, with a crossfade centred on the join as described for joins in the clip-fades capability. Afterwards the selection SHALL be cleared and the playhead SHALL be placed at the join. Delete SHALL be disabled when the selection covers the whole file, because a file can't be left with no audio.

#### Scenario: Remove a section
- **WHEN** a 10.0 s file has 4.0–5.0 s selected and the user presses Delete
- **THEN** the file is 9.0 s long, the audio that was at 5.0 s now plays at 4.0 s, and the playhead is at 4.0 s

#### Scenario: Whole file selected
- **WHEN** the whole file is selected
- **THEN** Delete is greyed out and pressing the Delete key changes nothing

#### Scenario: Join is crossfaded
- **WHEN** the user deletes 4.0–5.0 s of a 10.0 s file
- **THEN** a crossfade marker appears at 4.0 s, and the audio within 5 ms of the join blends the two sides instead of switching abruptly

### Requirement: Fade in and fade out
Fade In and Fade Out SHALL open a dialog showing the fade curve with a draggable bend point, starting linear. Choosing Apply SHALL scale the selected range by a gain that rises along the chosen curve from 0 at the selection start to 1 at its end (Fade In), or falls along it from 1 to 0 (Fade Out); Cancel SHALL change nothing. Audio outside the range SHALL be unchanged, and the length and selection SHALL stay the same. Fading a range that was already faded or silenced SHALL multiply the gains.

#### Scenario: Fade out the ending
- **WHEN** the last 2.0 s of a file are selected, the user chooses Edit › Fade Out and applies it without moving the bend point
- **THEN** the waveform tapers linearly to silence at the end of the file and the audio before the selection is unchanged

#### Scenario: Fade twice
- **WHEN** the user applies a linear Fade Out to a range and then the same Fade Out again to the same range
- **THEN** each sample in the range is scaled by the square of the single-fade gain

#### Scenario: Curved fade in
- **WHEN** the user chooses Edit › Fade In, drags the bend point fully upward and applies it
- **THEN** the gain rises quickly at first and levels off towards full level, matching the curve shown in the dialog

#### Scenario: Cancel the dialog
- **WHEN** the user opens Fade Out and chooses Cancel
- **THEN** the audio and the undo history are unchanged
