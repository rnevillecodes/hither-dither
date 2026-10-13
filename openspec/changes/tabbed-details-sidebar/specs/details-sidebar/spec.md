## ADDED Requirements

### Requirement: Activity bar
While files are open, the window SHALL show an activity bar: a narrow vertical strip at the right edge of the window holding one icon button per sidebar view, in this order: Metrics, then Effects. The Metrics button SHALL use a bar-chart icon and the Effects button a plug icon. Each button SHALL have a tooltip and an accessible name giving the view's name. The button for the view currently shown SHALL be visibly highlighted with the accent colour.

#### Scenario: Buttons identify their views
- **WHEN** a file is open and the user hovers the plug icon
- **THEN** a tooltip names the Effects view

#### Scenario: Active view is highlighted
- **WHEN** the sidebar is showing the Metrics view
- **THEN** the bar-chart button is highlighted and the plug button is not

### Requirement: Hidden with no files open
While no files are open, neither the activity bar nor the sidebar SHALL be shown, so the empty screen uses the full window width.

#### Scenario: No files open
- **WHEN** no files are open
- **THEN** no activity bar and no sidebar are shown

### Requirement: Sidebar views
The sidebar SHALL show exactly one view at a time, beside the waveform and to the left of the activity bar. The Metrics view SHALL contain the File metadata and Levels statistics for the active tab. The Effects view SHALL contain the active tab's effect chain with all its existing controls. Each view SHALL show its name as a heading at the top. Switching views SHALL NOT change the active tab, the selection, playback, or the effect chain.

#### Scenario: Metrics view contents
- **WHEN** a loaded tab is active and the Metrics view is shown
- **THEN** the sidebar shows the File and Levels sections and no Effects section

#### Scenario: Effects view contents
- **WHEN** a loaded tab is active and the Effects view is shown
- **THEN** the sidebar shows the effect chain, with Add Effect, bypass, controls, remove and reorder available as before, and no File or Levels section

#### Scenario: Playback continues while switching
- **WHEN** audio is playing and the user switches from Metrics to Effects
- **THEN** playback continues uninterrupted

### Requirement: Switching and collapsing
Clicking the button of a view that is not shown SHALL open the sidebar on that view. Clicking the button of the view that is shown SHALL collapse the sidebar, and the waveform SHALL widen into the freed space; the activity bar SHALL stay visible. The buttons SHALL be operable from the keyboard.

#### Scenario: Switch view
- **WHEN** the Metrics view is shown and the user clicks the plug button
- **THEN** the sidebar shows the Effects view

#### Scenario: Collapse by clicking the active button
- **WHEN** the Effects view is shown and the user clicks the plug button
- **THEN** the sidebar is hidden, the waveform fills the space up to the activity bar, and no button is highlighted

#### Scenario: Reopen from collapsed
- **WHEN** the sidebar is collapsed and the user clicks the bar-chart button
- **THEN** the sidebar opens on the Metrics view

#### Scenario: Keyboard
- **WHEN** the user tabs to the plug button and presses Enter or Space
- **THEN** it behaves as a click

### Requirement: Effects count badge
The Effects button SHALL show a badge with the number of effects in the active tab's chain whenever that chain has at least one effect, whether or not the sidebar is open. With no effects, no badge SHALL be shown.

#### Scenario: Badge counts effects
- **WHEN** the active tab's chain holds 3 effects
- **THEN** the plug button shows a badge reading 3

#### Scenario: Badge follows the active tab
- **WHEN** `a.wav` has 2 effects, `b.wav` has none, and the user switches to `b.wav`
- **THEN** the plug button shows no badge

### Requirement: Adding an effect reveals the Effects view
When an effect is added to the active tab's chain by any route, the sidebar SHALL be open on the Effects view afterwards, so the new effect is visible.

#### Scenario: Add Effect from the menu while collapsed
- **WHEN** the sidebar is collapsed and the user adds an effect via Effects › Add Effect…
- **THEN** the sidebar opens on the Effects view showing the new effect

#### Scenario: Add Effect from the menu while Metrics is shown
- **WHEN** the Metrics view is shown and the user adds an effect via Effects › Add Effect…
- **THEN** the sidebar switches to the Effects view showing the new effect

### Requirement: Sidebar state is remembered
Which view is selected and whether the sidebar is open SHALL stay the same across tab switches and app restarts. On first launch the sidebar SHALL start open on the Metrics view. If the user had hidden the previous details panel, the sidebar SHALL start collapsed after upgrading.

#### Scenario: Remembered across restart
- **WHEN** the user shows the Effects view, restarts the app and opens a file
- **THEN** the sidebar is open on the Effects view

#### Scenario: Collapsed state remembered
- **WHEN** the user collapses the sidebar, switches tabs, then restarts the app and opens a file
- **THEN** the sidebar stays collapsed until the user clicks a button

#### Scenario: First launch
- **WHEN** the app is launched for the first time and a file is opened
- **THEN** the sidebar is open on the Metrics view

#### Scenario: Upgrade with the panel hidden
- **WHEN** the user had hidden the details panel in the previous version and launches this version
- **THEN** the sidebar starts collapsed

### Requirement: Narrow windows
When the window is too narrow to place the sidebar beside the waveform, the sidebar SHALL appear below the waveform and the activity bar SHALL become a horizontal strip above the sidebar, keeping the same buttons and behaviour.

#### Scenario: Narrow window
- **WHEN** the window is narrower than the side-by-side breakpoint and the Metrics view is open
- **THEN** the buttons appear in a row above the Metrics view, which sits below the waveform
