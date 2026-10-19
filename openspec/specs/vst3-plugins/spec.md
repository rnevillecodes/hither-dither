# vst3-plugins Specification

## Purpose

Finds the VST3 effect plugins installed on the computer, checks whether each one can be used, and lets the user manage plugin folders and individual plugins. A misbehaving plugin can't stop the app from starting.

## Requirements

### Requirement: Standard plugin folders
The system SHALL look for VST3 plugins (`.vst3` bundles or files) in the platform's standard folders, including subfolders:
- **Windows:** `%COMMONPROGRAMFILES%\VST3` and `%LOCALAPPDATA%\Programs\Common\VST3`
- **macOS:** `/Library/Audio/Plug-Ins/VST3` and `~/Library/Audio/Plug-Ins/VST3`
- **Linux:** `~/.vst3`, `/usr/lib/vst3` and `/usr/local/lib/vst3`

Missing folders SHALL be skipped silently.

#### Scenario: Plugin in the user folder
- **WHEN** a VST3 effect is installed in `~/Library/Audio/Plug-Ins/VST3` on macOS and the app starts
- **THEN** that effect appears in the Add Effect list once scanning finishes

#### Scenario: Standard folder missing
- **WHEN** `/usr/local/lib/vst3` does not exist on Linux
- **THEN** scanning completes without any error or notice about it

### Requirement: User plugin folders
The Manage Plugins dialog SHALL let the user add and remove extra folders to scan. The standard folders SHALL be listed but SHALL NOT be removable. The extra folders SHALL be remembered across restarts. Adding or removing a folder SHALL start a rescan.

#### Scenario: Add a folder
- **WHEN** the user adds `D:\Plugins\VST3` in Manage Plugins
- **THEN** the effects in that folder appear once the rescan finishes, and the folder is still listed after restarting the app

#### Scenario: Remove a folder
- **WHEN** the user removes an extra folder
- **THEN** its plugins disappear from the Add Effect list, and effects already in a chain keep working until removed

### Requirement: Only effects are offered
Only plugin classes that process audio as effects SHALL be offered. Instruments and classes without an audio input and output SHALL NOT be listed in Add Effect. Manage Plugins SHALL show plugins without any effect class as having no usable effects. A bundle with several effect classes SHALL offer each one separately.

#### Scenario: Instrument plugin
- **WHEN** an installed VST3 contains only a synthesizer
- **THEN** it is not offered in Add Effect, and Manage Plugins shows it as having no usable effects

#### Scenario: Bundle with two effects
- **WHEN** one `.vst3` bundle provides an EQ class and a compressor class
- **THEN** both are offered in Add Effect as separate entries

### Requirement: Scanning in the background, isolated from the app
Plugins SHALL be scanned in the background after startup, without delaying opening files. Each plugin SHALL be examined in a separate process, so that a plugin that crashes or hangs while being scanned can't stop the app. A plugin that crashes, or takes longer than 20 seconds, SHALL be marked as failed with the reason.

#### Scenario: Crashing plugin during scan
- **WHEN** a plugin crashes when it is loaded for scanning
- **THEN** the app keeps running, scanning continues with the other plugins, and Manage Plugins shows that plugin as failed with "Crashed while being scanned"

#### Scenario: Hanging plugin
- **WHEN** a plugin never finishes loading
- **THEN** after 20 seconds it is marked as failed with "Timed out while being scanned", and the rest of the scan continues

#### Scenario: Open files while scanning
- **WHEN** the app has just started and is still scanning plugins
- **THEN** files can be opened, viewed, played and edited as normal

### Requirement: Scan results are cached
Scan results SHALL be remembered across restarts. On startup, only plugins that are new, or whose files changed size or modification time since the last scan, SHALL be scanned again. Plugins that were removed SHALL disappear from the lists. Manage Plugins SHALL offer Rescan All, which scans every plugin again.

#### Scenario: Second launch is quick
- **WHEN** the app starts with the same plugins installed as last time
- **THEN** every plugin is available straight away, without being scanned again

#### Scenario: Updated plugin
- **WHEN** a plugin is updated to a new version between launches
- **THEN** only that plugin is scanned again, and Manage Plugins shows its new version

### Requirement: Compatibility is shown
Manage Plugins SHALL list every plugin found with its name, vendor, version, location and status: Ready, Failed (with reason), Incompatible (with reason) or Disabled. A plugin SHALL be Incompatible when it has no binary for the running OS and processor, such as an Intel-only plugin on Apple Silicon. Incompatible and failed plugins SHALL NOT be offered in Add Effect.

#### Scenario: Intel-only plugin on Apple Silicon
- **WHEN** a plugin contains only an x86_64 binary and the app runs natively on an Apple Silicon Mac
- **THEN** Manage Plugins shows it as Incompatible with "No version for this Mac's processor", and Add Effect doesn't offer it

### Requirement: Enable and disable plugins
The user SHALL be able to disable a plugin in Manage Plugins, which removes it from Add Effect, and enable it again. Disabling a plugin SHALL NOT remove effects of it already in a chain. The disabled state SHALL be remembered across restarts.

#### Scenario: Disable a plugin
- **WHEN** the user disables a plugin and restarts the app
- **THEN** that plugin is listed as Disabled and is not offered in Add Effect

### Requirement: Plugins disabled after a crash
If the app quits unexpectedly while a plugin was being loaded into a chain or while its editor was being opened, the system SHALL disable that plugin on the next launch. The system SHALL show a notice naming it and saying it can be enabled again in Manage Plugins.

#### Scenario: Crash while adding an effect
- **WHEN** the app crashes while the user is adding a plugin to a chain, and the user starts the app again
- **THEN** a notice says the plugin was disabled because the app closed unexpectedly while loading it, and the plugin is listed as Disabled

#### Scenario: Re-enable after a crash
- **WHEN** the user enables that plugin again in Manage Plugins
- **THEN** it is offered in Add Effect again

### Requirement: Scan progress is visible
While a scan is running, Manage Plugins and the Add Effect browser SHALL show that plugins are being scanned and how many of how many are done. Plugins already scanned SHALL be usable meanwhile.

#### Scenario: First launch with many plugins
- **WHEN** the app starts for the first time with 200 plugins installed, and the user opens Add Effect straight away
- **THEN** the browser shows "Scanning plugins… n of 200" and lists the effects found so far, updating as the scan continues
