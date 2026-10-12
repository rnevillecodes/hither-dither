# clip-fades Specification

## Purpose

Lets users trim and shape a file directly on the waveform, the way clip edges work in a DAW. The start and end can be dragged to trim, fades at each end have a draggable length and curve, and joins left by Delete get an adjustable crossfade. All of these stay adjustable until the file is saved.

## Requirements

### Requirement: Fade curve shape
Every fade and crossfade SHALL have a bend value from −1 to 1. At 0 the gain changes linearly. Positive values bow the fade line upward, so the audio stays louder through the fade; negative values bow it downward. A fade-in SHALL rise from silence at its start to full level at its end; a fade-out SHALL do the reverse. Each fade's bend SHALL be shown as a draggable point on its fade line.

#### Scenario: Linear by default
- **WHEN** the user creates a fade without touching its bend point
- **THEN** its gain changes linearly from start to end

#### Scenario: Bend upward
- **WHEN** the user drags a fade-out's bend point upward to its limit
- **THEN** the fade line bows upward and the audio halfway through the fade is louder than with a linear fade

#### Scenario: Reset the bend
- **WHEN** the user double-clicks a bend point
- **THEN** that fade's curve returns to linear

### Requirement: Trim handles
While a loaded file is shown, the waveform SHALL show a trim handle at the start and at the end of the audio. Dragging the start handle right, or the end handle left, SHALL trim audio from that end. Dragging back outward SHALL restore trimmed audio, up to the full length before trimming. The file SHALL always keep at least one frame. Releasing a handle SHALL change the audio's length accordingly. Trimmed audio SHALL stay recoverable this way until the file is saved or cropped.

#### Scenario: Trim the start
- **WHEN** a 10.0 s file is shown and the user drags its start handle to 1.5 s and releases
- **THEN** the file is 8.5 s long and starts with the audio that was at 1.5 s

#### Scenario: Restore trimmed audio
- **WHEN** the user has trimmed 1.5 s from the start, then deletes a selection, then drags the start handle back to the left edge
- **THEN** the trimmed 1.5 s is back at the start and the deletion is unaffected

#### Scenario: Can't trim to nothing
- **WHEN** the user drags the start handle past the end handle
- **THEN** the start stops one frame before the end

### Requirement: Start and end fades
Near the top of each end of the waveform the system SHALL show a fade handle. Dragging the start fade handle right SHALL set the length of a fade-in from the start of the audio; dragging the end fade handle left SHALL set the length of a fade-out ending at the end of the audio. A length of zero means no fade. The fade-in and fade-out together SHALL NOT be longer than the audio. Trimming SHALL keep each fade attached to its end of the audio.

#### Scenario: Fade in the start
- **WHEN** the user drags the start fade handle to 0.5 s and releases
- **THEN** the first 0.5 s rises from silence to full level and the rest of the audio is unchanged

#### Scenario: Fades can't overlap
- **WHEN** a 2.0 s file has a 1.5 s fade-out and the user drags the start fade handle towards the end
- **THEN** the fade-in stops at 0.5 s

#### Scenario: Fade follows a trim
- **WHEN** the file has a 0.5 s fade-in and the user trims 1.0 s from the start
- **THEN** the fade-in still covers the first 0.5 s of the remaining audio

### Requirement: Crossfades at joins
Every join created by Delete SHALL have a crossfade centred on the join. Across it the audio before the cut SHALL fade out, continuing into the audio that was removed, while the audio after the cut fades in, starting from the audio just before it. The crossfade SHALL NOT change the length of the file. A new join SHALL start with a 10 ms linear crossfade, shortened if less audio is available.

#### Scenario: Smooth cut
- **WHEN** the user deletes 4.0–5.0 s of a 10.0 s file
- **THEN** the file is 9.0 s long, and from 3.995 s to 4.005 s the audio before the cut fades out while the audio from just before 5.0 s fades in

### Requirement: Crossfade handles
Each join SHALL show a crossfade marker on the waveform with a handle to change the crossfade's length, down to zero (a hard join), and a bend point to shape it. The crossfade SHALL be no longer than the audio available on both sides of the join allows.

#### Scenario: Longer crossfade
- **WHEN** the user drags a join's crossfade handle out to 200 ms and releases
- **THEN** the blend covers 100 ms each side of the join and the file length is unchanged

#### Scenario: Hard join
- **WHEN** the user drags a join's crossfade length to zero
- **THEN** the audio switches directly from before the cut to after it, with every sample unchanged

#### Scenario: Limited by available audio
- **WHEN** a join is 20 ms from the end of the file and the user drags its crossfade to 200 ms
- **THEN** the crossfade stops at the longest length the audio on both sides allows

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

### Requirement: Handles take priority over selection
Pressing within a few pixels of a handle or bend point SHALL drag that handle; pressing anywhere else on the waveform SHALL select and seek as before. The pointer SHALL change shape over a handle, and each handle SHALL have a tooltip naming it.

#### Scenario: Select near an edge
- **WHEN** the user presses 20 pixels in from the start of the waveform and drags right
- **THEN** a time selection is made and no trim or fade changes

### Requirement: Clip changes are undoable settings
Each released handle drag SHALL be one entry in the undo history. Undo and Redo SHALL restore the previous or next trim, fade or crossfade setting. Adjusting a handle again SHALL replace its setting rather than add another fade. Clip changes SHALL mark the tab as having unsaved changes and SHALL take effect immediately, like other edits, without stopping playback.

#### Scenario: Adjust twice, undo once
- **WHEN** the user sets a 0.5 s fade-in, then drags it to 1.0 s, then presses Undo
- **THEN** the fade-in is 0.5 s again, with the same curve as before

#### Scenario: Release during playback
- **WHEN** the file is playing and the user releases a fade-out handle
- **THEN** playback continues, and the handles and waveform show the new fade with no progress box

### Requirement: Clip settings when saving
Saving SHALL write the trimmed, faded and crossfaded audio. Afterwards the saved audio SHALL be the new starting point: no trim, no start or end fade, no joins, and empty undo history.

#### Scenario: Save a shaped take
- **WHEN** the user trims both ends, adds fades and saves
- **THEN** the saved file contains exactly the audio that was shown, and the handles are back at the edges with no fades
