# Spec Delta

## Purpose

Defines how the main application window is presented: the state it opens in so the whole interface fits on the user's screen, and which window chrome is shown when no files are open.

## ADDED Requirements

### Requirement: Launch maximized
The main window SHALL open maximized on the display it appears on, so that the entire interface, including the bottom edge, is visible without resizing on any screen size, including small laptop screens. The title bar, native menu and OS taskbar/dock SHALL stay visible (not borderless fullscreen), and the user SHALL be able to restore, resize and re-maximize the window normally.

#### Scenario: Laptop screen
- **WHEN** the app is launched on a 1366×768 laptop display
- **THEN** the window fills the work area above the taskbar and no part of the interface is cut off at the bottom

#### Scenario: Restore down
- **WHEN** the user clicks the window's restore button after launch
- **THEN** the window becomes a normal resizable window no smaller than the minimum size

### Requirement: No toolbar on the empty screen
While no files are open or loading, the system SHALL NOT show the top toolbar; the empty "Open a WAV file" screen SHALL take the full height below the native menu. As soon as a file starts loading, the toolbar SHALL appear, and it SHALL disappear again when the last tab is closed.

#### Scenario: App starts with nothing open
- **WHEN** the app starts with no files to open
- **THEN** no toolbar strip is shown above the empty screen

#### Scenario: Toolbar returns with a file
- **WHEN** the empty screen is shown and the user opens `a.wav`
- **THEN** the toolbar appears with the tab strip, and once `a.wav` loads it shows the transport, zoom and selection controls

#### Scenario: Last tab closed
- **WHEN** only `a.wav` is open and the user closes its tab
- **THEN** the toolbar and tab strip are hidden and the empty screen fills the window
