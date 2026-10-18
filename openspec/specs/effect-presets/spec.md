# effect-presets Specification

## Purpose

Lets the user save a group of effects, with every plugin's settings, as a named preset kept by the app, and insert it into any tab's chain as a new group, so a chain built once can be reused on other recordings.

## Requirements

### Requirement: Save a group as a preset
Save as Preset (a button on a group's header, or Effects › Save Group as Preset… for the highlighted group) SHALL ask for a name, defaulting to the group's name, and save the group's effects as a preset: each effect in order with its plugin, its complete saved settings and its own bypass switch. A name SHALL be 1 to 64 characters after trimming. Saving SHALL NOT change the tab, its history or its unsaved marker. Save Group as Preset… SHALL be disabled when no group is highlighted.

#### Scenario: Save a group
- **WHEN** "Vocal cleanup" holds an EQ with a +6 dB band and a bypassed compressor, and the user saves it as a preset, keeping the name
- **THEN** a preset "Vocal cleanup" holds the EQ with the +6 dB band and the compressor marked bypassed, and the tab shows no new unsaved changes

#### Scenario: Effect can't save its settings
- **WHEN** one effect in the group fails to report its settings while the user saves the preset
- **THEN** no preset is saved and a notice names the effect whose settings could not be saved

### Requirement: Replacing a preset
Saving a preset under a name that another preset already has, ignoring case, SHALL ask whether to replace it. Replacing SHALL overwrite that preset; declining SHALL leave both the library and the name prompt as they were, so the user can choose another name.

#### Scenario: Name taken
- **WHEN** a preset "Master bus" exists and the user saves a group as "master bus"
- **THEN** the app asks whether to replace "Master bus", and only replaces it if the user agrees

### Requirement: Preset library
Presets SHALL be kept by the app in its own data folder, not in files the user chooses, and SHALL be available in every tab and after restarting. Effects › Insert Preset SHALL list them by name, sorted alphabetically ignoring case, with a note when there are none. Writing the library SHALL replace its file only once the new contents are complete, and a library that can't be read SHALL be treated as empty, with a notice, without being overwritten until the next save.

#### Scenario: Available after restarting
- **WHEN** the user saves "Vocal cleanup", quits and starts the app again
- **THEN** Effects › Insert Preset lists "Vocal cleanup"

#### Scenario: No presets
- **WHEN** no presets have been saved
- **THEN** Effects › Insert Preset shows a greyed-out "No presets" item

### Requirement: Insert a preset
Choosing a preset in Effects › Insert Preset SHALL add its effects, with their saved settings and own bypass switches, as a new group named after the preset, not bypassed, at the end of the active tab's chain, as one undo step that marks the tab as unsaved. The inserted group SHALL NOT be linked to the preset. Insert Preset SHALL be disabled while no tab can take effects.

#### Scenario: Insert into another tab
- **WHEN** `b.wav` has no effects and the user inserts "Vocal cleanup"
- **THEN** `b.wav`'s chain is the group "Vocal cleanup" holding the EQ with its +6 dB band and the bypassed compressor, and one Undo removes it

#### Scenario: Not linked
- **WHEN** the user inserts "Vocal cleanup", then changes the inserted EQ's band and inserts "Vocal cleanup" again
- **THEN** the second group's EQ has the +6 dB band from the preset

### Requirement: Preset effects that can't be inserted
When inserting a preset, an effect whose plugin is not found, is disabled, does not support the file's channels, or fails to load SHALL be left out, and the rest SHALL be inserted in their saved order. A plugin that rejects its saved settings SHALL be inserted with its defaults. One warning notice SHALL list each affected effect with the reason. If no effect can be inserted, the chain SHALL stay as it was and the notice SHALL say that the preset could not be inserted.

#### Scenario: Stereo-only effect on a 5.1 file
- **WHEN** a preset holds HD Test Gain and HD Test Stereo and the user inserts it into a 5.1 tab
- **THEN** the new group holds only HD Test Gain, and the notice names HD Test Stereo and says it doesn't support 5.1 audio

### Requirement: Manage presets
Effects › Manage Presets… SHALL open a dialog listing the presets with how many effects each holds. The user SHALL be able to rename a preset, with the same name rules and replace question as saving, and delete one after confirming. Changes SHALL show in Effects › Insert Preset at once. Renaming or deleting a preset SHALL NOT change any tab.

#### Scenario: Delete
- **WHEN** the user deletes "Master bus" in Manage Presets and confirms
- **THEN** "Master bus" is gone from the dialog and from Effects › Insert Preset, and groups inserted from it earlier are unchanged
