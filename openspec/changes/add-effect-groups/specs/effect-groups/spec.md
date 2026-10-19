## Purpose

Lets the user organise a tab's effect chain into named groups of neighbouring effects that can be collapsed, bypassed, moved and removed as one, without changing how the audio is processed: a grouped chain sounds exactly like the same effects ungrouped, unless the group is bypassed.

## ADDED Requirements

### Requirement: Selecting effects
The user SHALL be able to select effects in the Effects panel: a click selects one effect, Ctrl/Cmd+click adds or removes one effect from the selection, and Shift+click selects every effect between the last clicked one and the clicked one. Clicking a group's header SHALL select all of its effects. Selected effects SHALL be visibly marked. Selection SHALL NOT change the chain, and SHALL NOT be an undo step or mark the tab as unsaved.

#### Scenario: Range selection
- **WHEN** the chain is EQ, Compressor, Reverb, Limiter and the user clicks EQ then Shift+clicks Reverb
- **THEN** EQ, Compressor and Reverb are selected and Limiter is not

#### Scenario: Toggle one
- **WHEN** EQ and Compressor are selected and the user Ctrl/Cmd+clicks Compressor
- **THEN** only EQ is selected

### Requirement: Clearing the selection
Escape SHALL clear the selected effects. While no effect is selected, Escape SHALL clear the waveform selection as before.

#### Scenario: Escape clears the selection
- **WHEN** EQ and Compressor are selected, part of the waveform is selected, and the user presses Escape
- **THEN** no effect is selected and the waveform selection is unchanged; a second Escape clears the waveform selection

### Requirement: Group effects
Group Effects (Ctrl/Cmd+G, the Effects panel, and Effects › Group Effects) SHALL put the selected effects into a new group named "Group N", using the lowest N not already used by a group in that tab. The new group SHALL take the place of the first selected effect, and its effects SHALL keep their relative order. Processing SHALL follow the order shown, so grouping effects that weren't neighbours changes the processing order. Group Effects SHALL be disabled when no effect is selected.

#### Scenario: Group neighbours
- **WHEN** the chain is EQ, De-esser, Compressor, Reverb, the user selects De-esser and Compressor, and presses Ctrl/Cmd+G
- **THEN** the chain shows EQ, then "Group 1" holding De-esser and Compressor, then Reverb, and playback processes EQ, De-esser, Compressor, Reverb in that order as before

#### Scenario: Group effects that aren't neighbours
- **WHEN** the chain is EQ, Compressor, Reverb and the user selects EQ and Reverb and groups them
- **THEN** the chain shows "Group 1" holding EQ then Reverb, followed by Compressor

#### Scenario: Nothing selected
- **WHEN** no effect is selected
- **THEN** Group Effects is greyed out in the panel and the Effects menu

### Requirement: Groups hold only effects
Groups SHALL NOT contain other groups. Selected effects that are already in a group SHALL leave it and join the new group, and a group left with no effects SHALL disappear. When the new group's place is inside another group that keeps some effects, the new group SHALL go just before that group if none of its remaining effects came before the first selected effect, and just after it otherwise.

#### Scenario: Group across groups
- **WHEN** "Group 1" holds EQ and Compressor, and the user selects Compressor and the ungrouped Reverb after the group and groups them
- **THEN** "Group 1" holds EQ, and a new "Group 2" after it holds Compressor and Reverb

#### Scenario: Whole group regrouped
- **WHEN** "Group 1" holds EQ and Compressor, and the user selects both and the ungrouped Reverb after the group and groups them
- **THEN** "Group 1" is gone, and "Group 1" is created anew holding EQ, Compressor and Reverb

### Requirement: Ungroup
Ungroup (Ctrl/Cmd+Shift+G, the group header, and Effects › Ungroup) SHALL remove the group that holds the focused or selected effects, leaving its effects in the chain at the group's position, in the same order, with their own bypass switches and settings. Ungrouping a bypassed group SHALL leave its effects playing according to their own bypass switches. Ungroup SHALL be disabled when no selected or focused effect is in a group.

#### Scenario: Ungroup in place
- **WHEN** the chain is EQ, then "Group 1" holding De-esser and Compressor, then Reverb, and the user ungroups "Group 1"
- **THEN** the chain is EQ, De-esser, Compressor, Reverb with no groups

### Requirement: Group header
A group SHALL show as a header followed by its effects, indented. The header SHALL show a collapse arrow, the group's name, how many effects it holds, a bypass switch, and a remove button. Collapsing a group SHALL hide its effects in the panel without changing processing. Groups SHALL start expanded when created or when a session is opened, and collapsing SHALL NOT be an undo step, SHALL NOT mark the tab as unsaved, and SHALL NOT be saved in the session.

#### Scenario: Collapse
- **WHEN** the user collapses "Group 1", which holds three effects
- **THEN** its effects are hidden, the header says it holds 3 effects, and playback is unchanged

