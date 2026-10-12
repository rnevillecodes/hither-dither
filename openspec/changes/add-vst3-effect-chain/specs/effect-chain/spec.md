# Spec Delta

## Purpose

Lets each open file run its edited audio through an ordered chain of VST3 effects. The chain is heard live in playback and applied only when the file is saved, while the waveform and statistics keep describing the audio before effects.

## ADDED Requirements

### Requirement: Effects panel
While a loaded tab is active, the details panel SHALL show an Effects section that lists the tab's effects in processing order. Each effect SHALL show its name and have controls to bypass it, open its controls, and remove it. With no effects, the section SHALL show an Add Effect button and a short hint. Each tab SHALL have its own chain.

#### Scenario: Empty chain
- **WHEN** a file is opened
- **THEN** its Effects section has no effects and offers Add Effect

#### Scenario: Chains are per tab
- **WHEN** the user adds a compressor to `a.wav` and switches to `b.wav`
- **THEN** `b.wav`'s Effects section is empty, and playing `b.wav` is not processed by the compressor

### Requirement: Add an effect
Add Effect (Effects panel button, or Effects › Add Effect…) SHALL open a browser of available effects that can be searched by name and vendor. Choosing one SHALL add it to the end of the active tab's chain, with the plugin's default settings, and open its controls. A chain SHALL hold at most 8 effects; Add Effect SHALL be disabled when it is full.

#### Scenario: Add an EQ
- **WHEN** the user opens Add Effect, types "eq" and chooses an EQ plugin
- **THEN** the EQ is the last effect in the chain and its editor opens

#### Scenario: Full chain
- **WHEN** a tab already has 8 effects
- **THEN** Add Effect is greyed out for that tab, and its tooltip says the chain is full

### Requirement: Channel compatibility
An effect SHALL only be added if it can process the file's channel layout on its main input and output, with the same number of channels in and out. In the browser, effects that can't SHALL be greyed out with the reason. A plugin that fails to load SHALL leave the chain unchanged and show an error notice.

#### Scenario: Stereo-only plugin on a 5.1 file
- **WHEN** a 5.1 file is active and the user opens Add Effect
- **THEN** a plugin that only supports stereo is greyed out with "Doesn't support 5.1 audio"

#### Scenario: Plugin fails to load
- **WHEN** the chosen plugin reports an error while it is being set up
- **THEN** a notice says the effect could not be loaded, and the chain is unchanged

### Requirement: Remove and reorder effects
The user SHALL be able to remove an effect, which also closes its controls. The user SHALL be able to move an effect earlier or later in the chain by dragging it, or with the keyboard using Alt+Up and Alt+Down on the focused effect. Processing SHALL follow the order shown.

#### Scenario: Reorder
- **WHEN** the chain is EQ then Compressor and the user drags Compressor above EQ
- **THEN** the chain is Compressor then EQ, and playback processes the audio through the compressor first

#### Scenario: Keyboard reorder
- **WHEN** the second effect is focused and the user presses Alt+Up
- **THEN** it becomes the first effect

### Requirement: Bypass
Each effect SHALL have a bypass switch, and the chain SHALL have Bypass All (also Effects › Bypass All Effects). A bypassed effect, or every effect while Bypass All is on, SHALL pass audio through unchanged in playback and when saving. Bypassing SHALL keep each effect's settings and SHALL NOT be an undo step.

#### Scenario: A/B compare
- **WHEN** a selection is looping with an EQ in the chain and the user turns Bypass All on and off
- **THEN** playback switches between unprocessed and processed audio within 0.25 s each time, without stopping

#### Scenario: Save with a bypassed effect
- **WHEN** the chain has a bypassed reverb and an active EQ, and the user saves
- **THEN** the saved audio has the EQ applied and not the reverb

### Requirement: Chain changes are undoable
Adding, removing and reordering effects SHALL each be one step in the tab's undo history, alongside edits. Undoing the removal of an effect SHALL restore it at its position with the settings it had when removed. An effect's parameter changes SHALL NOT be undo steps.

#### Scenario: Undo a removal
- **WHEN** the user sets an EQ's gain to +6 dB, removes the EQ and presses Undo
- **THEN** the EQ is back in its place with +6 dB gain

#### Scenario: Edits and effects in one history
- **WHEN** the user applies Silence, adds a limiter, and presses Undo twice
- **THEN** the first Undo removes the limiter and the second removes the silence

### Requirement: Unsaved marker with effects
A tab SHALL be marked as having unsaved changes while its chain has at least one effect that is not bypassed, even with no edits. It SHALL count as unchanged when every effect is bypassed, or Bypass All is on, and it has no edits. Closing or quitting with an unsaved chain SHALL ask to save as for other unsaved changes.

#### Scenario: Effect alone marks the tab
- **WHEN** the user adds an EQ to an unedited file
- **THEN** the tab shows the unsaved marker, and closing it asks whether to save

#### Scenario: Everything bypassed
- **WHEN** an unedited file's only effect is bypassed
- **THEN** the tab has no unsaved marker, and Save is greyed out

### Requirement: Effects are heard during playback
Playback of a tab SHALL run its edited audio through its active effects in order, at the file's sample rate. Adding, removing, reordering or bypassing an effect, or changing a parameter, SHALL be heard within 0.25 s without stopping playback. The playhead SHALL stay in time with what is heard, compensating for the effects' processing delay.

#### Scenario: Adjust while listening
- **WHEN** a file is playing and the user turns up a compressor's ratio in its editor
- **THEN** the change is heard within 0.25 s and playback does not stop

#### Scenario: Effect with latency
- **WHEN** the chain contains a lookahead limiter with 5 ms latency and the user plays from 2.0 s
- **THEN** the audio heard first is the audio at 2.0 s, and the playhead matches what is heard

### Requirement: Effects not drawn or analysed
The waveform, overview, clip markers, and whole-file and selection statistics SHALL describe the edited audio without effects. Changing the chain or an effect's parameters SHALL NOT redraw the waveform or recompute statistics. While a tab has active effects, the Effects section SHALL say that the waveform and statistics don't include them.

#### Scenario: Gain plugin
- **WHEN** a file peaks at −6 dBFS and the user adds a plugin adding +12 dB of gain
- **THEN** the statistics still report a −6 dBFS peak, and the waveform is unchanged

#### Scenario: After saving
- **WHEN** the user saves that file
- **THEN** the waveform and statistics describe the saved, processed audio

### Requirement: Effects without an audio device
When no audio output is available, effects SHALL still be added, edited and applied by saving. Only hearing them SHALL be unavailable.

#### Scenario: No output device
- **WHEN** no audio device is available and the user adds an EQ and saves
- **THEN** the saved file has the EQ applied

### Requirement: Effects too heavy for real time
If the chain can't process audio fast enough for real-time playback, so that playback drops out, the system SHALL show a notice once per playback saying the effects are too demanding to play smoothly. Saving SHALL still render the effects correctly.

#### Scenario: Overloaded chain
- **WHEN** playback underruns repeatedly because the chain is too slow
- **THEN** a notice says the effects are too demanding to play smoothly, and later Save writes the fully processed audio

### Requirement: Closing a tab releases its effects
Closing a tab SHALL close its effects' editor windows and unload its effects, including any kept for undo.

#### Scenario: Close with editors open
- **WHEN** a tab with two open plugin editors is closed with Don't Save
- **THEN** both editor windows close
