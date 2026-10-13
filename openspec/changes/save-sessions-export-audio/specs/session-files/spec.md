# Spec Delta

## MODIFIED Requirements

### Requirement: Restored history and unsaved marker
A restored session SHALL start a new undo history whose first state is the restored state, so Undo is unavailable until the next change. The restored tab SHALL NOT show the unsaved marker, because it matches its session file; any later change SHALL show it, as for any other tab.

#### Scenario: Nothing to undo after opening
- **WHEN** a session with edits is opened
- **THEN** Undo is disabled and the tab doesn't show the unsaved marker

#### Scenario: Change after opening
- **WHEN** a session is opened and the user bypasses one of its effects
- **THEN** the tab shows the unsaved marker

## REMOVED Requirements

### Requirement: Save Session
**Reason**: Saving the session is now what File › Save and Save As do, so the separate Save Session items and Ctrl/Cmd+Alt+S are removed.
**Migration**: Use File › Save (Ctrl/Cmd+S) and File › Save As… (Ctrl/Cmd+Shift+S); see "Save and Save As write the session".

## ADDED Requirements

### Requirement: Save and Save As write the session
File › Save (Ctrl/Cmd+S) SHALL write the active tab's session file without asking once the tab has one, and otherwise behave as Save As. File › Save As… (Ctrl/Cmd+Shift+S) SHALL ask for a location, defaulting to the tab's session file or else the WAV's folder and name with the `.hdsession.json` extension, and the tab SHALL then use the new session file.

#### Scenario: First save asks
- **WHEN** the user presses Ctrl/Cmd+S in a `take.wav` tab that has never had a session file
- **THEN** a save dialog opens proposing `take.hdsession.json` next to `take.wav`

#### Scenario: Later saves overwrite
- **WHEN** the tab was saved to or opened from `mix.hdsession.json`, has unsaved changes, and the user presses Ctrl/Cmd+S
- **THEN** `mix.hdsession.json` is overwritten with the current state and no dialog is shown

#### Scenario: Save As switches session
- **WHEN** a tab opened from `mix.hdsession.json` is saved as `mix-alt.hdsession.json`
- **THEN** `mix.hdsession.json` is unchanged and later saves write `mix-alt.hdsession.json`

#### Scenario: Name typed without the extension
- **WHEN** the user types `mix` or `mix.json` as the file name in the Save As dialog
- **THEN** the session is saved as `mix.hdsession.json`

#### Scenario: Cancel the dialog
- **WHEN** the user cancels the Save As dialog
- **THEN** no file is written, the tab's session file is unchanged, and the tab keeps its unsaved marker


### Requirement: When Save is available
Save SHALL be enabled while the active tab is loaded, not exporting, and either unsaved or without a session file. Save As SHALL be enabled whenever the active tab is loaded and not exporting.

#### Scenario: Nothing new to save
- **WHEN** a tab opened from `mix.hdsession.json` has no unsaved changes
- **THEN** Save is disabled and Save As is enabled

#### Scenario: Not available while exporting
- **WHEN** the active tab is still loading or an export of it is in progress
- **THEN** Save and Save As are disabled

### Requirement: Last-session restore reopens sessions
When the app remembers the open tabs for the next launch, a tab with a session file SHALL be remembered by its session file, and reopened by opening that session, restored as it was last saved. Tabs without a session file SHALL be remembered by their WAV, as before.

#### Scenario: Session tab after a restart
- **WHEN** the app is closed with a tab opened from `mix.hdsession.json` and a tab for `b.wav` without a session, and then started again
- **THEN** `mix.hdsession.json` is opened with its edits and effects, and `b.wav` is opened as a plain file, in the same tab order

#### Scenario: Unsaved changes are not kept by the restart
- **WHEN** a session tab has unsaved changes and the user quits choosing Don't Save, then starts the app
- **THEN** the session is reopened as it was last saved
