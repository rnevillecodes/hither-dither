# wav-export Specification

## Purpose

Writes the edited audio to a new WAV file with Export, never over the WAV being edited, and protects unsaved changes from being lost when a tab is closed or the app quits.

## Requirements

### Requirement: Unsaved marker
A tab SHALL show an unsaved marker in the tab strip, with a tooltip saying it has unsaved changes, while its state differs from its session as last saved or opened (or, for a tab without a session, from the WAV as opened). Edits, undo and redo, effect chain changes, bypass switches and plugin setting changes SHALL all count. The marker SHALL go away when the session is saved, or when undo or redo returns to the saved state with no setting changed since.

#### Scenario: Edit marks the tab
- **WHEN** the user silences a selection in `a.wav`
- **THEN** the `a.wav` tab shows the unsaved marker

#### Scenario: Undo back to the original
- **WHEN** the user makes one edit to `a.wav` and then undoes it
- **THEN** the unsaved marker disappears

#### Scenario: Plugin setting changed
- **WHEN** the session of `a.wav` was just saved and the user turns a knob in an effect's editor
- **THEN** the tab shows the unsaved marker, and undoing doesn't remove it

#### Scenario: Saving the session clears it
- **WHEN** `a.wav` has unsaved edits and the user saves its session
- **THEN** the marker disappears, `a.wav` is unchanged on disk, and Undo is still available

### Requirement: Output format
Exported files SHALL be RIFF WAVE files containing exactly a `fmt ` chunk identical to the source's and a `data` chunk, with no other chunks. Sample format, bit depth, valid bits, sample rate, channel count and channel mask SHALL match the source. Samples that no edit, fade, crossfade or active effect changed SHALL be written bit-for-bit as in the source. Changed integer samples, including blended crossfade samples, SHALL be rounded to the nearest value and clamped to the format's range.

#### Scenario: Untouched audio is bit-exact
- **WHEN** the user deletes 1.0–2.0 s from a 24-bit file and exports it
- **THEN** every sample of the exported file outside the crossfade at the join equals the source sample at the corresponding position

#### Scenario: Hard join is bit-exact
- **WHEN** the user deletes 1.0–2.0 s from a 24-bit file, drags the join's crossfade to zero and exports it
- **THEN** every sample of the exported file equals the source sample at the corresponding position

#### Scenario: Metadata is not carried over
- **WHEN** the source has `LIST` and `bext` chunks and the user exports an edit
- **THEN** the exported file has only `fmt ` and `data` chunks, and HitherDither reopens it with the same format details

#### Scenario: Result too large for WAV
- **WHEN** the edited audio would make the `data` chunk larger than 4 GiB minus the header
- **THEN** Export fails with a notice saying the result is too large, and nothing is written

### Requirement: Closing a tab with unsaved edits
Closing a tab that has unsaved changes (close button, middle-click, Ctrl/Cmd+W, File › Close Tab) SHALL ask whether to Save, Don't Save or Cancel. Save SHALL save the tab's session, asking where if the tab has no session file yet, and then close the tab, unless saving fails or the user cancels the dialog. Don't Save SHALL close it and discard the changes. Cancel SHALL keep the tab open. No choice SHALL write the WAV.

#### Scenario: Cancel closing
- **WHEN** `a.wav` has unsaved edits, the user presses Ctrl/Cmd+W and chooses Cancel
- **THEN** the tab stays open with its edits

#### Scenario: Save on close
- **WHEN** `a.wav`, opened from `mix.hdsession.json`, has unsaved edits, and the user closes its tab and chooses Save
- **THEN** `mix.hdsession.json` is written with the edits, `a.wav` is unchanged, and the tab closes

#### Scenario: Save on close without a session
- **WHEN** `a.wav` has no session file, has unsaved edits, and the user closes its tab and chooses Save
- **THEN** a save dialog proposes `a.hdsession.json`; saving there closes the tab, and cancelling the dialog keeps it open

