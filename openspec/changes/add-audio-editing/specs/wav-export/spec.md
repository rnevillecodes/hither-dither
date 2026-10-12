# Spec Delta

## Purpose

Writes edited audio back to disk as a WAV file. Save replaces the original and Save As creates a new file. Unsaved edits are protected from being lost when a tab is closed or the app quits.

## ADDED Requirements

### Requirement: Unsaved marker
A tab whose audio differs from its file on disk SHALL show an unsaved marker in the tab strip, and its tooltip SHALL say that it has unsaved changes. The marker SHALL go away when the edits are saved or every edit is undone.

#### Scenario: Edit marks the tab
- **WHEN** the user silences a selection in `a.wav`
- **THEN** the `a.wav` tab shows the unsaved marker

#### Scenario: Undo back to the original
- **WHEN** the user makes one edit to `a.wav` and then undoes it
- **THEN** the unsaved marker disappears

### Requirement: Save over the original
File › Save (Ctrl/Cmd+S) SHALL write the active tab's edited audio to its own path. It SHALL be disabled when the tab has no unsaved changes. The first Save for a tab SHALL ask for confirmation that the original will be replaced; once confirmed, later Saves of that tab SHALL NOT ask again.

#### Scenario: First save asks
- **WHEN** the user crops `a.wav` and presses Ctrl/Cmd+S
- **THEN** a dialog asks to confirm overwriting `a.wav`; choosing Overwrite saves it, and choosing Cancel leaves the file and the edits as they were

#### Scenario: Later saves don't ask
- **WHEN** the user has already confirmed one Save of `a.wav`, makes another edit and presses Ctrl/Cmd+S
- **THEN** the file is saved without a dialog

### Requirement: Save As
File › Save As (Ctrl/Cmd+Shift+S) SHALL ask for a new file name with a native save dialog filtered to WAV, and write the active tab's audio there. It SHALL be available for any loaded tab, even without edits. Afterwards the tab SHALL refer to the new file (name, tooltip, remembered session), and the original file SHALL be unchanged.

#### Scenario: Save a trimmed copy
- **WHEN** the user crops `take1.wav` and saves it as `take1-trim.wav`
- **THEN** `take1-trim.wav` contains the cropped audio, `take1.wav` is unchanged, and the tab is labelled `take1-trim.wav` with no unsaved marker

#### Scenario: Cancel the dialog
- **WHEN** the user opens Save As and cancels
- **THEN** nothing is written and the tab keeps its edits and its unsaved marker

#### Scenario: Name of another open file
- **WHEN** the user chooses the path of a file that is open in another tab
- **THEN** nothing is written and a notice explains that the file is open in another tab

### Requirement: Output format
Saved files SHALL be RIFF WAVE files containing exactly a `fmt ` chunk identical to the source's and a `data` chunk, with no other chunks. Sample format, bit depth, valid bits, sample rate, channel count and channel mask SHALL match the source. Samples an edit didn't change SHALL be written bit-for-bit as in the source. Changed integer samples SHALL be rounded to the nearest value and clamped to the format's range.

#### Scenario: Untouched audio is bit-exact
- **WHEN** the user deletes 1.0–2.0 s from a 24-bit file and saves it
- **THEN** every sample of the saved file equals the source sample at the corresponding position

#### Scenario: Metadata is not carried over
- **WHEN** the source has `LIST` and `bext` chunks and the user saves an edit
- **THEN** the saved file has only `fmt ` and `data` chunks, and HitherDither reopens it with the same format details

#### Scenario: Result too large for WAV
- **WHEN** the edited audio would make the `data` chunk larger than 4 GiB minus the header
- **THEN** Save and Save As fail with a notice saying the result is too large, and nothing is written

### Requirement: Safe writing
Saving SHALL NOT leave a partial or corrupted file at the target path. If writing fails (for example the disk is full or access is denied), the system SHALL show an error notice, leave any existing file at the target unchanged, and keep the tab's edits and unsaved marker. Save SHALL refuse with a notice suggesting Save As if the original file has changed on disk since it was opened.

#### Scenario: Read-only target
- **WHEN** the user saves a file whose folder is read-only
- **THEN** an error notice explains that it could not be saved, the original file is intact, and the edits are still there

#### Scenario: File changed by another program
- **WHEN** another program modifies `a.wav` after it was opened, and the user edits and saves it
- **THEN** nothing is written and a notice says the file changed on disk and suggests Save As

### Requirement: After saving
After a successful Save or Save As, the tab SHALL have no unsaved marker and an empty undo/redo history. Its view, selection and playhead SHALL be unchanged, and the shown audio, statistics and clip markers SHALL be identical to before the save, without a reload.

#### Scenario: Keep working after save
- **WHEN** the user zooms in, selects 1.0–2.0 s, applies Fade In and saves
- **THEN** the zoom and the 1.0–2.0 s selection are unchanged, and Undo is greyed out

### Requirement: Closing a tab with unsaved edits
Closing a tab that has unsaved changes (close button, middle-click, Ctrl/Cmd+W, File › Close Tab) SHALL ask whether to Save, Don't Save or Cancel. Save SHALL save the tab without asking for overwrite confirmation and then close it, unless saving fails. Don't Save SHALL close it and discard the edits. Cancel SHALL keep the tab open.

#### Scenario: Cancel closing
- **WHEN** `a.wav` has unsaved edits, the user presses Ctrl/Cmd+W and chooses Cancel
- **THEN** the tab stays open with its edits

#### Scenario: Save on close
- **WHEN** `a.wav` has unsaved edits, the user closes its tab and chooses Save
- **THEN** `a.wav` is written with the edits and the tab closes

### Requirement: Quitting with unsaved edits
Quitting the app (window close button, File › Quit, or the OS) while any tab has unsaved changes SHALL ask, for each such tab in tab order, whether to Save, Don't Save or Cancel. Cancel at any prompt SHALL abort the quit and keep every tab open. Quitting with no unsaved changes SHALL NOT ask.

#### Scenario: Two edited files
- **WHEN** `a.wav` and `b.wav` both have unsaved edits and the user closes the window
- **THEN** the app asks about `a.wav`, then about `b.wav`, and quits only after both are answered with Save or Don't Save

#### Scenario: Cancel quitting
- **WHEN** the user quits with unsaved edits and chooses Cancel at the first prompt
- **THEN** the app keeps running with every tab and edit intact
