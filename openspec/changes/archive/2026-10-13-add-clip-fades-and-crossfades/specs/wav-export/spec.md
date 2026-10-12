# Spec Delta

## MODIFIED Requirements

### Requirement: Output format
Saved files SHALL be RIFF WAVE files containing exactly a `fmt ` chunk identical to the source's and a `data` chunk, with no other chunks. Sample format, bit depth, valid bits, sample rate, channel count and channel mask SHALL match the source. Samples that no edit, fade or crossfade changed SHALL be written bit-for-bit as in the source. Changed integer samples, including blended crossfade samples, SHALL be rounded to the nearest value and clamped to the format's range.

#### Scenario: Untouched audio is bit-exact
- **WHEN** the user deletes 1.0–2.0 s from a 24-bit file and saves it
- **THEN** every sample of the saved file outside the crossfade at the join equals the source sample at the corresponding position

#### Scenario: Hard join is bit-exact
- **WHEN** the user deletes 1.0–2.0 s from a 24-bit file, drags the join's crossfade to zero and saves it
- **THEN** every sample of the saved file equals the source sample at the corresponding position

#### Scenario: Metadata is not carried over
- **WHEN** the source has `LIST` and `bext` chunks and the user saves an edit
- **THEN** the saved file has only `fmt ` and `data` chunks, and HitherDither reopens it with the same format details

#### Scenario: Result too large for WAV
- **WHEN** the edited audio would make the `data` chunk larger than 4 GiB minus the header
- **THEN** Save and Save As fail with a notice saying the result is too large, and nothing is written