### Requirement: Quitting with unsaved edits
Quitting the app (window close button, File › Quit, or the OS) while any tab has unsaved changes SHALL ask, for each such tab in tab order, whether to Save, Don't Save or Cancel, where Save saves the tab's session as when closing it. Cancel at any prompt, or cancelling a session save dialog, SHALL abort the quit and keep every tab open. Quitting with no unsaved changes SHALL NOT ask.

#### Scenario: Two edited files
- **WHEN** `a.wav` and `b.wav` both have unsaved edits and the user closes the window
- **THEN** the app asks about `a.wav`, then about `b.wav`, and quits only after both are answered with Save or Don't Save

#### Scenario: Cancel quitting
- **WHEN** the user quits with unsaved edits and chooses Cancel at the first prompt
- **THEN** the app keeps running with every tab and edit intact

### Requirement: Export
File › Export… (Ctrl/Cmd+E) SHALL ask for a file name with a native save dialog filtered to WAV, defaulting to the WAV's folder and name with `-export` (`take.wav` → `take-export.wav`), and write the active tab's audio there, rendered through its edits and active effects. It SHALL be available for any loaded tab that isn't already exporting, even without edits.

#### Scenario: Export a trimmed copy
- **WHEN** the user crops `take1.wav` and exports it as `take1-trim.wav`
- **THEN** `take1-trim.wav` contains the cropped audio and `take1.wav` is unchanged

#### Scenario: Cancel the dialog
- **WHEN** the user opens Export and cancels
- **THEN** nothing is written and the tab is unchanged

### Requirement: Export never overwrites open audio
Export SHALL refuse a target that is the tab's own WAV or a WAV open in another tab, writing nothing and showing a notice that explains why.

#### Scenario: Export over the source
- **WHEN** the user chooses `take1.wav` as the export target for the `take1.wav` tab
- **THEN** nothing is written and a notice says the audio being edited can't be overwritten, suggesting another name

#### Scenario: Name of another open file
- **WHEN** the user chooses the path of a file that is open in another tab
- **THEN** nothing is written and a notice explains that the file is open in another tab

### Requirement: Export leaves the tab unchanged
After an export, successful or not, the tab SHALL refer to the same WAV and keep its edits, clip settings, effect chain, plugin settings, bypass states, undo history, session file, unsaved marker, view, selection and playhead. Its waveform and statistics SHALL keep describing the edited audio before effects. While exporting, the tab SHALL show the delayed progress indicator used for loads, and edits on it SHALL be disabled; other tabs stay usable.

#### Scenario: Keep working after export
- **WHEN** the user adds an effect, fades in 1.0–2.0 s and exports
- **THEN** the effect is still in the chain with its settings, the fade is still there, 1.0–2.0 s is still selected, and Undo removes the fade

#### Scenario: Progress while rendering
- **WHEN** the user exports a long edited file
- **THEN** the delayed progress indicator shows how far the export has got, and other tabs stay usable

### Requirement: Safe exporting
Exporting SHALL NOT leave a partial or corrupted file at the target path. If writing fails (for example the disk is full or access is denied), the system SHALL show an error notice and leave any existing file at the target unchanged. The tab SHALL be unchanged whether the export succeeds or fails.

#### Scenario: Read-only target
- **WHEN** the user exports into a read-only folder
- **THEN** an error notice explains that it could not be exported, and the tab keeps its edits, effects and unsaved marker

#### Scenario: Replacing an earlier export
- **WHEN** the user exports over `take-export.wav` from an earlier export and confirms replacing it in the dialog
- **THEN** `take-export.wav` is replaced only once the new file is complete

### Requirement: Clamped samples are reported
When exporting to an integer format clamps any samples that the active effects pushed past full scale, the system SHALL show a notice after the export saying how many samples were clipped. Float formats SHALL NOT be clamped, so exporting to them SHALL NOT show this notice.

#### Scenario: Too much gain
- **WHEN** a 16-bit file peaking at −3 dBFS is exported with a +6 dB gain effect
- **THEN** the file is exported, and a notice says that some samples were clipped to full scale and how many

#### Scenario: Float file keeps overs
- **WHEN** the same file in 32-bit float format is exported with the same effect
- **THEN** samples above full scale are written unclamped and no notice is shown
