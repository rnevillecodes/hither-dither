# Spec Delta

## Purpose

Lets users swap a tab's WAV for another recording while keeping its effect chain, plugin settings and session file, so the same processing can be applied to a different take or a new render.

## ADDED Requirements

### Requirement: Replace Audio
File › Replace Audio… SHALL ask for a WAV with a native open dialog and replace the active tab's audio with it. It SHALL be enabled while the active tab is loaded and not exporting. While the new WAV loads, the tab SHALL show the normal loading progress.

#### Scenario: Replace with another take
- **WHEN** the `take1.wav` tab has two effects and the user replaces its audio with `take2.wav`
- **THEN** the tab shows `take2.wav` with both effects in the same order and with the same settings

#### Scenario: Cancel the dialog
- **WHEN** the user opens Replace Audio and cancels
- **THEN** the tab is unchanged

### Requirement: What the tab keeps
After replacing the audio, the tab SHALL keep its effect chain in order with every effect's settings and bypass switch, Bypass All, and its session file. The tab SHALL drop its edits, clip settings and joins, start a new undo history at the new audio, and clear its selection and playhead. Effect editors SHALL be closed.

#### Scenario: Edits are dropped
- **WHEN** a tab with a crop and a fade-in has its audio replaced
- **THEN** the new audio is shown unedited, Undo is disabled, and the effects are kept

#### Scenario: Session follows the new audio
- **WHEN** a tab opened from `mix.hdsession.json` has its audio replaced with `take2.wav` and the user saves
- **THEN** `mix.hdsession.json` refers to `take2.wav` and holds the same effects

### Requirement: Confirm before dropping edits
If the tab has edits or anything to undo, Replace Audio SHALL first ask for confirmation, saying that the edits and undo history will be discarded and the effects kept. Cancelling SHALL leave the tab unchanged.

#### Scenario: Confirmation shown
- **WHEN** the tab has a fade and the user chooses `take2.wav` in Replace Audio
- **THEN** a dialog asks to confirm discarding the edits; Cancel leaves the tab as it was

#### Scenario: No confirmation without edits
- **WHEN** the tab has effects but no edits and nothing to undo
- **THEN** the audio is replaced without a confirmation

### Requirement: Effects that don't fit the new audio
When the new WAV has a channel layout an effect can't process, that effect SHALL be left out and the others kept in order. One warning notice SHALL name each effect left out and why, as when restoring a session.

#### Scenario: Stereo-only effect, mono take
- **WHEN** a stereo tab with HD Test Gain then HD Test Stereo has its audio replaced with a mono WAV
- **THEN** the chain is HD Test Gain alone and a notice says HD Test Stereo doesn't support mono audio

### Requirement: Replaced audio is unsaved
After a replacement, the tab SHALL show the unsaved marker until its session is saved, and closing or quitting SHALL ask to save as for any unsaved change.

#### Scenario: Marked unsaved
- **WHEN** the session of a tab was just saved and the user replaces its audio
- **THEN** the tab shows the unsaved marker

### Requirement: Refused replacements
Replace Audio SHALL refuse, leaving the tab unchanged and showing a notice, when the chosen file is the tab's current WAV, is open in another tab, or can't be opened as a WAV.

#### Scenario: Open in another tab
- **WHEN** the user chooses a WAV that is open in another tab
- **THEN** nothing changes and a notice says it is open in another tab

#### Scenario: Not a WAV
- **WHEN** the user chooses a file that can't be read as a WAV
- **THEN** the tab keeps its audio, edits and effects, and a "Could not replace the audio" notice gives the reason
