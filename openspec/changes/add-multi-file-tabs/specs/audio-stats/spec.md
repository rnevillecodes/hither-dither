# Spec Delta

## ADDED Requirements

### Requirement: Details panel visibility
The panel showing file metadata and level statistics SHALL be hidden while no files are open, so the empty screen uses the full window width. While files are open, a toolbar button SHALL show and hide the panel, and the waveform SHALL widen into the freed space. The panel SHALL start shown, and the user's choice SHALL stay the same across tab switches and app restarts.

#### Scenario: No files open
- **WHEN** no files are open
- **THEN** no details panel and no panel toggle button are shown

#### Scenario: Hide the panel
- **WHEN** a file is open, the panel is shown, and the user clicks the panel toggle button
- **THEN** the panel is hidden and the waveform fills the window width

#### Scenario: Choice is remembered
- **WHEN** the user hides the panel, switches tabs, and later restarts the app and opens a file
- **THEN** the panel stays hidden until the user shows it again
