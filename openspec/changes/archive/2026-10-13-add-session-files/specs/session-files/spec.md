# Spec Delta

## Purpose

Lets users save a tab's unsaved edits and effect chain, including every plugin's settings, to a session file that refers to the WAV, and later reopen it to carry on in exactly the same state without the WAV ever being rewritten.

## ADDED Requirements

### Requirement: Session file contents
A session file SHALL describe one tab: a reference to its WAV, the WAV's sample rate, channel count and layout, sample format and frame count as loaded, its size and modification time, the tab's current audio edits (segments, silence, fade curves, crossfades, trim window and clip fades), Bypass All, and each effect in chain order with its class ID, name, vendor, version, bypass switch and the plugin's complete saved state. Saving a session SHALL NOT write or change the WAV.

#### Scenario: Edits and effects captured
- **WHEN** a tab for `take.wav` has a delete with a crossfade, a fade-out, and two effects with the second bypassed, and the user saves its session
- **THEN** the session file lists the edits and both effects in order, with the second marked bypassed, and `take.wav` is unchanged on disk

#### Scenario: Clean tab
- **WHEN** a tab has no edits and no effects and the user saves its session
- **THEN** the session file is written with no edits and an empty chain

### Requirement: Plugin settings saved exactly
The stored state of each effect SHALL include every setting the plugin reports in its saved state, plus any parameter change made in its editor or generic controls that the plugin has not yet processed (for example a change made while playback is stopped).

#### Scenario: Change made while stopped
- **WHEN** playback is stopped, the user sets HD Test Gain to +6 dB in its generic controls, and saves the session without playing
- **THEN** reopening the session restores HD Test Gain at +6 dB

### Requirement: Save Session
The system SHALL provide File › Save Session (Ctrl/Cmd+Alt+S) and File › Save Session As…, enabled while the active tab is loaded and not saving. Save Session As SHALL ask for a location, defaulting to the WAV's folder and file name with the `.hdsession.json` extension. Save Session SHALL overwrite the tab's session file without asking once the tab has one, and otherwise behave as Save Session As.

#### Scenario: First save asks
- **WHEN** the user chooses Save Session in a tab that has never had a session file
- **THEN** a save dialog opens proposing `take.hdsession.json` next to `take.wav`

#### Scenario: Later saves overwrite
- **WHEN** the tab was saved to or opened from `mix.hdsession.json` and the user chooses Save Session
- **THEN** `mix.hdsession.json` is overwritten with the current state and no dialog is shown

#### Scenario: Name typed without the extension
- **WHEN** the user types `mix` or `mix.json` as the file name in the Save Session As dialog
- **THEN** the session is saved as `mix.hdsession.json`

#### Scenario: Cancel the dialog
- **WHEN** the user cancels the Save Session As dialog
- **THEN** no file is written and the tab's session file is unchanged

#### Scenario: Not available while loading
- **WHEN** the active tab is still loading or a save of its WAV is in progress
- **THEN** Save Session and Save Session As are disabled

### Requirement: Session write safety
Writing a session SHALL replace the target file only once the new contents are complete, so a failed write leaves any existing file intact. A failed write SHALL show a notice with the reason and leave the tab unchanged. If an effect fails to report its saved state, the session SHALL NOT be written and the notice SHALL name that effect.

#### Scenario: Plugin can't report its state
- **WHEN** one effect in the chain fails to report its saved state while the user saves the session
- **THEN** no file is written and a notice names the effect whose settings could not be saved

#### Scenario: Read-only target
- **WHEN** the user saves a session over a read-only file
- **THEN** a notice explains that the session could not be saved, and the existing file is unchanged

### Requirement: Open a session
File › Open… SHALL offer session files alongside WAV files, and dropping a `.hdsession.json` file on the window SHALL open it. Opening a session SHALL open its WAV in a new tab with the normal loading progress and then restore the edits, Bypass All and effects with their settings and bypass switches, in order. The tab SHALL remember the session file for Save Session. Effect editor windows SHALL start closed.

#### Scenario: Round trip
- **WHEN** the user saves a session for a tab with edits and HD Test Gain at +6 dB, closes the tab, and opens the session
- **THEN** a tab for the WAV opens with the same edits, clip handles and chain, HD Test Gain is at +6 dB, and playback and Save produce the same audio as before closing

#### Scenario: Mixed selection
- **WHEN** the user picks `a.wav` and `mix.hdsession.json` in the Open dialog
- **THEN** `a.wav` opens as usual and `mix.hdsession.json` opens its WAV with the session restored, each in its own tab

