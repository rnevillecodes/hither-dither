# Spec Delta

## Purpose

Lets users open WAV files from disk and have them decoded into audio the app can display, analyse and play, with clear feedback when a file cannot be used.

## ADDED Requirements

### Requirement: Open a file via dialog
The system SHALL provide an "Open" action (menu item and keyboard shortcut Ctrl/Cmd+O) that shows a native file dialog filtered to `.wav` files and loads the chosen file.

#### Scenario: User opens a WAV file from the dialog
- **WHEN** the user triggers Open and selects a valid WAV file
- **THEN** the file is loaded and its waveform, metadata and stats are displayed

#### Scenario: User cancels the dialog
- **WHEN** the user triggers Open and cancels the dialog
- **THEN** the currently loaded file (if any) remains loaded and unchanged

### Requirement: Open a file via drag-and-drop
The system SHALL load a WAV file dropped onto the application window. If multiple files are dropped, the system SHALL load only the first and inform the user that only one file is opened at a time.

#### Scenario: User drops a WAV file
- **WHEN** the user drops a single valid WAV file onto the window
- **THEN** the file is loaded, replacing any previously loaded file

#### Scenario: User drops several files
- **WHEN** the user drops more than one file onto the window
- **THEN** only the first file is loaded and a notice states that only one file can be opened at a time

### Requirement: Supported sample formats
The system SHALL decode RIFF/WAVE files with these sample formats: unsigned 8-bit PCM, signed 16-, 24- and 32-bit integer PCM, and 32- and 64-bit IEEE float. It SHALL accept both `WAVE_FORMAT_PCM`/`WAVE_FORMAT_IEEE_FLOAT` and `WAVE_FORMAT_EXTENSIBLE` headers whose subformat is PCM or IEEE float, with any channel count from 1 to 32 and any sample rate from 1 Hz to 768 kHz.

#### Scenario: 24-bit stereo file
- **WHEN** the user opens a 48 kHz, 24-bit, 2-channel PCM WAV file
- **THEN** the file loads with 2 channels and the decoded sample values match the file's samples to within 24-bit precision

#### Scenario: Extensible float multichannel file
- **WHEN** the user opens a `WAVE_FORMAT_EXTENSIBLE` file with an IEEE float subformat and 6 channels
- **THEN** the file loads with 6 channels

### Requirement: Tolerant chunk parsing
The system SHALL ignore unknown chunks (e.g. `LIST`, `bext`, `cue `) and handle odd-sized chunk padding. If the `data` chunk's declared size exceeds the bytes available, the system SHALL load the available whole frames and show a warning that the file appears truncated.

#### Scenario: File with metadata chunks
- **WHEN** the user opens a valid WAV file containing `LIST` and `bext` chunks before the `data` chunk
- **THEN** the file loads normally

#### Scenario: Truncated file
- **WHEN** the user opens a WAV file whose `data` chunk declares more bytes than the file contains
- **THEN** the available frames are loaded and a "file appears truncated" warning is shown

### Requirement: Reject unsupported or invalid files
The system SHALL refuse to load a file that is not a RIFF/WAVE file, lacks a `fmt ` or `data` chunk, or uses an unsupported encoding (e.g. ADPCM, µ-law, A-law, RF64), and SHALL show an error message naming the file and the reason. A failed load SHALL NOT crash the app or discard the currently loaded file.

#### Scenario: Non-WAV file
- **WHEN** the user opens a file that does not start with a RIFF/WAVE header
- **THEN** an error states the file is not a valid WAV file and the previously loaded file stays loaded

#### Scenario: Unsupported encoding
- **WHEN** the user opens an IMA ADPCM WAV file
- **THEN** an error states that the encoding is not supported, naming the encoding

### Requirement: Load progress and responsiveness
The system SHALL keep the UI responsive while loading and SHALL show a progress indicator for loads lasting longer than 500 ms. Files up to 2 GB (the largest standard WAV size the app guarantees) SHALL be loadable on a machine with 8 GB of RAM.

#### Scenario: Large file load
- **WHEN** the user opens a 1 GB WAV file
- **THEN** a progress indicator is shown and the window remains interactive (can be moved, menus open) until loading completes
