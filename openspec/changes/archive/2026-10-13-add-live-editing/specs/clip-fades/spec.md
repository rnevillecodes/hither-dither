# Spec Delta

## MODIFIED Requirements

### Requirement: Live preview while dragging
While the user drags a trim, fade, crossfade or bend handle, the waveform SHALL immediately show the result: the gain envelope as a line over the waveform, the waveform drawn scaled by that envelope, and trimmed audio dimmed. The new length, or the fade time, SHALL be shown next to the pointer. If the file is playing, the dragged setting SHALL also be heard, as described in the live-editing capability. Releasing SHALL apply the change; pressing Escape before releasing SHALL cancel it.

#### Scenario: Preview a fade
- **WHEN** the user is dragging the end fade handle
- **THEN** the fade line and the tapered waveform follow the pointer, and a label shows the fade length

#### Scenario: Cancel a drag
- **WHEN** the user drags a trim handle and presses Escape before releasing
- **THEN** the audio, the handles and the undo history are unchanged

#### Scenario: Hear a trim while playing
- **WHEN** the file is playing to the end at 5.0 s and the user drags the end trim handle from 10.0 s to 7.0 s without releasing
- **THEN** playback ends at the audio that is at 7.0 s

### Requirement: Clip changes are undoable settings
Each released handle drag SHALL be one entry in the undo history. Undo and Redo SHALL restore the previous or next trim, fade or crossfade setting. Adjusting a handle again SHALL replace its setting rather than add another fade. Clip changes SHALL mark the tab as having unsaved changes and SHALL take effect immediately, like other edits, without stopping playback.

#### Scenario: Adjust twice, undo once
- **WHEN** the user sets a 0.5 s fade-in, then drags it to 1.0 s, then presses Undo
- **THEN** the fade-in is 0.5 s again, with the same curve as before

#### Scenario: Release during playback
- **WHEN** the file is playing and the user releases a fade-out handle
- **THEN** playback continues, and the handles and waveform show the new fade with no progress box
