# Spec Delta

## Purpose

Lets users pick up where they left off: the files that were open when the app last closed are reopened automatically on the next launch, in the same tab order and with the same tab active.

## ADDED Requirements

### Requirement: Remember the open files
The system SHALL remember the paths of all open files, in tab order, and which tab is active. The remembered session SHALL be kept up to date as files are opened, closed, reordered or activated, so that it survives the app being quit, closed from the window, or terminated unexpectedly. Files whose load failed SHALL NOT be remembered.

#### Scenario: Session updated on open
- **WHEN** the user opens `a.wav` and `b.wav` and then quits the app
- **THEN** the remembered session lists `a.wav` then `b.wav`, with `b.wav` active

#### Scenario: Session updated on close
- **WHEN** `a.wav` and `b.wav` are open and the user closes `b.wav`, then the app is terminated
- **THEN** the remembered session lists only `a.wav`

#### Scenario: Failed load not remembered
- **WHEN** the user opens a file that cannot be parsed and its error notice is shown
- **THEN** that file is not part of the remembered session

### Requirement: Reopen the last session on launch
When the app starts and a remembered session lists one or more files, the system SHALL reopen each file in its own tab, in the remembered order, following the normal loading behaviour. Once loading finishes, the remembered active tab SHALL be active if it loaded; otherwise the normal "last file to finish loading" rule applies.

#### Scenario: Restore two files
- **WHEN** the app was closed with `a.wav` and `b.wav` open and `a.wav` active, and the app is launched again
- **THEN** tabs for `a.wav` and `b.wav` appear in that order and, after loading, `a.wav` is active

#### Scenario: Nothing to restore
- **WHEN** the app was closed with no files open (or has never been run)
- **THEN** the app starts on the empty "Open a WAV file" screen

### Requirement: Missing or unreadable files on restore
If a remembered file no longer exists or fails to load, the system SHALL skip it, show the normal "Could not open file" notice for it, and still restore the remaining files. The skipped file SHALL be dropped from the remembered session.

#### Scenario: One file deleted between runs
- **WHEN** the remembered session lists `a.wav` and `b.wav`, `a.wav` has since been deleted, and the app is launched
- **THEN** a "Could not open file" notice names `a.wav`, `b.wav` opens in a tab and is active, and the next remembered session lists only `b.wav`

### Requirement: App stays usable while restoring
While remembered files are being reopened, the user SHALL be able to open further files, switch tabs and close tabs, exactly as during any other multi-file load.

#### Scenario: Close a restoring tab
- **WHEN** the app is restoring a large file and the user closes its tab
- **THEN** its load stops, no error is shown, and that file is not part of the remembered session

### Requirement: Restore scope
Restoring SHALL bring back only the files, their order and the active tab. Each restored file SHALL start with the default view: zoomed to fit, no selection, whole-file stats and the playhead at the start.

#### Scenario: Selection not restored
- **WHEN** the app was closed while `a.wav` had a 1.0–2.0 s selection and was zoomed in, and the app is launched again
- **THEN** `a.wav` reopens zoomed to fit with no selection and whole-file stats
