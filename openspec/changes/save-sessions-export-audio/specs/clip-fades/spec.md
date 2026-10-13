# Spec Delta

## REMOVED Requirements

### Requirement: Clip settings when saving
**Reason**: Writing audio is now Export, which leaves the clip settings in place instead of resetting them.
**Migration**: Replaced by "Clip settings when exporting".

## ADDED Requirements

### Requirement: Clip settings when exporting
Exporting SHALL write the trimmed, faded and crossfaded audio. Afterwards the trim, the end fades, the joins and their crossfades, and the undo history SHALL be as they were before the export. Saving the session SHALL keep them too, so they come back when the session is reopened.

#### Scenario: Export a shaped take
- **WHEN** the user trims both ends, adds fades and exports
- **THEN** the exported file contains exactly the audio that was shown, and the handles still show the trims and fades

#### Scenario: Shaped take in a session
- **WHEN** the user trims both ends, adds fades, saves the session, closes the tab and reopens the session
- **THEN** the handles show the same trims and fades
