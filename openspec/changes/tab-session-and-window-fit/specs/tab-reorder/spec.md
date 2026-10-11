# Spec Delta

## Purpose

Lets users arrange their open-file tabs in whatever order suits them by dragging tabs along the tab strip, and keeps every order-dependent tab behaviour consistent with that arrangement.

## ADDED Requirements

### Requirement: Drag a tab to a new position
The system SHALL let the user press on a tab and drag it horizontally along the tab strip. While dragging, the system SHALL show where the tab will land. Releasing the pointer SHALL move the tab to that position; other tabs keep their relative order.

#### Scenario: Move the last tab to the front
- **WHEN** tabs `a`, `b`, `c` are open and the user drags `c` to the left of `a` and releases
- **THEN** the tabs are shown in the order `c`, `a`, `b`

#### Scenario: Drop indicator while dragging
- **WHEN** the user is dragging tab `a` over the gap between `b` and `c`
- **THEN** a visible marker shows that `a` will be placed between `b` and `c`

### Requirement: Dragging does not disturb the files
Moving a tab SHALL NOT reload its file, change which tab is active, stop playback, or alter any tab's view state, selection or playhead. Loading tabs SHALL be movable too.

#### Scenario: Reorder during playback
- **WHEN** `a.wav` is active and playing and the user drags the `a.wav` tab to a new position
- **THEN** playback continues uninterrupted and `a.wav` stays active

#### Scenario: Reorder an inactive tab
- **WHEN** `b.wav` is active and the user drags the `a.wav` tab past `b.wav`
- **THEN** the order changes, `b.wav` stays active, and `a.wav` keeps its selection and zoom

### Requirement: Click and close still work
A press-and-release on a tab without meaningful movement SHALL behave as a normal click (activating the tab), and the close button and middle-click close SHALL keep working; a press on the close button SHALL NOT start a drag.

#### Scenario: Small jitter is a click
- **WHEN** the user presses on an inactive tab, moves the pointer a couple of pixels, and releases
- **THEN** that tab becomes active and the tab order is unchanged

### Requirement: Cancel a drag
Pressing Escape during a drag, or releasing the pointer outside the tab strip, SHALL cancel the drag and leave the tab order unchanged.

#### Scenario: Escape cancels
- **WHEN** the user is dragging tab `a` and presses Escape
- **THEN** the drag ends and the tabs keep their original order

### Requirement: Order-dependent behaviour follows the new order
After a reorder, Ctrl+Tab / Ctrl+Shift+Tab cycling, the choice of tab to activate after closing the active tab, and the remembered session SHALL all use the tab order as currently displayed.

#### Scenario: Cycle after reordering
- **WHEN** tabs were opened as `a`, `b`, `c`, the user drags `c` to the front (giving `c`, `a`, `b`), `c` is active, and the user presses Ctrl+Tab
- **THEN** `a` becomes active

#### Scenario: Close after reordering
- **WHEN** the tabs are `c`, `a`, `b`, `a` is active, and the user closes `a`
- **THEN** `b` becomes active
