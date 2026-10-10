# Spec Delta

## Purpose

Gives users the technical facts about a loaded WAV file and quantitative level measurements, so they can check format, loudness and signal problems without external tools.

## ADDED Requirements

### Requirement: File metadata display
The system SHALL display, for the loaded file: file name, file size, encoding (PCM integer or IEEE float), bit depth, sample rate, channel count, total frame count, and duration (as h:mm:ss.mmm).

#### Scenario: Metadata shown after load
- **WHEN** the user loads a 44.1 kHz, 16-bit, stereo PCM file of 10 seconds
- **THEN** the panel shows encoding PCM, 16-bit, 44100 Hz, 2 channels, 441000 frames, duration 0:00:10.000

### Requirement: Per-channel level statistics
The system SHALL compute and display, for each channel: sample peak (dBFS and linear), RMS level (dBFS), DC offset (as a percentage of full scale), and crest factor (dB). Silence SHALL be shown as "-inf dBFS" rather than an error or numeric overflow.

#### Scenario: Full-scale sine wave
- **WHEN** the file contains a full-scale 1 kHz sine wave on one channel
- **THEN** that channel shows peak 0.0 dBFS (±0.1), RMS -3.0 dBFS (±0.1), DC offset 0.0% (±0.1), and crest factor 3.0 dB (±0.1)

#### Scenario: Silent channel
- **WHEN** a channel contains only zero samples
- **THEN** its peak and RMS are shown as "-inf dBFS"

### Requirement: Integrated loudness
The system SHALL compute integrated loudness in LUFS per ITU-R BS.1770-4 / EBU R128 (K-weighting with gating) over all channels for mono, stereo and 5.1 layouts, and SHALL show "n/a" with an explanatory tooltip for other channel layouts or when the measured range is shorter than 400 ms.

#### Scenario: Reference tone
- **WHEN** the file contains a stereo 1 kHz sine at -18 dBFS on both channels
- **THEN** integrated loudness is shown as -18.0 LUFS (±0.1)

#### Scenario: Too short to measure
- **WHEN** the measured range is 200 ms long
- **THEN** the loudness value shows "n/a"

### Requirement: Clipping detection
The system SHALL count, per channel, clipped runs: sequences of 3 or more consecutive samples at or beyond full scale (the maximum/minimum integer value for PCM, or |x| ≥ 1.0 for float). It SHALL display the count and SHALL mark clipped regions in the waveform view.

#### Scenario: Clipped audio
- **WHEN** a channel contains two separate runs of 5 consecutive samples at the maximum 16-bit value
- **THEN** that channel shows 2 clipped runs and both regions are marked in the waveform

#### Scenario: Isolated full-scale sample
- **WHEN** a channel contains a single full-scale sample surrounded by lower values
- **THEN** it is not counted as clipping

### Requirement: Selection-scoped statistics
The system SHALL compute level statistics, loudness and clipping for the whole file by default and, when a time selection exists, for the selected range, clearly labelling which scope is shown. Statistics SHALL update within 1 second after the selection changes for selections up to 10 minutes of stereo 48 kHz audio, and the UI SHALL remain responsive while computing.

#### Scenario: Stats follow selection
- **WHEN** the user selects a range containing only silence in an otherwise loud file
- **THEN** the panel is labelled "Selection" and shows "-inf dBFS" peak for each channel

#### Scenario: Clearing selection restores whole-file stats
- **WHEN** the user clears the selection
- **THEN** the panel is labelled "Whole file" and shows the whole-file values
