# Spec Delta

## MODIFIED Requirements

### Requirement: Locating the WAV
A session SHALL record the WAV's absolute path and its path relative to the session file. Opening a session SHALL use the absolute path if that file exists, otherwise the relative path. If neither exists, a dialog SHALL name the missing WAV and offer Choose Another WAV… and Cancel; Cancel SHALL open nothing, and choosing a WAV SHALL open the session with it as in "Choosing another WAV".

#### Scenario: Folder moved
- **WHEN** `mix.hdsession.json` and `take.wav` were saved side by side, the folder is moved, and the session is opened from its new location
- **THEN** `take.wav` is opened from the new folder

#### Scenario: WAV missing
- **WHEN** the WAV a session refers to has been deleted and the user opens the session, then chooses Cancel
- **THEN** a dialog says the session's audio file `take.wav` could not be found, and after Cancel no tab is left open

### Requirement: WAV must match the session
After the WAV loads, the system SHALL compare its sample rate, channel count and layout, sample format and frame count with those recorded in the session. If any differ, the session SHALL NOT be applied, the tab SHALL be closed, and a dialog SHALL say the audio file no longer matches and offer Choose Another WAV… and Cancel. If only the size or modification time differ, the session SHALL be applied and a warning notice SHALL say the audio file has changed since the session was saved.

#### Scenario: WAV re-rendered shorter
- **WHEN** the WAV was overwritten with a shorter recording after the session was saved, and the user opens the session and chooses Cancel
- **THEN** the session is not applied, its tab closes, and the dialog said the audio file no longer matches the session

#### Scenario: WAV touched but same format
- **WHEN** the WAV's modification time changed but its format and length are the same
- **THEN** the session is restored and a warning says the audio file has changed since the session was saved

## ADDED Requirements

### Requirement: Choosing another WAV
When the user chooses another WAV for a broken session, the system SHALL open it in a new tab and restore the session onto it. If the chosen WAV has the recorded sample rate, channel layout, sample format and length, the edits SHALL be restored too. Otherwise the session's effects SHALL be restored as for Replace Audio, without its edits, and a notice SHALL say the edits were left out. The tab SHALL keep the session file and show the unsaved marker.

#### Scenario: Renamed WAV
- **WHEN** `take.wav` was renamed to `take-final.wav`, the user opens its session, chooses Choose Another WAV… and picks `take-final.wav`
- **THEN** the tab shows `take-final.wav` with the session's edits and effects, and the unsaved marker

#### Scenario: Different recording
- **WHEN** the session's WAV no longer matches and the user picks a WAV of a different length
- **THEN** the tab shows that WAV unedited with the session's effects and settings, and a notice says the edits were left out because the audio is different

#### Scenario: Saving points the session at the new WAV
- **WHEN** after choosing another WAV the user saves
- **THEN** the session file refers to the chosen WAV, and reopening it needs no dialog

#### Scenario: Chosen WAV already open
- **WHEN** the WAV the user picks is already open in another tab
- **THEN** that tab becomes active, nothing else opens, and a notice says to close it first
