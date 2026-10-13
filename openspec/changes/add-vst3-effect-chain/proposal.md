# Proposal

## Why

HitherDither can trim, fade and clean up a WAV file, but it can't apply any processing beyond gain changes. Users who want to EQ, compress, denoise or limit a file before saving have to go to a DAW or another editor. VST3 is the standard plugin format on Windows, macOS and Linux, and since VST 3.8 the SDK is MIT-licensed, which matches this project's licence. So the app can host the plugins people already own.

## What Changes

- **Plugin discovery:** the app finds VST3 effect plugins in the standard per-OS folders and in folders the user adds. It scans them in a separate process, so a plugin that crashes while being scanned can't bring down the app. Results are cached and rescanned only when a plugin changes. A Manage Plugins dialog lists every plugin with its status (ready, failed, incompatible, disabled) and lets the user add or remove folders, rescan, and enable or disable plugins.
- **Crash recovery:** if the app quits unexpectedly while a plugin was being loaded or shown, that plugin is disabled on the next launch, with a notice.
- **Per-tab effect chain:** each tab can have an ordered chain of up to 8 effects, shown in a new Effects section of the details panel. The user can add, remove, reorder and bypass effects, and bypass the whole chain to compare.
  - The chain is non-destructive: it is heard live in playback.
  - Adding, removing and reordering effects are undo steps.
  - An active chain marks the tab as unsaved.
- **Effects are heard, not drawn:**
  - Playback runs the edited audio through the chain in real time. Parameter changes are heard within 0.25 s, and the chain's latency is compensated so the playhead stays in sync.
  - The waveform, overview, clip markers and statistics keep describing the edited audio **before** effects. Effects never trigger re-analysis.
- **Saving renders the effects:**
  - Save and Save As render the edited audio through the chain offline, with latency compensated and the file length unchanged.
  - Afterwards the processed audio is the new starting point: the chain is cleared and its editors closed, like clip fades.
  - If rendering clamps samples in an integer format, a notice says how many.
- **Plugin editors:** each effect opens its own native editor in a separate window that stays above the main window. Plugins without an editor, or any plugin on request, get generic parameter controls in the main window. On Linux, native editors need an X11 display (X11 or XWayland). Without one, generic controls are used.
- **Platforms:** Windows, macOS (Apple Silicon and Intel; a plugin must include the running architecture) and Linux x64.
- **Out of scope:**
  - instruments and MIDI;
  - side-chain inputs;
  - VST2, AU, CLAP and LV2 plugins;
  - extending the file with effect tails;
  - host-side presets;
  - remembering chains across restarts;
  - drawing or analysing the processed audio.

## Capabilities

### New Capabilities
- `vst3-plugins`: finding, scanning, caching and managing VST3 effect plugins, including plugin folders, compatibility, and disabling plugins after a crash.
- `effect-chain`: the per-tab chain of effects: adding, removing, reordering and bypassing effects, undo, the unsaved marker, real-time playback through the chain, and how effects relate to what is drawn and analysed.
- `plugin-editors`: showing an effect's controls, either the plugin's native editor window or generic parameter controls, and how those windows behave.

### Modified Capabilities
- `wav-export`:
  - Output format: with active effects, samples are the processed values and not bit-exact copies.
  - After saving: the chain is cleared, its editors close, and rendering effects is part of the save progress.
  - Saving warns about clamped samples.

## Impact

- **Rust (`src-tauri`):**
  - New `plugins/` module covering VST3 module loading, the host-side COM interfaces, the scanner subprocess, the plugin cache and settings, and native editor windows per OS.
  - New `fx/` module for chain state, the live processing used by the playback feeder, and offline rendering.
  - Changes to `playback/engine.rs` (process chunks through the chain before resampling, compensate latency), `state.rs` (chain per file, chain changes in the undo history, dirtiness), `wav/writer.rs` (render through the chain) and `commands.rs`/`lib.rs` (new commands).
  - `main.rs` gains a `--vst3-scan` subprocess entry point.
- **Dependencies:**
  - VST3 API bindings (`vst3` crate, MIT/Apache-2.0) and `libloading`;
  - per-OS windowing: `windows` (Win32), `objc2`/`objc2-app-kit` (macOS), `x11rb` plus `glib` run-loop integration (Linux).
- **Frontend (`src`):**
  - New Effects panel, Add Effect browser, Manage Plugins dialog, generic parameter panel, and an Effects menu;
  - tab state gains the chain;
  - `api.ts` gains the plugin and chain commands.
- **Tests and CI:** a small in-repo VST3 test plugin (gain plus a fixed latency) is built during Rust tests on all three CI platforms.
- **Docs:** `docs/ipc.md` (new commands and events), `README.md` (plugin folders, the Linux X11 note), `docs/acceptance.md`.
- **Risk:** plugins run in-process for processing and editors, so a crashing plugin can still crash the app (this is mitigated by out-of-process scanning and crash recovery).
