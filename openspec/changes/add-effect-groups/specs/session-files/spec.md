## MODIFIED Requirements

### Requirement: Session file contents
A session file SHALL describe one tab: a reference to its WAV, the WAV's sample rate, channel count and layout, sample format and frame count as loaded, its size and modification time, the tab's current audio edits (segments, silence, fade curves, crossfades, trim window and clip fades), Bypass All, each effect in chain order with its class ID, name, vendor, version, bypass switch and the plugin's complete saved state, and each effect group with its name, bypass switch and the effects it holds. A session with at least one group SHALL be written as format version 2; a session without groups SHALL be written as format version 1, so builds without groups can still open it. Saving a session SHALL NOT write or change the WAV.

#### Scenario: Edits and effects captured
- **WHEN** a tab for `take.wav` has a delete with a crossfade, a fade-out, and two effects with the second bypassed, and the user saves its session
- **THEN** the session file lists the edits and both effects in order, with the second marked bypassed, and `take.wav` is unchanged on disk

#### Scenario: Clean tab
- **WHEN** a tab has no edits and no effects and the user saves its session
- **THEN** the session file is written with no edits and an empty chain

#### Scenario: Groups captured
- **WHEN** a tab's chain is EQ, then "Vocal cleanup" (bypassed) holding De-esser and Compressor, and the user saves its session
- **THEN** the session file is format version 2 and records "Vocal cleanup" as bypassed, holding De-esser and Compressor, with the three effects in chain order

#### Scenario: No groups, old format
- **WHEN** a tab has effects but no groups and the user saves its session
- **THEN** the session file is format version 1

#### Scenario: Older build opens a grouped session
- **WHEN** a build without groups opens a session with a group
- **THEN** no tab is opened and the notice says it was made by a newer version of HitherDither
