## MODIFIED Requirements

### Requirement: Add an effect
Add Effect (Effects panel button, or Effects › Add Effect…) SHALL open a browser of available effects that can be searched by name and vendor. Choosing one SHALL add it to the end of the active tab's chain, with the plugin's default settings, and open its controls. The system SHALL NOT limit how many effects a chain holds.

#### Scenario: Add an EQ
- **WHEN** the user opens Add Effect, types "eq" and chooses an EQ plugin
- **THEN** the EQ is the last effect in the chain and its editor opens

#### Scenario: Long chain
- **WHEN** a tab already has 8 effects and the user opens Add Effect and chooses another
- **THEN** Add Effect was enabled, and the new effect becomes the ninth in the chain

#### Scenario: Long chain restored from a session
- **WHEN** the user opens a session whose chain holds 12 effects, all installed and enabled
- **THEN** the tab's chain holds all 12 in their saved order, with no notice about the chain being full
