# Spec Delta

## Purpose

Defines HitherDither's single dark colour scheme: slate-grey surfaces, whitish text and teal highlights. It covers which colour plays which role and the minimum contrast between them, so the interface reads clearly and matches the app icon.

## ADDED Requirements

### Requirement: Always-dark appearance
The app SHALL always use its dark colour scheme, whatever the operating system's light/dark setting, and SHALL NOT switch theme when that setting changes while the app is running. Native form controls and scrollbars SHALL render in their dark style.

#### Scenario: OS set to light mode
- **WHEN** the operating system uses a light theme and the app starts
- **THEN** the app shows slate-grey surfaces with whitish text

#### Scenario: OS theme changes while running
- **WHEN** the user switches the OS from dark to light while a file is open
- **THEN** the app's colours, including the waveform, do not change

### Requirement: Slate-grey surfaces
The window background, panels, toolbar, tab strip, buttons and waveform lanes SHALL use slate-grey tones, from darkest for the window and waveform background to lighter for raised surfaces such as panels, buttons and the active tab. Borders SHALL be a mid slate grey visible against the surfaces they separate.

#### Scenario: Panels stand out from the background
- **WHEN** a file is open and the details panel is shown
- **THEN** the panel, toolbar and active tab are visibly lighter than the window background, and their borders are visible

### Requirement: Whitish text with strong contrast
Primary text SHALL be a whitish colour with a contrast ratio of at least 7:1 against every surface it appears on. Secondary (dimmed) text SHALL have at least 4.5:1 against those surfaces.

#### Scenario: Reading the stats panel
- **WHEN** the stats panel shows values and labels
- **THEN** values use the whitish primary text colour and labels use the dimmed colour, each meeting its contrast minimum on the panel background

### Requirement: Teal highlights
Teal SHALL be the single highlight colour. It marks interactive emphasis and current state: primary buttons, the active tab, toggled-on buttons, progress bars, informational notice accents, and the time selection and overview window in the waveform view. Text drawn on a solid teal fill SHALL have at least 4.5:1 contrast with it. Teal used as a marker or text on slate surfaces SHALL have at least 3:1 contrast with them.

#### Scenario: Active tab and toggles
- **WHEN** two tabs are open and the details panel is shown
- **THEN** the active tab's marker and the pressed panel toggle are teal, and no blue highlight appears anywhere

#### Scenario: Selection
- **WHEN** the user selects a time range on the waveform
- **THEN** the selection is a translucent teal band with teal edges, and the waveform under it stays visible

### Requirement: Distinct waveform and status colours
The waveform SHALL be drawn in a light teal tint that stays distinguishable from the teal selection band over it. The playhead SHALL be amber and clipping markers red, each with at least 3:1 contrast against the lane background. Error and warning notices SHALL keep their red and amber colours.

#### Scenario: Playhead over a selection
- **WHEN** audio plays through a selected range
- **THEN** the amber playhead line is clearly visible over both the waveform and the teal selection

#### Scenario: Clipped audio
- **WHEN** a file with clipped runs is open
- **THEN** the clip markers show in red, distinct from the teal and amber colours

### Requirement: Teal keyboard focus indicator
Every focusable control (buttons, tabs, the volume slider) SHALL show a teal focus ring when reached with the keyboard. Mouse clicks SHALL NOT show the ring.

#### Scenario: Tabbing through the toolbar
- **WHEN** the user presses Tab to move focus onto a toolbar button
- **THEN** that button shows a teal outline

### Requirement: Dark window frame
Where the platform allows the app to choose, the native window title bar and frame SHALL use the dark style.

#### Scenario: Windows title bar
- **WHEN** the app runs on Windows 10/11 with the OS in light mode
- **THEN** the window's title bar is dark
