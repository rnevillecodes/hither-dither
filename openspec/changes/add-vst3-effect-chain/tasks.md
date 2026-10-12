# Tasks

## 1. Dependencies and test plugins

- [x] 1.1 Add the `vst3` bindings crate, pinned after checking its licence (MIT/Apache-2.0) and that it exposes `IRunLoop`, `IPlugViewContentScaleSupport` and the component and controller interfaces. Also add `libloading`, plus target-specific `windows`, `objc2`/`objc2-app-kit`/`core-foundation`, and `x11rb`/`glib` (matching the version Tauri uses) to `src-tauri/Cargo.toml`. Verify `cargo build` succeeds on the host and `npm run lint:rust` passes.
- [x] 1.2 Turn `src-tauri` into a Cargo workspace with `test-plugins/hd-test-fx` (HD Test Gain: a dB gain parameter, a 64-sample delay reported as latency, 1–8 channels, no editor; HD Test Stereo: stereo only; HD Test Synth: an instrument class) and the `hd-test-crash` and `hd-test-hang` bundles from design.md. Add a test helper that builds them and assembles `.vst3` bundle directories in a temp folder. Verify with a Rust test that the helper produces bundles with the per-OS `Contents/<arch>-<os>/` layout, and that `npm run test:rust` passes.

## 2. Loading and driving a plugin

- [x] 2.1 Implement `plugins/module.rs` (per-OS bundle loading, entry and exit calls, factory access, process-lifetime module cache) and `plugins/main_thread.rs` (`run` a closure on the Tauri main thread and wait for it). Verify with a test that loads the HD Test Gain bundle and lists its three classes with names, vendor and subcategories.
- [x] 2.2 Implement the host objects in `plugins/host.rs` (`IHostApplication`, `IComponentHandler`, `IPlugFrame`, parameter changes and value queues, an empty event list, memory `IBStream`, `IMessage`/`IAttributeList`). Implement `plugins/instance.rs`:
  - create and initialise the component and controller, then connect them;
  - set the bus arrangement from the WAV channel layout, deactivate non-main buses;
  - set up processing, query latency and get/set state.

  Add tests:
  - HD Test Gain at +6 dB on stereo doubles samples, delayed by its reported 64-sample latency;
  - HD Test Stereo refuses a 5.1 arrangement;
  - a state round trip restores the gain.

  Verify `npm run test:rust` passes.

## 3. Plugin discovery and management

- [ ] 3.1 Add the `--vst3-scan <bundle>` mode to `main.rs` and `plugins/scan.rs`. It enumerates effect classes only, probes the layouts listed in design.md, records whether each class has an editor, and prints JSON. The parent side adds the architecture check, up to 4 concurrent scans, a 20 s timeout (overridable in tests) and crash detection. Add tests:
  - HD Test Synth is not offered;
  - HD Test Stereo's layouts are stereo only;
  - `hd-test-crash` is "Crashed while being scanned";
  - `hd-test-hang` times out while the other plugins still scan.

  Verify `npm run test:rust` passes.
- [ ] 3.2 Implement `plugins/cache.rs`:
  - standard folders per OS and extra folders;
  - `plugins.json`, keyed by path with size and modification time;
  - `plugin-settings.json` (extra folders, disabled class IDs);
  - the incremental rescan and Rescan All;
  - background scanning started at app startup, emitting `plugins-changed` with progress.

  Add tests:
  - a second scan of unchanged bundles launches no subprocess;
  - touching one bundle rescans only it;
  - removed bundles disappear;
  - a missing standard folder is skipped silently.

  Verify `npm run test:rust` passes.
- [ ] 3.3 Implement the crash marker (`plugin-loading.json` written around first-time instance creation and view opening) and startup handling that disables the class and queues a notice. Add the commands `list_plugins`, `plugin_folders`, `set_plugin_folders`, `rescan_plugins`, `set_plugin_enabled` and `take_startup_notices`. Add tests: a leftover marker disables that class and yields one notice; enabling it again clears the disabled entry. Document the commands and the `plugins-changed` event in `docs/ipc.md`. Verify `npm run lint:rust` and `npm run test:rust` pass.

## 4. Chain state, history and dirtiness

