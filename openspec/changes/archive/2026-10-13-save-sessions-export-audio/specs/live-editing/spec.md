# Spec Delta

## MODIFIED Requirements

### Requirement: Audio rendered only when saving
Edits, undo, redo, previews and saving a session SHALL NOT write audio to disk or create rendered copies. The edited audio SHALL be rendered only by File › Export….

#### Scenario: Many edits, no files
- **WHEN** the user makes many edits to `a.wav` and saves its session
- **THEN** the only new file is the session file, and `a.wav` is unchanged on disk

#### Scenario: Closing never renders
- **WHEN** the user closes a tab with unsaved edits and chooses Save
- **THEN** only the session file is written; no WAV is rendered
