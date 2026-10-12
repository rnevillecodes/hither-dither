# Spec Delta

## MODIFIED Requirements

### Requirement: After saving
Saving SHALL render the edited audio to the file. While it does, the tab SHALL show the same delayed progress indicator as a load, and further edits on that tab SHALL be disabled until it finishes. If the background analysis of the saved audio has not finished, saving SHALL complete it first. After a successful Save or Save As, the tab SHALL have no unsaved marker and an empty undo/redo history. Its view, selection and playhead SHALL be unchanged, and the shown audio SHALL be identical to before the save. Whole-file statistics, clip markers and the waveform SHALL be exact and not marked as updating.

#### Scenario: Keep working after save
- **WHEN** the user zooms in, selects 1.0–2.0 s, applies Fade In and saves
- **THEN** the zoom and the 1.0–2.0 s selection are unchanged, and Undo is greyed out

#### Scenario: Save before the analysis finishes
- **WHEN** the user edits a long file and saves it straight away, while the whole-file statistics are still updating
- **THEN** the progress indicator appears if the save takes a moment, and afterwards the statistics are up to date and no longer marked as updating

#### Scenario: Progress while rendering
- **WHEN** the user saves a long edited file
- **THEN** the delayed progress indicator shows how far the save has got, and other tabs stay usable