- [ ] 4.1 Add `fx/chain.rs` (slots, order, per-slot bypass via `kIsBypass` or host skip, `bypass_all`, total latency). Extend `LoadedFile` with slots and chain, and change history entries to `Snapshot { edits, chain }`. Release unreferenced slots on the UI thread when history is truncated or cleared and when the file closes. Update the existing history tests in `tests/state.rs` so they still pass. Verify `npm run test:rust` passes.
- [ ] 4.2 Add the commands `get_chain`, `add_effect` (channel compatibility check, cap of 8, crash marker), `remove_effect`, `move_effect`, `set_effect_bypass` and `set_bypass_all`. They return `EditOutcome` with `chain` and `audioChanged: false`, and the revision is unchanged. Use the new dirtiness rule. Add `tests/state.rs` cases for these spec scenarios:
  - "Undo a removal" (same gain after undo);
  - "Edits and effects in one history";
  - "Effect alone marks the tab";
  - "Everything bypassed";
  - "Full chain";
  - "Stereo-only plugin on a 5.1 file";
  - a chain change does not start a background analysis.

  Document the commands and the `EditOutcome` fields in `docs/ipc.md`. Verify `npm run lint:rust` and `npm run test:rust` pass.

## 5. Hearing the chain in playback

- [ ] 5.1 Implement `fx/live.rs` and the feeder integration:
  - the `fx` snapshot slot in `Shared`, with planar buffers;
  - `process` per chunk at the file rate, before resampling;
  - delivery of pending parameter changes at offset 0;
  - discarding `L` frames after the start and after each flush;
  - feeding `L` frames of silence at the end of the range.

  Add engine tests that drive `Engine::render` with HD Test Gain:
  - output is the gained input with no latency offset;
  - the reported positions match;
  - looping keeps alignment across the wrap;
  - playback ends exactly at the range end.

  Verify `npm run test:rust` passes.
- [ ] 5.2 Make chain changes (add, remove, move, bypass, bypass all) install a new snapshot and flush through the retarget path. Reset active slots on `play`, and recompute latency on `restartComponent(kLatencyChanged)`. Limit ring fill to 0.15 s with an active chain. Send `overload` on the play channel after three or more mid-range underruns within 2 s. Add tests:
  - after bypass-all, the rendered output equals the dry audio within one flush;
  - a parameter change reaches the output within 0.25 s of audio;
  - a synthetic slow slot triggers exactly one `overload`.

  Document `overload` in `docs/ipc.md`. Verify `npm run test:rust` passes.

## 6. Saving through the chain

- [ ] 6.1 Split `stats::analyse` into a streaming accumulator plus the existing driver, and change the writer to take a `NativeFrames` source, with `Audio` implementing it. Verify that all existing stats, writer and `tests/state.rs` save tests still pass unchanged with `npm run test:rust`.
- [ ] 6.2 Implement `fx/render.rs` and the chain path in `save_inner`:
  - skip the dry analysis;
  - switch slots to offline mode and deliver pending parameters;
  - render with latency removed and the length unchanged;
  - quantise, counting clamped samples, and analyse in the same pass with single-pass progress;
  - on success, clear the chain and close its editors; on failure, restore realtime mode and keep everything.

  Return `clampedSamples`. Add `tests/state.rs` cases for these spec scenarios:
  - "Effects applied with latency removed" (16-bit, +6 dB, 64-sample latency, same length, doubled samples);
  - "Bypassed chain is bit-exact";
  - "Too much gain" (clamped count above 0);
  - "Float file keeps overs";
  - "Effects are baked in" (empty chain, stats of the processed audio, no pending analysis);
  - "Save with effects fails" (read-only target keeps the chain and settings).

  Update `docs/ipc.md` for `save_file`. Verify `npm run lint:rust` and `npm run test:rust` pass.

## 7. Plugin editors

- [ ] 7.1 Implement `plugins/editor/win.rs` (an owned Win32 window, `kPlatformTypeHWND`, `resizeView` and user resize, DPI scaling) and the shared editor manager:
  - one window per slot, titled "<effect> — <file>";
  - bring an open window to the front instead of opening another;
  - closing the window emits `fx-editor-closed`;
  - close on remove, tab close, save and quit;
  - `set_active_file` hides and shows editors per tab, remembering positions.

  Add `open_editor` and `close_editor` returning `{ native, reason? }`. Add Windows steps to `docs/acceptance.md` and check them by hand with a free plugin that has a resizable editor.
- [ ] 7.2 Implement `plugins/editor/mac.rs` (an `NSWindow` as a child window of the main window, `kPlatformTypeNSView`, resizing). Add macOS steps to `docs/acceptance.md` and check them by hand on Apple Silicon, including an Intel-only plugin showing as Incompatible.
- [ ] 7.3 Implement `plugins/editor/x11.rs`:
  - an `x11rb` window with `WM_TRANSIENT_FOR` when GTK is on X11, using `kPlatformTypeX11EmbedWindowID`;
  - the `IRunLoop` file descriptors and timers, and the X connection, as GLib sources on the main context;
  - "no X11 display" reported as an unavailable reason.

  Add Linux steps to `docs/acceptance.md`, including a Wayland session with and without XWayland, and check them by hand. Verify `npm run lint:rust` passes on Linux CI.

