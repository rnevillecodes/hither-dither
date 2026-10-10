# Spec Delta

## Purpose

Shows the loaded audio as an interactive per-channel waveform so users can visually inspect it, navigate in time, and select a region for analysis or playback.

## ADDED Requirements

### Requirement: Per-channel waveform display
The system SHALL render one waveform lane per channel, stacked vertically, each with an amplitude scale from -1.0 to +1.0 (full scale) and a channel label. When zoomed out so that many samples map to one pixel column, each column SHALL show the min-to-max range of the samples it covers so that peaks are never hidden.

#### Scenario: Stereo file display
- **WHEN** a 2-channel file is loaded
- **THEN** two labelled lanes are shown, for the left and right channels

#### Scenario: Short transient remains visible when zoomed out
- **WHEN** a file contains a single full-scale sample in otherwise silent audio and the whole file is in view
- **THEN** the lane shows a visible spike at that position reaching full scale

### Requirement: Time axis
The system SHALL show a time ruler aligned with the waveform, with labels that adapt to the zoom level (from hours:minutes:seconds down to individual sample indices at maximum zoom).

#### Scenario: Ruler adapts to zoom
- **WHEN** the user zooms in until individual samples are distinguishable
- **THEN** the ruler shows sample-level or sub-millisecond tick labels

### Requirement: Zoom and scroll
The system SHALL let users zoom horizontally with Ctrl/Cmd+mouse-wheel (centred on the pointer), with +/- keys, and with a "zoom to fit" action, from the whole file down to at least 1 sample per 10 pixels. It SHALL let users scroll horizontally with the mouse wheel, Shift+wheel, a scrollbar, and by dragging an overview strip. When zoomed in far enough, individual samples SHALL be drawn as points connected by lines.

#### Scenario: Zoom around pointer
- **WHEN** the user Ctrl+scrolls up with the pointer over a given time position
- **THEN** the view zooms in and that time position remains under the pointer

#### Scenario: Zoom to fit
- **WHEN** the user triggers "zoom to fit"
- **THEN** the entire file is visible in the view

### Requirement: Time selection
The system SHALL let users select a time range by click-dragging across the waveform, extend it with Shift+click, and clear it with Escape or a click without dragging. The selection SHALL apply to all channels, be visibly highlighted, and its start, end and duration SHALL be displayed.

#### Scenario: Create a selection
- **WHEN** the user click-drags from 1.0 s to 2.5 s
- **THEN** that range is highlighted across all lanes and the display shows start 1.000 s, end 2.500 s, duration 1.500 s

#### Scenario: Clear the selection
- **WHEN** a selection exists and the user presses Escape
- **THEN** the selection is removed

### Requirement: Rendering performance
The system SHALL redraw the waveform while zooming or scrolling at an interactive rate (target 30 frames per second or better) for files up to 2 GB.

#### Scenario: Scrolling a long file
- **WHEN** the user continuously scrolls a 1-hour stereo 48 kHz file
- **THEN** the view updates smoothly without freezing the UI
