# Spec Delta

## ADDED Requirements

### Requirement: Clamped samples are reported
When exporting to an integer format clamps any samples that the active effects pushed past full scale, the system SHALL show a notice after the export saying how many samples were clipped. Float formats SHALL NOT be clamped, so exporting to them SHALL NOT show this notice.

#### Scenario: Too much gain
- **WHEN** a 16-bit file peaking at −3 dBFS is exported with a +6 dB gain effect
- **THEN** the file is exported, and a notice says that some samples were clipped to full scale and how many

#### Scenario: Float file keeps overs
- **WHEN** the same file in 32-bit float format is exported with the same effect
- **THEN** samples above full scale are written unclamped and no notice is shown