## 8. Generic controls backend

- [ ] 8.1 Add `get_fx_params` (automatable, visible, writable parameters excluding bypass, with display strings) and `set_fx_param` (controller plus the processor's pending map, returning the new display string). Emit `fx-param-changed` (throttled to 30 Hz per slot) for changes from native editors, and `fx-params-reset` on parameter restarts. Apply output parameter changes to the controller on a UI-thread timer. Add tests with HD Test Gain:
  - the gain parameter is listed with units "dB" and its default;
  - setting it changes the processed output on the next block;
  - `set_fx_param` adds no undo step.

  Document the commands and events in `docs/ipc.md`. Verify `npm run test:rust` passes.

## 9. Frontend

- [ ] 9.1 Add the plugin, chain, editor and parameter commands and events to `api.ts`. Add `chain` to `Tab` and handle `audioChanged: false` outcomes in `run()` without invalidating peaks or waiting for analysis. Call `setActiveFile` on tab switches. Show notices for startup crash-disables, `clampedSamples` after a save, and `overload` (once per playback). Verify `npm run check` and `npm test` pass, with vitest cases for the outcome handling that leaves `peaksVersion` and `statsPending` unchanged.
- [ ] 9.2 Add `EffectsPanel.svelte` at the top of the details panel:
  - rows with name, bypass, controls and remove buttons;
  - drag reorder, plus Alt+Up and Alt+Down on a focused row;
  - Bypass All;
  - the empty-state hint;
  - a note that the waveform and statistics don't include effects;
  - Add Effect disabled with a tooltip when the chain is full.

  Put the pure reorder and index logic in `src/lib/` with vitest tests. Verify `npm run check` and `npm test` pass, and check the "Reorder", "Keyboard reorder" and "A/B compare" scenarios by hand.
- [ ] 9.3 Add `EffectBrowser.svelte`: search by name and vendor, grouped by vendor, incompatible effects greyed out with the reason (layout matched against the file's channels), and "Scanning plugins… n of m" progress. Choosing an effect adds it and opens its controls. Put the filtering and compatibility logic in `src/lib/` with vitest tests (search matching, the 5.1 file greys out stereo-only effects, failed and disabled plugins hidden). Verify `npm run check` and `npm test` pass.
- [ ] 9.4 Add `PluginManager.svelte`:
  - the plugin list with name, vendor, version, location, status and reason;
  - enable and disable;
  - standard folders shown read-only and extra folders added with the folder dialog or removed;
  - Rescan All and progress.

  Check the "Add a folder", "Remove a folder", "Disable a plugin" and "Re-enable after a crash" scenarios by hand. Verify `npm run check` passes.
- [ ] 9.5 Add `GenericControls.svelte`, a floating, non-modal panel:
  - sliders with display text and units;
  - double-click to reset;
  - follows `fx-param-changed` and `fx-params-reset`;
  - a note when the native editor is unavailable;
  - a "Show generic controls" entry on each effect.

  Check the "Both views agree" and "Reset a parameter" scenarios by hand. Verify `npm run check` passes.
- [ ] 9.6 Add the Effects menu (Add Effect…, Bypass All Effects as a check item, Manage Plugins…) to `menu.ts`, with enabled states following the active tab. Verify `npm run check` passes, and check by hand that the items are greyed out with no file open.

## 10. Documentation and integration checks

- [ ] 10.1 Update `README.md`: VST3 effects, the plugin folders per OS, the Linux X11/XWayland note for editors, the fact that offline rendering may differ from what is heard for some plugins, and Steinberg's trademark notice. Add the macOS signing note (`disable-library-validation`) to `docs/releasing.md`. Verify the documented folders match `plugins/cache.rs`.
- [ ] 10.2 Run the full `docs/acceptance.md` effects pass on Windows, macOS and Linux. Cover every scenario in the `effect-chain`, `plugin-editors` and `vst3-plugins` specs that the automated tests don't, and record the plugins used. Verify each scenario passes, or file its failure as a follow-up task here.
- [ ] 10.3 Push the branch and verify that CI (check, vitest, clippy, Rust tests including the test-plugin builds, and installers) passes on all three platforms. Then run `openspec validate add-vst3-effect-chain --strict`.

## Workflow follow-up

- Open a pull request from `feat/vst3-plugins` once the tasks are complete.
- Archive the change after review, which syncs the `effect-chain`, `plugin-editors`, `vst3-plugins` and `wav-export` specs.
