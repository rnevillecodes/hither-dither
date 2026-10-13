## ADDED Requirements

### Requirement: Meter placement
While files are open, the window SHALL show a level meter as a narrow vertical strip immediately to the right of the waveform: between the waveform and the sidebar when the sidebar is open, and between the waveform and the activity bar when it is collapsed. While no files are open, no meter SHALL be shown.

#### Scenario: Beside the sidebar
- **WHEN** a file is open and the sidebar is open
- **THEN** the meter sits between the waveform and the sidebar, running the height of the waveform area

#### Scenario: Sidebar collapsed
- **WHEN** the user collapses the sidebar
- **THEN** the meter stays visible, between the waveform and the activity bar

#### Scenario: No files open
- **WHEN** no files are open
- **THEN** no meter is shown

### Requirement: One bar per channel
The meter SHALL show one bar per channel of the active tab's file, labelled with the file's channel labels (as in the Levels table), in channel order. The meter SHALL widen to fit the channels.

#### Scenario: Stereo file
- **WHEN** a stereo file is active
- **THEN** the meter shows two bars labelled L and R

#### Scenario: 5.1 file
- **WHEN** a 5.1 file is active
- **THEN** the meter shows six bars labelled with the file's six channel labels

### Requirement: What is measured
During playback, the meter SHALL measure the active tab's audio after its effect chain, at the file's sample rate. It SHALL measure before the volume setting and before the audio is mapped to the output device's channels or resampled. Bypassed effects, and Bypass All, SHALL be reflected as they are heard. The levels shown SHALL be those of the audio being heard at that moment, compensated for output latency in the same way as the playhead.

#### Scenario: Effects are measured
- **WHEN** a mono file with peaks at −12 dBFS plays through an effect that adds 6 dB
- **THEN** the meter's peak reaches about −6 dBFS

#### Scenario: Bypass All
- **WHEN** the same file plays and the user turns on Bypass All
- **THEN** within 0.25 s the meter's peak returns to about −12 dBFS

#### Scenario: Volume does not move the meter
- **WHEN** the user lowers the volume slider to 50% during playback
- **THEN** the playback gets quieter but the meter's levels do not change

#### Scenario: In step with the sound
- **WHEN** `beep-every-second-mono-60s.wav` plays
- **THEN** each beep's rise on the meter coincides with hearing the beep and with the playhead crossing it on the waveform

### Requirement: Peak and RMS display
Each channel's bar SHALL show the peak level (the largest sample magnitude) as an outer bar and the RMS level as a narrower, brighter bar inside it, both on a dBFS scale. The scale SHALL run from −60 dBFS at the bottom to 0 dBFS at the top, with labelled marks at 0, −6, −12, −18, −24, −36 and −60 dBFS; levels below −60 dBFS SHALL show as empty. RMS SHALL be computed over the most recent 300 ms, with a full-scale sine reading −3 dBFS RMS, as in the Levels table.

#### Scenario: Full-scale sine
- **WHEN** a 0 dBFS sine plays with no effects
- **THEN** the peak bar reaches the top and the RMS bar settles at −3 dBFS within 0.5 s

#### Scenario: Silence
- **WHEN** a silent passage plays
- **THEN** both bars are empty

### Requirement: Level colours
Peak bar segments at or above −6 dBFS SHALL be drawn in the warning colour, and those below in the accent colour.

#### Scenario: Loud passage
- **WHEN** a passage peaking at −3 dBFS plays
- **THEN** the top of the peak bar above −6 dBFS is drawn in the warning colour

### Requirement: Ballistics
The peak bar SHALL rise immediately to a new peak and fall at 20 dB per second. The RMS bar SHALL follow its 300 ms average without further smoothing. Each channel SHALL show a peak-hold tick at the highest recent peak; it SHALL stay for 1.5 s, then fall at 20 dB per second unless a higher peak replaces it. The meter SHALL be redrawn at the display's frame rate.

#### Scenario: Peak falls back
- **WHEN** a single loud click at −1 dBFS is followed by silence
- **THEN** the peak bar jumps to −1 dBFS and falls to the bottom of the scale in about 3 s

#### Scenario: Peak hold
- **WHEN** the same click plays
- **THEN** the hold tick stays at −1 dBFS for 1.5 s before falling

### Requirement: Clip indication
Each channel SHALL have a clip light above its bar that turns on, in the danger colour, when a sample with a magnitude of 1.0 (0 dBFS) or more is measured. It SHALL stay on until cleared.

#### Scenario: Clipping after effects
- **WHEN** a file peaking at −3 dBFS plays through an effect that adds 6 dB
- **THEN** the clip light turns on for each affected channel and stays on after the passage ends

#### Scenario: No clipping
- **WHEN** a file peaking at −0.5 dBFS plays with no effects
- **THEN** no clip light turns on

### Requirement: Maximum peak readout
Above each channel, the meter SHALL show the highest peak measured since the readouts were last cleared, in dBFS to one decimal place, or "–∞" if nothing above silence has been measured. Starting playback, and clicking the meter, SHALL clear the readouts and clip lights.

#### Scenario: Readout after playing
- **WHEN** a passage peaking at −4.2 dBFS has played and playback is paused
- **THEN** the readout shows −4.2

#### Scenario: Clear by clicking
- **WHEN** a clip light is on and the user clicks the meter
- **THEN** the clip lights turn off and the readouts show "–∞"

#### Scenario: New playback clears
- **WHEN** the user starts playback again
- **THEN** the readouts and clip lights start afresh

### Requirement: Idle meter
While nothing is playing, the bars SHALL fall to empty with the same ballistics, and the maximum peak readouts and clip lights SHALL stay as they were. Switching tabs SHALL clear the meter and show the newly active file's channels. The meter SHALL NOT measure a tab other than the one playing.

#### Scenario: Pause
- **WHEN** the user pauses during a loud passage
- **THEN** the bars fall to empty while the readouts keep their values

#### Scenario: Switch tabs
- **WHEN** a stereo file has played and the user switches to a mono tab
- **THEN** the meter shows one empty bar with a "–∞" readout

### Requirement: Narrow windows
When the window is too narrow to place the sidebar beside the waveform, the meter SHALL run horizontally across the window between the waveform and the activity bar, with the same channels, scale, colours and behaviour, rising from left to right.

#### Scenario: Narrow window
- **WHEN** the window is narrower than the side-by-side breakpoint and a stereo file plays
- **THEN** two horizontal bars appear below the waveform and above the activity bar, moving with the audio

### Requirement: Accessible description
The meter SHALL have an accessible name of "Level meter". Its numeric readouts SHALL be available to assistive technology without being announced on every update. Clicking to clear SHALL also be possible from the keyboard.

#### Scenario: Keyboard clear
- **WHEN** the user tabs to the meter and presses Enter
- **THEN** the readouts and clip lights are cleared
