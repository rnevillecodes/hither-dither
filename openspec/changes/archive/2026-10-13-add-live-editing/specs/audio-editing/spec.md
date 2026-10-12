# Spec Delta

## MODIFIED Requirements

### Requirement: The app shows the edited audio
After each edit, undo or redo, the waveform, overview, ruler, duration, frame count, selection statistics and playback SHALL reflect the edited audio immediately, as described in the live-editing capability. The tab SHALL NOT show a progress indicator for edits, undo or redo, and SHALL stay editable. Whole-file statistics and clip markers SHALL be brought up to date by a background analysis and marked as updating until it finishes. Other tabs SHALL stay usable meanwhile.

#### Scenario: Clipping removed
- **WHEN** a file has clipped samples, the user selects the clipped region and chooses Silence
- **THEN** the clip markers in that region disappear, and once the whole-file statistics have updated they no longer count those clipped samples

#### Scenario: Playback of an edited file
- **WHEN** the user deletes 4.0–5.0 s and plays from 3.5 s
- **THEN** playback goes from the audio that was before 4.0 s to the audio that was after 5.0 s, blended by the join's crossfade, with no gap

#### Scenario: No wait after an edit
- **WHEN** the user crops a long file
- **THEN** the cropped waveform is shown with no progress box, and Edit menu items are available straight away

## REMOVED Requirements

### Requirement: Editing stops playback
**Reason**: Edits now apply while playing. Stopping playback for every change got in the way of shaping audio by ear.
**Migration**: Playback continues through edits, undo and redo, and switches to the edited audio (see "Playback follows edits" in the live-editing capability). Users who want silence while editing press Stop first.
