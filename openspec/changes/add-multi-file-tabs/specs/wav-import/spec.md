# Spec Delta

## MODIFIED Requirements

### Requirement: Open a file via dialog
The system SHALL provide an "Open" action (menu item File › Open… and keyboard shortcut Ctrl/Cmd+O) that shows a native file dialog filtered to `.wav` files, allows several files to be selected, and opens each chosen file in its own tab.

#### Scenario: User opens a WAV file from the dialog
- **WHEN** the user triggers Open and selects a valid WAV file
- **THEN** the file is loaded in a new active tab and its waveform, metadata and stats are displayed

#### Scenario: User opens several files from the dialog
- **WHEN** the user triggers Open and selects three valid WAV files
- **THEN** each file opens in its own tab, and files that were already open stay open

#### Scenario: User cancels the dialog
- **WHEN** the user triggers Open and cancels the dialog
- **THEN** the open tabs (if any) and the active tab remain unchanged

### Requirement: Open a file via drag-and-drop
The system SHALL open every file dropped onto the application window, each in its own tab, in the order they were dropped.

#### Scenario: User drops a WAV file
- **WHEN** the user drops a single valid WAV file onto the window while another file is open
- **THEN** the dropped file opens in a new active tab and the previously open file stays open in its tab

#### Scenario: User drops several files
- **WHEN** the user drops `u8-mono.wav` and `float64-mono.wav` onto the window together
- **THEN** both files open, each in its own tab, and no "one file at a time" notice is shown

### Requirement: Reject unsupported or invalid files
The system SHALL refuse to load a file that is not a RIFF/WAVE file, lacks a `fmt ` or `data` chunk, or uses an unsupported encoding (e.g. ADPCM, µ-law, A-law, RF64), and SHALL show an error message naming the file and the reason. A failed load SHALL NOT crash the app, SHALL NOT leave a tab for the failed file, and SHALL NOT close or change any other open tab. When several files are opened together, a failure SHALL NOT stop the others from loading.

#### Scenario: Non-WAV file
- **WHEN** the user opens a file that does not start with a RIFF/WAVE header
- **THEN** an error states the file is not a valid WAV file, no tab remains for it, and the previously open tabs stay open

#### Scenario: Unsupported encoding
- **WHEN** the user opens an IMA ADPCM WAV file
- **THEN** an error states that the encoding is not supported, naming the encoding

#### Scenario: One bad file among several
- **WHEN** the user drops `not-a-wav.wav` together with two valid WAV files
- **THEN** an error names `not-a-wav.wav`, and the two valid files each open in their own tab

## ADDED Requirements

### Requirement: No Open button in the toolbar
The toolbar SHALL NOT contain an **Open…** button. Files SHALL be opened through File › Open… (Ctrl/Cmd+O), drag-and-drop, or, while no file is open, the **Open…** button on the empty "Open a WAV file" screen.

#### Scenario: Nothing open
- **WHEN** no files are open
- **THEN** the toolbar has no Open… button, and the empty screen shows an Open… button that opens the dialog

#### Scenario: Files open
- **WHEN** a file has been opened
- **THEN** the toolbar has no Open… button, and File › Open… still opens the dialog
