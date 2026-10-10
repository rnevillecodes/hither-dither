# Spec Delta

## Purpose

Gives hither-dither a distinctive, recognisable identity wherever the operating system shows the app: window, taskbar or dock, installer and the web favicon.

## ADDED Requirements

### Requirement: Custom application icon
The system SHALL use a project-specific icon, not the default Tauri logo, for the window and taskbar/dock icon, the platform installers and bundles (Windows `.ico`, macOS `.icns`, Linux PNGs) and the frontend favicon. Every variant SHALL be generated from a single source image kept in the repository.

#### Scenario: Running app on Windows
- **WHEN** the app is running on Windows
- **THEN** the window title bar and taskbar show the hither-dither icon, not the Tauri logo

#### Scenario: Installed bundle
- **WHEN** the release installer from `tauri build` is installed
- **THEN** the installed app's shortcut or launcher shows the hither-dither icon

### Requirement: Icon legibility
The icon SHALL express the app's purpose (audio waveform analysis) through a simple shape that stays recognisable at 16×16 and 32×32 pixels. It SHALL also stay clearly visible on light and on dark taskbar or dock backgrounds.

#### Scenario: Small size
- **WHEN** the 32×32 icon is viewed at 100% scale
- **THEN** the main shape can still be identified and is not muddy noise

#### Scenario: Dark and light backgrounds
- **WHEN** the icon is shown on a pure black and on a pure white background
- **THEN** its outline is clearly separated from the background in both cases

### Requirement: Empty-screen watermark
While no files are open, the empty screen SHALL show a large, flat, single-colour imprint of the app icon's details (its waveform bars and dither dots, without the tile) above the "Open a WAV file" message. It SHALL be low-contrast against the background in both light and dark themes, be decorative only (hidden from assistive technology), and not block clicks on the Open button.

#### Scenario: Nothing open
- **WHEN** the app starts with no files open
- **THEN** a large faint imprint of the icon's waveform is shown above the "Open a WAV file" message and Open button

#### Scenario: Dark theme
- **WHEN** the OS uses a dark theme and no files are open
- **THEN** the imprint is a faint light tint on the dark background

#### Scenario: Files open
- **WHEN** a file is open
- **THEN** the imprint is not shown
