# Spec Delta

## Purpose

Covers the transport buttons in the toolbar that move the playhead, starting with a one-click way back to the start of the file.

## ADDED Requirements

### Requirement: Go to start button
The transport SHALL include a "Go to start" button, placed before the Play button, that moves the active tab's playhead to the first frame, exactly like the Home key. If playback is running, it SHALL continue from the start. If the start is scrolled out of view, the view SHALL scroll to show it without changing the zoom. The button SHALL be enabled whenever a loaded file is active, even when no audio output is available.

#### Scenario: While stopped
- **WHEN** the playhead is at 12.0 s and playback is stopped, and the user clicks Go to start
- **THEN** the playhead moves to 0.0 s and the time display shows 0:00:00.000

#### Scenario: While playing
- **WHEN** the file is playing at 30.0 s and the user clicks Go to start
- **THEN** playback continues from 0.0 s without stopping

#### Scenario: Zoomed in elsewhere
- **WHEN** the view is zoomed in around 5:00 and the user clicks Go to start
- **THEN** the view scrolls to the beginning of the file at the same zoom level, with the playhead visible at 0.0 s

#### Scenario: Accessible name and tooltip
- **WHEN** the user hovers or focuses the button
- **THEN** its tooltip reads "Go to start (Home)" and assistive technology announces it as "Go to start"
