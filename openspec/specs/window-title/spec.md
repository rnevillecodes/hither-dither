# window-title Specification

## Purpose

Shows in the main window's title which session the active tab belongs to, and whether it has unsaved changes, so the user can tell what they are working on from the title bar, taskbar and window switcher.

## Requirements

### Requirement: Session name in the title
While the active tab is loaded and has a session file, the main window's title SHALL be the session file's name without its `.hdsession.json` extension, followed by " — HitherDither". Otherwise (no tab open, the active tab still loading, or the active tab never saved as or opened from a session) the title SHALL be "HitherDither".

#### Scenario: Saved session
- **WHEN** the active tab was opened from `C:\takes\mix.hdsession.json` and has no unsaved changes
- **THEN** the window title is "mix — HitherDither"

#### Scenario: Never saved
- **WHEN** the active tab is `take.wav` and has never been saved as a session
- **THEN** the window title is "HitherDither", even if it has unsaved changes

#### Scenario: No tabs
- **WHEN** the last tab is closed
- **THEN** the window title is "HitherDither"

### Requirement: Unsaved marker in the title
While the active tab has a session file and unsaved changes, a "•" SHALL follow the session name in the title, as in "mix • — HitherDither", and SHALL go away when the changes are saved or undone back to the saved state.

#### Scenario: Change after saving
- **WHEN** the active tab's session `mix` was just saved and the user applies a fade
- **THEN** the title is "mix • — HitherDither", and after Save it is "mix — HitherDither" again

### Requirement: Title follows the active tab
The title SHALL change as soon as the active tab changes, its session file changes (Save on a tab without a session, Save As, opening a session) or its unsaved state changes.

#### Scenario: Switch tabs
- **WHEN** tab `a.wav` has session `mix` and tab `b.wav` has none, and the user switches from `a.wav` to `b.wav`
- **THEN** the title changes from "mix — HitherDither" to "HitherDither"

#### Scenario: Save As
- **WHEN** the active tab's session is `mix` and the user saves it as `mix-v2.hdsession.json`
- **THEN** the title is "mix-v2 — HitherDither"
