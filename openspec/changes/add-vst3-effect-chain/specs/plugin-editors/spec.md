# Spec Delta

## Purpose

Shows the controls of an effect in a chain, either as the plugin's own editor in a separate window or as generic parameter controls in the main window. Changes made there are heard straight away.

## ADDED Requirements

### Requirement: Native editor window
Opening an effect's controls SHALL show the plugin's own editor in a separate window titled "<effect name> — <file name>", sized to the editor. The window SHALL stay above the main window, without blocking it, and SHALL be resizable only if the plugin allows. At most one editor window SHALL be open per effect; opening it again SHALL bring the window to the front.

#### Scenario: Open an editor
- **WHEN** the user clicks the controls button of a compressor on `a.wav`
- **THEN** a window titled "Compressor — a.wav" shows the plugin's editor, and the main window stays usable

#### Scenario: Open twice
- **WHEN** the compressor's editor is already open and the user clicks its controls button again
- **THEN** the existing window comes to the front and no second window opens

#### Scenario: Plugin resizes its editor
- **WHEN** the plugin enlarges its editor (for example, to show an advanced panel)
- **THEN** the window grows to fit it

### Requirement: Editor windows follow their tab
Editor windows SHALL be hidden while their tab is not the active tab and shown again, at the same place, when it is. They SHALL close when their effect is removed, when their tab closes, when a save clears the chain, or when the app quits.

#### Scenario: Switch tabs
- **WHEN** `a.wav` has its EQ editor open and the user switches to `b.wav`
- **THEN** the EQ editor is hidden, and it reappears where it was when the user switches back

#### Scenario: Remove the effect
- **WHEN** the user removes an effect whose editor is open
- **THEN** its editor window closes

### Requirement: Generic controls
For effects without their own editor, opening the controls SHALL show generic controls in the main window. These SHALL list each automatable parameter with its name, value as text including units, and a slider. Changing a slider SHALL change the parameter. Double-clicking a slider SHALL reset it to its default.

#### Scenario: Plugin without an editor
- **WHEN** the user opens the controls of an effect that has no editor
- **THEN** generic controls list its parameters, such as "Threshold  −18.0 dB", with sliders

#### Scenario: Reset a parameter
- **WHEN** the user double-clicks the Threshold slider
- **THEN** the threshold returns to the plugin's default

### Requirement: Generic controls on request
Each effect SHALL offer "Show generic controls" in addition to its own editor, so that any plugin can be adjusted from the main window. Changes made in either place SHALL show in the other.

#### Scenario: Both views agree
- **WHEN** a plugin's editor and its generic controls are both open and the user turns a knob in the editor
- **THEN** the matching slider in the generic controls moves to the new value

### Requirement: Editor not available
When a plugin's own editor can't be shown, opening its controls SHALL show the generic controls instead, with a note saying why the editor isn't available. This applies when the plugin has no editor for this platform, when the editor fails to open, or when no X11 display is available on Linux.

#### Scenario: Linux without X11
- **WHEN** the app runs on a Linux Wayland session without XWayland and the user opens a plugin's controls
- **THEN** generic controls are shown with a note that plugin editors need X11 or XWayland

### Requirement: Parameter changes are live
Parameter changes made in an editor or in the generic controls SHALL take effect in playback within 0.25 s, as described in the effect-chain capability, and SHALL apply to the next Save. They SHALL NOT add undo steps.

#### Scenario: Turn a knob, then save
- **WHEN** the user sets an EQ band to +3 dB in its editor, closes the editor and saves
- **THEN** the saved audio has the +3 dB band applied, and Undo is greyed out after the save
