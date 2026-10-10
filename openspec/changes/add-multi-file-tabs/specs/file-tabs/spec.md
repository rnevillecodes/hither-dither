# Spec Delta

## Purpose

Lets users keep several WAV files open at once, each in its own tab, and switch between them without losing each file's view, selection or playhead.

## ADDED Requirements

### Requirement: Tab per open file
The system SHALL show a tab strip whenever one or more files are open or loading, with one tab per file in the order the files were opened. Each tab SHALL show the file name, give the full path in a tooltip, and have a close button. Exactly one tab SHALL be active and visibly highlighted, and the waveform, metadata and stats panels SHALL show the active tab's file.

#### Scenario: Two files open
- **WHEN** the user opens `a.wav` and then `b.wav`
- **THEN** two tabs labelled `a.wav` and `b.wav` are shown in that order, `b.wav` is active, and the panels show `b.wav`

#### Scenario: No files open
- **WHEN** no files are open or loading
- **THEN** no tab strip is shown and the empty "Open a WAV file" screen is displayed

### Requirement: Newly opened file becomes active
When a file finishes loading, the system SHALL make its tab active. When several files are opened in one action, the tab of the last file to finish loading successfully SHALL end up active.

#### Scenario: Open while another file is shown
- **WHEN** `a.wav` is active and the user opens `b.wav`
- **THEN** a new tab for `b.wav` is added and becomes active, and `a.wav` stays open in its own tab

### Requirement: Re-opening an already open file
If the user opens a file whose path is already open in a tab, the system SHALL activate that tab instead of loading the file again or adding a second tab.

#### Scenario: Open the same file twice
- **WHEN** `a.wav` and `b.wav` are open, `b.wav` is active, and the user opens `a.wav` again
- **THEN** the `a.wav` tab becomes active and there are still exactly two tabs

### Requirement: Loading tabs
While a file is loading, its tab SHALL appear in the tab strip with a loading indicator, and the load progress indicator SHALL still follow the existing progress rules. Other tabs SHALL stay usable during the load.

#### Scenario: Switch tabs during a long load
- **WHEN** `a.wav` is open and the user opens a 1 GB file
- **THEN** a loading tab for the large file appears, and the user can click the `a.wav` tab and interact with its waveform while the load continues

### Requirement: Switching tabs
The system SHALL activate a tab when the user clicks it, and SHALL move to the next or previous tab with Ctrl+Tab and Ctrl+Shift+Tab, wrapping around at the ends.

#### Scenario: Click a tab
- **WHEN** `a.wav` and `b.wav` are open, `b.wav` is active, and the user clicks the `a.wav` tab
- **THEN** `a.wav` becomes active and its waveform, metadata and stats are shown

#### Scenario: Cycle with the keyboard
- **WHEN** three tabs are open, the last one is active, and the user presses Ctrl+Tab
- **THEN** the first tab becomes active

### Requirement: Per-tab view state
Each tab SHALL keep its own zoom level, scroll position, time selection, statistics scope and playhead position. Switching back to a tab SHALL restore all of them exactly as they were left.

#### Scenario: Selection kept across switches
- **WHEN** the user selects 1.0–2.0 s in `a.wav`, zooms in, switches to `b.wav`, then switches back to `a.wav`
- **THEN** `a.wav` shows the same zoom and scroll position, the 1.0–2.0 s selection, and stats labelled "Selection" for that range

#### Scenario: Selections are independent
- **WHEN** `a.wav` has a selection and the user switches to `b.wav`, which has none
- **THEN** `b.wav` shows no selection and whole-file stats

### Requirement: Closing tabs
The system SHALL close a tab when the user clicks its close button, middle-clicks it, or chooses File › Close Tab (Ctrl/Cmd+W) for the active tab. Closing a file SHALL release the resources held for it. Closing a tab that is still loading SHALL cancel its load.

#### Scenario: Close with the keyboard
- **WHEN** `a.wav` and `b.wav` are open, `b.wav` is active, and the user presses Ctrl/Cmd+W
- **THEN** the `b.wav` tab is removed and `a.wav` remains open

#### Scenario: Close a loading tab
- **WHEN** a large file is loading in a tab and the user clicks that tab's close button
- **THEN** the load stops, the tab is removed, and no error is shown

### Requirement: Active tab after closing
When the active tab is closed, the system SHALL activate the tab to its right, or the tab to its left if it was the last tab. When the last remaining tab is closed, the system SHALL show the empty "Open a WAV file" screen.

#### Scenario: Close the middle tab
- **WHEN** tabs `a`, `b`, `c` are open, `b` is active, and the user closes `b`
- **THEN** `c` becomes active

#### Scenario: Close the last tab
- **WHEN** only `a.wav` is open and the user closes its tab
- **THEN** no tabs remain and the empty "Open a WAV file" screen with its Open button is shown

### Requirement: Playback follows the active tab
Transport controls SHALL act on the active tab's file. Switching to another tab, or closing the tab that is playing, SHALL stop playback. The playhead of the tab that was playing SHALL stay where playback stopped.

#### Scenario: Switch tabs during playback
- **WHEN** `a.wav` is playing at 12.0 s and the user switches to `b.wav`
- **THEN** playback stops, and switching back to `a.wav` shows the playhead at about 12.0 s

#### Scenario: Play in another tab
- **WHEN** the user switches to `b.wav` and presses Space
- **THEN** `b.wav` plays from its own playhead position