### Requirement: Session file name
Session files SHALL use the `.hdsession.json` extension. A file SHALL be treated as a session only when its name ends in `.hdsession.json`, ignoring case; other `.json` files SHALL be handled like any other file that isn't a WAV.

#### Scenario: Other JSON file dropped
- **WHEN** the user drops `notes.json` on the window
- **THEN** it is not opened as a session, and the usual "Could not open file" notice is shown

#### Scenario: Extension case
- **WHEN** the user opens `MIX.HDSESSION.JSON`
- **THEN** it is opened as a session

### Requirement: Restored history and unsaved marker
A restored session SHALL start a new undo history whose first state is the restored state, so Undo is unavailable until the next change. The tab's unsaved marker SHALL follow the existing rule: it is shown when the restored edits or an active effect change the audio.

#### Scenario: Nothing to undo after opening
- **WHEN** a session with edits is opened
- **THEN** Undo is disabled and the tab shows the unsaved marker

#### Scenario: Effects all bypassed
- **WHEN** a session with no edits and only bypassed effects is opened
- **THEN** the tab does not show the unsaved marker

### Requirement: Locating the WAV
A session SHALL record the WAV's absolute path and its path relative to the session file. Opening a session SHALL use the absolute path if that file exists, otherwise the relative path. If neither exists, the session SHALL NOT open and a "Could not open session" notice SHALL name the missing WAV.

#### Scenario: Folder moved
- **WHEN** `mix.hdsession.json` and `take.wav` were saved side by side, the folder is moved, and the session is opened from its new location
- **THEN** `take.wav` is opened from the new folder

#### Scenario: WAV missing
- **WHEN** the WAV a session refers to has been deleted
- **THEN** no tab is left open and a notice says the session's audio file `take.wav` could not be found

### Requirement: WAV must match the session
After the WAV loads, the system SHALL compare its sample rate, channel count and layout, sample format and frame count with those recorded in the session. If any differ, the session SHALL NOT be applied, the tab SHALL be closed, and a "Could not open session" notice SHALL say the audio file no longer matches. If only the size or modification time differ, the session SHALL be applied and a warning notice SHALL say the audio file has changed since the session was saved.

#### Scenario: WAV re-rendered shorter
- **WHEN** the WAV was overwritten with a shorter recording after the session was saved
- **THEN** the session is not applied, its tab closes, and the notice says the audio file no longer matches the session

#### Scenario: WAV touched but same format
- **WHEN** the WAV's modification time changed but its format and length are the same
- **THEN** the session is restored and a warning says the audio file has changed since the session was saved

### Requirement: WAV already open
If the WAV of a session being opened is already open in a tab, the system SHALL NOT open the session; it SHALL activate that tab and show a notice saying to close the tab first to open the session.

#### Scenario: Same WAV open
- **WHEN** `take.wav` is open and the user opens `mix.hdsession.json`, which refers to `take.wav`
- **THEN** the `take.wav` tab becomes active, its state is unchanged, and a notice explains that it must be closed to open the session

### Requirement: Effects that can't be restored
When restoring the chain, an effect whose plugin is not found, is disabled, does not support the file's channels, or fails to load SHALL be left out, and the remaining effects SHALL be restored in their saved order. A plugin that rejects its saved state SHALL stay in the chain at its default settings. One warning notice SHALL list each affected effect by name with the reason.

#### Scenario: Plugin uninstalled
- **WHEN** a session's chain is HD Test Gain then an effect that is no longer installed then HD Test Stereo
- **THEN** the tab's chain is HD Test Gain then HD Test Stereo, with their saved settings, and a notice names the missing effect

#### Scenario: Saved state rejected
- **WHEN** a plugin's saved state is corrupt and the plugin rejects it
- **THEN** the effect is in the chain at its default settings and the notice says its settings could not be restored

### Requirement: Invalid session files
A file that is not a valid session, or whose edits are inconsistent with the recorded frame count, SHALL NOT open and SHALL produce a "Could not open session" notice saying it is not a valid session file. A session written by a newer, incompatible format version SHALL produce a notice saying it was made by a newer version of HitherDither.

#### Scenario: Not a session
- **WHEN** the user opens a `.hdsession.json` file containing arbitrary text
- **THEN** no tab is opened and the notice says the file is not a valid session file

#### Scenario: Newer format
- **WHEN** the user opens a session whose format version is higher than the app supports
- **THEN** no tab is opened and the notice says it was made by a newer version of HitherDither
