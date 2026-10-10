# Spec Delta

## Purpose

Lets users listen to the loaded WAV file, or a selected part of it, with a playhead that shows the current position in the waveform view.

## ADDED Requirements

### Requirement: Transport controls
The system SHALL provide Play/Pause (toolbar button and Space key) and Stop (toolbar button). Stop SHALL halt playback and return the playhead to the start position of the last play action.

#### Scenario: Play and pause
- **WHEN** a file is loaded and the user presses Space
- **THEN** playback starts from the playhead position; pressing Space again pauses it and the playhead stays where it paused

#### Scenario: Stop
- **WHEN** audio is playing and the user clicks Stop
- **THEN** playback halts and the playhead returns to where that playback began

### Requirement: Seeking
The system SHALL move the playhead to the clicked time when the user clicks (without dragging) on the waveform or ruler, and SHALL move it to the start or end with the Home and End keys. Seeking during playback SHALL continue playback from the new position.

#### Scenario: Seek during playback
- **WHEN** audio is playing and the user clicks at 30.0 s
- **THEN** playback continues from 30.0 s

### Requirement: Selection playback
When a selection exists, the system SHALL play only the selected range and stop at its end, unless loop mode is enabled, in which case it SHALL repeat the selection until paused or stopped.

#### Scenario: Play selection
- **WHEN** a selection from 1.0 s to 2.0 s exists and the user presses Play
- **THEN** audio plays from 1.0 s and stops at 2.0 s

#### Scenario: Loop selection
- **WHEN** loop mode is on and a selection exists and the user presses Play
- **THEN** the selection repeats until the user pauses or stops

### Requirement: Playhead display
The system SHALL draw a playhead line across all waveform lanes and display the current position as time, updating at least 30 times per second during playback with visual offset from the audible position no greater than 50 ms. The view SHALL auto-scroll to keep the playhead visible during playback unless the user has scrolled manually since playback started.

#### Scenario: Playhead follows playback
- **WHEN** audio plays past the right edge of the visible range
- **THEN** the view scrolls so the playhead remains visible

### Requirement: Output device and format handling
The system SHALL play through the system's default output device, resampling if the device does not support the file's sample rate and down-mixing or mapping channels when the file has more channels than the device. Mono files SHALL play on both channels of a stereo device. If no output device is available, playback controls SHALL be disabled with a message explaining why, while analysis features keep working.

#### Scenario: Sample rate mismatch
- **WHEN** a 96 kHz file is played on a device running at 48 kHz
- **THEN** audio plays at the correct pitch and speed

#### Scenario: No audio device
- **WHEN** no output device is available
- **THEN** playback controls are disabled with an explanatory message and waveform and stats still work

### Requirement: Volume control
The system SHALL provide an output volume control that scales playback gain only and does not change displayed waveform or statistics.

#### Scenario: Lower volume
- **WHEN** the user reduces volume to 50%
- **THEN** playback is quieter and the stats panel values are unchanged