### Requirement: Rename a group
The user SHALL be able to rename a group by double-clicking its name, pressing F2 on its focused header, or pressing Ctrl/Cmd+R (also Effects › Rename Group) while its effects are highlighted, then typing a name and pressing Enter, or Escape to cancel. A name SHALL be 1 to 64 characters after trimming spaces; an empty name SHALL keep the old one. Group names need not be unique.

#### Scenario: Rename
- **WHEN** the user double-clicks "Group 1", types "Vocal cleanup" and presses Enter
- **THEN** the header shows "Vocal cleanup"

#### Scenario: Rename the highlighted group
- **WHEN** the user clicks the header of "Group 1", presses Ctrl/Cmd+R, types "Drums" and presses Enter
- **THEN** the header shows "Drums", and the app did not reload

#### Scenario: Empty name
- **WHEN** the user clears the name and presses Enter
- **THEN** the group keeps its previous name

### Requirement: Move a group
Dragging a group's header SHALL move the whole group, with its effects in order, to the drop position, and Alt+Up and Alt+Down on a focused header SHALL move the group past the item before or after it. A group SHALL NOT be dropped inside another group. Processing SHALL follow the order shown.

#### Scenario: Move a group
- **WHEN** the chain is EQ, then "Group 1" holding De-esser and Compressor, and the user drags the "Group 1" header above EQ
- **THEN** the chain is "Group 1" (De-esser, Compressor) then EQ, and playback processes De-esser, Compressor, EQ

### Requirement: Move effects into and out of a group
Dragging an effect among a group's effects SHALL make it a member, and dragging a member outside the group SHALL take it out. Alt+Up on a group's first effect, or Alt+Down on its last, SHALL move it out of the group to just before or just after the group; elsewhere Alt+Up and Alt+Down move it within the group.

#### Scenario: Drag into a group
- **WHEN** "Group 1" holds EQ and Compressor and the user drags the ungrouped Reverb between them
- **THEN** "Group 1" holds EQ, Reverb, Compressor in that order

#### Scenario: Keyboard out of a group
- **WHEN** Compressor is the last effect in "Group 1" and is focused, and the user presses Alt+Down
- **THEN** Compressor is just after "Group 1" and no longer in it

### Requirement: Remove a group
Removing a group from its header SHALL remove the group and all of its effects, closing their controls. To keep the effects, the user SHALL ungroup instead.

#### Scenario: Remove a group
- **WHEN** "Group 1" holds EQ and Compressor with EQ's editor open, and the user removes "Group 1"
- **THEN** the chain no longer has the group, EQ or Compressor, and EQ's editor is closed

### Requirement: Group bypass
Each group SHALL have its own bypass switch. An effect SHALL be processed, in playback and when exporting, only if neither Bypass All, its group's bypass, nor its own bypass is on. Group bypass SHALL NOT change the effects' own bypass switches, which the panel SHALL still show, and SHALL keep their settings. Like other bypass switches, it SHALL NOT be an undo step and SHALL mark the tab as unsaved.

#### Scenario: Bypass a group, then restore
- **WHEN** "Group 1" holds an active EQ and a bypassed Compressor, and the user turns the group bypass on and then off
- **THEN** while it is on, neither effect processes audio; after it is off, the EQ processes audio again and the Compressor is still bypassed

#### Scenario: Export with a bypassed group
- **WHEN** the chain is "Group 1" (bypassed) holding a reverb, followed by an active EQ, and the user exports
- **THEN** the exported audio has the EQ applied and not the reverb

### Requirement: Group changes are undoable and unsaved
Grouping, ungrouping, renaming a group, moving a group, moving an effect into or out of a group, and removing a group SHALL each be one step in the tab's undo history, alongside edits and other chain changes, and SHALL each mark the tab as having unsaved changes. Undoing the removal of a group SHALL restore the group with its name, bypass switch and effects, at its position, with each effect's settings.

#### Scenario: Undo removing a group
- **WHEN** "Vocal cleanup" holds an EQ at +6 dB, the user removes the group and presses Undo
- **THEN** "Vocal cleanup" is back in its place holding the EQ at +6 dB

#### Scenario: Undo grouping
- **WHEN** the user groups EQ and Compressor and presses Undo
- **THEN** EQ and Compressor are back ungrouped in their previous order, and Redo groups them again

### Requirement: Groups in sessions
Saving a session SHALL store each group's name, bypass switch and effects, and opening the session SHALL restore them, expanded. When restoring leaves out some of a group's effects (see "Effects that can't be restored" in session-files), the group SHALL keep the rest; a group with none left SHALL NOT be restored.

#### Scenario: Round trip with a group
- **WHEN** a tab has EQ, then "Vocal cleanup" (bypassed) holding De-esser and Compressor, and the user saves its session, closes the tab and opens the session
- **THEN** the chain shows EQ then "Vocal cleanup", bypassed, holding De-esser and Compressor with their settings

#### Scenario: Group member missing
- **WHEN** a session's "Vocal cleanup" group holds HD Test Gain and an effect that is no longer installed
- **THEN** the tab's "Vocal cleanup" holds only HD Test Gain, and the notice names the missing effect
