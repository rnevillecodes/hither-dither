# live-editing Specification

## Purpose

Makes editing feel live. Edits, undo and redo take effect immediately in the waveform and in playback, and handle drags and the fade curve can be heard as they change. Analysis runs in the background, and the edited audio is only rendered when it is exported.

## Requirements

### Requirement: Edits apply immediately
An edit, undo, redo or handle release SHALL update the waveform, overview, ruler, length, frame count, handles and playhead without waiting for the edited audio to be analysed and without a progress indicator. The tab SHALL stay editable straight away, so the next edit can be made before earlier ones have been analysed.

#### Scenario: Delete on a long file
- **WHEN** a 1-hour file is open and the user deletes 10:00–20:00
- **THEN** the waveform and duration show the 50-minute result straight away, with no progress box

#### Scenario: Edit again straight away
- **WHEN** the user applies Silence and immediately applies Fade Out to another selection
- **THEN** both edits are applied, and Undo reverts the fade first

#### Scenario: Undo is instant
- **WHEN** the user presses Undo repeatedly on a long file
- **THEN** each step shows its result straight away, with no progress box

### Requirement: Waveform drawn from the edits
Immediately after a change, the waveform and overview SHALL show the edited audio. Silenced audio SHALL be flat, removed audio gone, and faded audio scaled. Where the gain varies within one pixel column, the drawn peak MAY be larger than the true peak, but SHALL NEVER be smaller. Once the background analysis of that state finishes, the drawing SHALL be exact.

#### Scenario: No peak hidden
- **WHEN** a short loud transient sits inside a long fade-out and the view is zoomed out to the whole file
- **THEN** the transient is visible at no less than its faded height

#### Scenario: Exact when zoomed in
- **WHEN** the user zooms in until each pixel column covers one sample or fewer
- **THEN** the drawn samples are exactly the edited samples

#### Scenario: Old view kept until redrawn
- **WHEN** the user makes an edit
- **THEN** the waveform does not blank or flicker while the new peaks are fetched; the zoom and scroll position stay as the edit rules say

### Requirement: Background analysis
After a change, the system SHALL analyse the edited audio in the background to update whole-file statistics, clip markers and exact peaks. The analysis SHALL start shortly after the last change, and a further change SHALL cancel and restart it. The analysis SHALL NOT block editing, playback, selection, zooming or switching tabs. Selection statistics SHALL always describe the current edited audio.

#### Scenario: Rapid changes
- **WHEN** the user makes five edits in quick succession
- **THEN** whole-file statistics are computed once, for the final state

#### Scenario: Selection stats stay current
- **WHEN** the user silences part of a selection while its whole-file analysis is still pending
- **THEN** the selection statistics reflect the silenced audio

### Requirement: Pending statistics are marked
While the whole-file statistics are older than the current edits, the statistics panel SHALL mark them as updating, and SHALL NOT present them as describing the current audio. When the analysis finishes, the panel SHALL show the new figures and clear the mark. Clip markers SHALL immediately disappear from audio that was removed, silenced or gain-changed, and SHALL be exact once the analysis finishes.

#### Scenario: Stats updating
- **WHEN** the user edits a long file and looks at the whole-file statistics before the analysis finishes
- **THEN** the panel shows that the statistics are updating, and shows the new figures when it finishes

#### Scenario: Clip markers removed at once
- **WHEN** the user silences a clipped region
- **THEN** that region's clip markers disappear straight away

### Requirement: Playback follows edits
An edit, undo, redo or handle release on the tab that is playing SHALL NOT stop playback. Within 0.25 s, the playing audio SHALL switch to the edited audio, without a click. Playback SHALL continue from the same point in the audio. If that point was removed, playback SHALL continue from the join that replaced it, or stop if no audio after it remains. While playing, the playhead SHALL follow playback, overriding where the edit would place it when stopped.

#### Scenario: Fade while listening
- **WHEN** the file is playing at 3.0 s and the user applies Fade Out to 5.0–8.0 s
- **THEN** playback continues without interruption, and the fade is heard when playback reaches 5.0 s

#### Scenario: Delete before the playhead
- **WHEN** the file is playing at 30.0 s and the user deletes 10.0–20.0 s
- **THEN** the same audio keeps playing without a jump in sound, and the playhead moves to 20.0 s

#### Scenario: Delete the audio being played
- **WHEN** the file is playing at 15.0 s and the user deletes 10.0–20.0 s
- **THEN** playback continues from the join at 10.0 s

#### Scenario: Looping selection
- **WHEN** a selection is looping and the user applies Silence to part of it
- **THEN** the loop keeps playing the same range, with the silenced part now silent

#### Scenario: Selection cleared by the edit
- **WHEN** a selection is playing and an edit clears the selection
- **THEN** playback continues to the end of the file

### Requirement: Hear handle drags while playing
While the file is playing and the user drags a trim, fade, crossfade or bend handle, the playback SHALL reflect the dragged setting within 0.25 s of each pointer movement, before the handle is released. Pressing Escape during the drag SHALL return playback to the previous setting. Preview changes alone SHALL NOT add undo steps or mark the tab as unsaved.

#### Scenario: Shape a fade by ear
- **WHEN** a selection near the end is looping and the user drags the end fade handle inward
- **THEN** the fade becomes audible while dragging, and becomes one undo step on release

#### Scenario: Cancel a heard drag
- **WHEN** the user drags a crossfade handle during playback and presses Escape
- **THEN** playback sounds as it did before the drag, and Undo is unchanged

### Requirement: Preview the fade curve dialog
While the Fade In / Fade Out curve dialog is open, the waveform SHALL show the selection faded with the dialog's current curve. If the file is playing, playback SHALL use that curve within 0.25 s of each change. Apply SHALL commit the fade as one undo step. Cancel or Escape SHALL restore the waveform and playback with nothing added to the history.

#### Scenario: Audition a curve
- **WHEN** the selection is looping and the user opens Fade Out and drags the bend point
- **THEN** the waveform under the dialog and the playing audio follow the bend

#### Scenario: Cancel the preview
- **WHEN** the user opens Fade In, drags the bend point and chooses Cancel
- **THEN** the waveform and playback are as before, and Undo is unchanged

### Requirement: Audio rendered only when saving
Edits, undo, redo, previews and saving a session SHALL NOT write audio to disk or create rendered copies. The edited audio SHALL be rendered only by File › Export….

#### Scenario: Many edits, no files
- **WHEN** the user makes many edits to `a.wav` and saves its session
- **THEN** the only new file is the session file, and `a.wav` is unchanged on disk

#### Scenario: Closing never renders
- **WHEN** the user closes a tab with unsaved edits and chooses Save
- **THEN** only the session file is written; no WAV is rendered
