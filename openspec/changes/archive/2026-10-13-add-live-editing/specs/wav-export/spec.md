# Spec Delta

## MODIFIED Requirements

### Requirement: After saving
Saving SHALL render the edited audio, first finishing any pending background analysis of it. Meanwhile the tab SHALL show the delayed progress indicator used for loads, and edits on it SHALL be disabled. After a successful Save or Save As, the tab SHALL have no unsaved marker and an empty undo/redo history; its view, selection, playhead and shown audio SHALL be unchanged; and its whole-file statistics, clip markers and waveform SHALL be exact, not marked as updating.

#### Scenario: Keep working after save
- **WHEN** the user zooms in, selects 1.0–2.0 s, applies Fade In and saves
- **THEN** the zoom and the 1.0–2.0 s selection are unchanged, and Undo is greyed out

#### Scenario: Save before the analysis finishes
- **WHEN** the user edits a long file and saves it straight away, while the whole-file statistics are still updating
- **THEN** the progress indicator appears if the save takes a moment, and afterwards the statistics are up to date and no longer marked as updating

#### Scenario: Progress while rendering
- **WHEN** the user saves a long edited file
- **THEN** the delayed progress indicator shows how far the save has got, and other tabs stay usable
