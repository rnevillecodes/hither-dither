# Spec Delta

## MODIFIED Requirements

### Requirement: Undo and redo
The system SHALL keep a per-tab history of edits since the tab was opened, from a WAV or from a session file. Saving the session or exporting SHALL NOT clear it. Undo (Ctrl/Cmd+Z, Edit › Undo) SHALL revert the most recent edit, and Redo (Ctrl/Cmd+Shift+Z, Ctrl+Y, Edit › Redo) SHALL reapply the most recently undone edit. Making a new edit after undoing SHALL discard the redo history. Undo and Redo SHALL be disabled when there is nothing to undo or redo.

#### Scenario: Undo a delete
- **WHEN** the user deletes 4.0–5.0 s of a 10.0 s file and presses Ctrl/Cmd+Z
- **THEN** the file is 10.0 s long again with the original audio, and Redo is enabled

#### Scenario: Several undos
- **WHEN** the user applies Crop, then Silence, then Fade In, and presses Undo three times
- **THEN** the audio is identical to the file as opened, and Undo is greyed out

#### Scenario: New edit clears redo
- **WHEN** the user undoes an edit and then applies a different edit
- **THEN** Redo is greyed out

#### Scenario: History is per tab
- **WHEN** the user edits `a.wav`, switches to `b.wav` and presses Ctrl/Cmd+Z
- **THEN** nothing in either file changes, because `b.wav` has no edits

#### Scenario: Undo after saving the session
- **WHEN** the user fades in a selection, saves the session and presses Ctrl/Cmd+Z
- **THEN** the fade is undone and the tab shows the unsaved marker

## REMOVED Requirements

### Requirement: Original file untouched until saved
**Reason**: Save no longer writes the WAV at all, so "until saved" no longer applies.
**Migration**: Replaced by "Original file never modified".

## ADDED Requirements

### Requirement: Original file never modified
Edits, undo, redo, saving a session and exporting SHALL NOT modify the WAV a tab was opened from.

#### Scenario: Close without saving
- **WHEN** the user crops a file, closes its tab and chooses Don't Save, then reopens the file
- **THEN** the file has its original length and content

#### Scenario: Save and export leave it alone
- **WHEN** the user crops `a.wav`, saves its session and exports it to `a-export.wav`
- **THEN** `a.wav` has its original length and content
