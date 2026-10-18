# Design

## Context

- **Edit model:** `EditList` and `Audio` (`src-tauri/src/edit/`) render the edited audio lazily. Peaks, statistics, playback and the writer all read through `Audio`. Undo history in `state.rs` is a list of edit states, and `LoadedFile` holds the current `Arc<Audio>` with its revision and background analysis.
- **Playback:** a feeder thread (`playback/engine.rs`):
  - reads `Audio` in 1024-frame chunks at the file's sample rate;
  - resamples with `rubato` to the device rate;
  - fills a 0.2 s `rtrb` ring that the device callback drains.

  `Retarget` already lets the feeder switch audio mid-play and flush queued output with a 5 ms declick. The feeder is not the device callback, so blocking briefly there doesn't glitch the audio.
- **Saving** (`state.rs::save_inner`):
  1. finishes the analysis of the edited audio;
  2. writes the file to a temporary path through `write_wav_with_progress(&Audio)` (`render_native`);
  3. renames it over the target;
  4. installs that analysis as the saved file's source analysis.

  The release profile uses `panic = "abort"`.
- **Platforms:**
  - Windows and macOS: Tauri runs the native event loop on the main thread.
  - Linux: Tauri uses GTK, which may run natively on Wayland.
- **Plugin code:** there is none yet. The VST3 SDK is MIT-licensed as of 3.8.
- **Specs:** the stats requirement is in `specs/effect-chain` ("Effects not drawn or analysed"), so effects never touch analysis, peaks or clip markers. The only path that renders effects offline is Save.

## Goals / Non-Goals

**Goals:**
- Host VST3 effects in-process with a strict, documented threading model that well-behaved plugins expect.
- Reuse the feeder and `Retarget` machinery for live effects instead of adding a second playback path.
- Save renders effects, writes the file and analyses the result in a single pass.
- Run automated tests on all three CI platforms against a real VST3 binary built in the repository.

**Non-Goals:**
- Out-of-process plugin hosting (sandboxing). Only scanning runs out of process.
- Sample-accurate parameter automation. Parameter changes are delivered at the start of the next processed block.
- 64-bit float processing. The chain always processes 32-bit float.
- Unloading plugin modules before the app exits.

## Decisions

### VST3 bindings and a hand-written host
Use the `vst3` crate (Rust bindings generated from the MIT-licensed SDK headers, MIT/Apache-2.0) for the COM interfaces and smart pointers. Implement the host side in Rust:
- `IHostApplication`;
- `IComponentHandler` (with `IComponentHandler2` stubs);
- `IPlugFrame`;
- `IRunLoop` on Linux;
- `IParameterChanges` / `IParamValueQueue`;
- an empty `IEventList`;
- a memory `IBStream`;
- `IAttributeList` / `IMessage`, for plugins that connect their component and controller through the host.

Pin the crate version in task 1.1 after checking its licence and that it exposes the Linux run-loop and content-scale interfaces.

*Alternatives:*
- `vst3-sys` is GPLv3 and incompatible with distributing this MIT app.
- Hosting through the C++ SDK via `cxx` brings CMake and a C++ toolchain into every build.
- `nih-plug` only builds plugins; it can't host them.

### Code layout
```text
src-tauri/src/plugins/     discovery and hosting primitives
  module.rs                load a .vst3 bundle per OS (Windows: Contents/x86_64-win/*.vst3 or a single-file DLL, InitDll/ExitDll;
                           macOS: CFBundle, bundleEntry/bundleExit; Linux: Contents/x86_64-linux/*.so, ModuleEntry/ModuleExit)
  host.rs                  host COM objects listed above
  instance.rs              one effect: component + controller, setup, bus negotiation, latency, state
  scan.rs, cache.rs        scanner subprocess protocol, plugins.json cache, plugin-settings.json, crash marker
  editor/{mod,win,mac,x11}.rs   native editor windows
  main_thread.rs           run a closure on the UI thread and wait for its result
src-tauri/src/fx/          chains
  chain.rs                 slots, order, bypass, latency; the snapshot the feeder processes
  live.rs                  realtime processing used by the feeder
  render.rs                offline render used by Save
```
Loaded modules are cached per bundle path and stay loaded until the app exits. Many plugins misbehave when unloaded and reloaded.

### Scanning out of process
- `main.rs` checks for `--vst3-scan <bundle>` before starting Tauri. In that mode it:
  1. loads the bundle;
  2. enumerates factory classes;
  3. for each `Audio Module Class` whose subcategories contain `Fx` and not `Instrument`, instantiates the class and initialises it;
  4. records name, vendor, version, subcategories and whether the plugin creates an editor view;
  5. probes `setBusArrangements` for the layouts HitherDither can open (mono, stereo, and the WAV channel masks for quad, 5.0, 5.1, 7.1, plus "n discrete channels" for each count from 1 to 8);
  6. prints JSON to stdout and exits.
- The app runs up to 4 scans at once with a 20 s timeout. A non-zero exit or a signal means "Crashed while being scanned"; a timeout kills the process.
- **Architecture check:** a bundle with no binary for the running OS and architecture (for example only `x86_64-win` on ARM64, or a Mach-O without the running slice, checked with `lipo`-style header parsing) is marked Incompatible without launching a subprocess.
- **Cache:** `plugins.json` in the app data directory, keyed by bundle path, holding the size and modification time of the binary and the class results.
- **Settings:** `plugin-settings.json` holds the extra folders and the disabled class IDs.
- Results stream to the frontend as a `plugins-changed` event with `{ done, total }`.

*Alternatives:*
- Scanning in-process is faster, but one bad plugin crashes the app on every start.
- Reading `moduleinfo.json` alone skips loading, but it has no bus information, which the browser needs to grey out effects. It can be added later as a fast path.

### Crash marker
Before any in-process step that runs plugin code for the first time in this session, the host writes `plugin-loading.json` (class ID, step) and removes it afterwards. These steps are creating or initialising an instance and opening a view. On startup, a leftover marker:
- adds that class to the disabled list;
- queues a startup notice, fetched by the frontend with `take_startup_notices`.

### Threading model
- **UI thread** (the Tauri main thread):
  - all `IEditController` and `IPlugView` calls;
  - instance creation and `initialize`;
  - `setBusArrangements`, `setupProcessing`, `setActive` and `getState`/`setState`;
  - editor windows.

  Commands that need these calls marshal through `main_thread::run(app, closure)` (`AppHandle::run_on_main_thread` plus a oneshot channel) and wait for the result.
- **Processing thread:** `setProcessing` and `process` run on whichever thread owns the chain's processing lock: the feeder during playback, or the save worker during render. A slot's processor is never driven from two threads at once.
- **Parameter flow (UI to processor):**
  - `IComponentHandler::performEdit` (from a native editor) and `set_fx_param` (generic controls) write `(paramId, value)` into the slot's pending map (latest value wins).
  - The next `process` call on any thread drains the map into `IParameterChanges` at offset 0.
  - While stopped, changes simply wait, because play and render both start by delivering them. So no zero-length `process` calls are needed.
- **Parameter flow (processor to UI):** output parameter changes, such as meters, are queued and applied to the controller on the UI thread on a 30 Hz timer.
- **Restarts:** `restartComponent` flags are handled on the UI thread:
  - latency changed: recompute the chain latency and retarget playback;
  - parameter values or titles changed: emit `fx-params-reset`.

### Chain state, history and dirtiness
- `LoadedFile` gains:
  - `slots: HashMap<SlotId, Arc<Slot>>`, holding every instance referenced by the current state or by any history entry;
  - `chain: Vec<SlotId>`;
  - `bypass_all: bool`.
- History entries become `Snapshot { edits: Arc<EditList>, chain: Vec<SlotId> }`:
  - Add, remove and move push a snapshot with the same `edits`. Undoing a removal brings the same live instance back, with its settings.
  - When history is truncated (a new step after undo) or cleared (save, close), slots no longer referenced are released on the UI thread.
- Bypass (per slot, and `bypass_all`) lives outside history, as the spec requires.
- `dirty = !edits.is_identity() || chain has a slot that is not bypassed and bypass_all is off`.
- Chain steps return the usual `EditOutcome` with `audioChanged: false`. The frontend then skips peak invalidation and `wait_analysis`, and the revision is not bumped, so background analysis is untouched.
- Chain length is capped at 8 slots in `add_effect`.

*Alternative:* versioning parameter state in history (`getState` per step). This was rejected because parameter changes aren't undo steps, and storing state blobs per step costs memory for large plugins such as convolution reverbs.

### Live processing in the feeder
- **Chain snapshot:** `Shared` gains `fx: Mutex<Option<Arc<LiveChain>>>`, the ordered active slots plus the total latency `L`. The feeder checks it before each chunk, like `retarget`. A chain change (add, remove, move, bypass) installs a new snapshot and requests a flush through the existing retarget path, with the same audio and range, so the change is heard at once.
- **Per chunk:** the feeder:
  1. reads up to 1024 frames from `Audio`;
  2. deinterleaves them into the chain's preallocated planar buffers;
  3. calls each slot's `process` in order (realtime mode, 32-bit float, with `ProcessContext` playing, the file's sample rate and the output frame as project time);
  4. interleaves the result and resamples.
- **Bypass:** a slot with a `kIsBypass` parameter is bypassed through that parameter, which keeps its latency and avoids clicks. Otherwise the host skips the slot.
- **Latency:** after a play start or any flush, the feeder discards the first `L` processed frames, so output stays aligned with file frames and position markers are unchanged. At the end of the range it feeds `L` frames of silence to drain the chain. On a loop wrap the input stream simply continues, so the alignment holds.
- **Reset:** `play` resets active slots (`setActive(false/true)` on the UI thread) before the feeder starts, so tails from an earlier play don't leak in. Retargets after edits don't reset, and tails carry over naturally.
- **Responsiveness:** with an active chain, the feeder limits ring fill to 0.15 s, so parameter changes stay within the 0.25 s budget (ring, plus one chunk, plus the device buffer).
- **Overload:** the engine counts callbacks that find the ring empty mid-range. Three or more within 2 s with an active chain sends a single `overload` message on the play command's event channel.

### Save renders through the reused instances
Save already stops playback. With an active chain, `save_inner`:
1. Skips the separate analysis of the unprocessed audio, because it would be thrown away.
2. Takes the processing lock and reconfigures each active slot for offline rendering (`setActive(false)`, `setupProcessing(kOffline, 4096)`, `setActive(true)`). It then delivers pending parameter changes.
3. Streams `fx::render`: read `Audio` in 4096-frame chunks, process, drop the first `L` frames, append `L` frames of silence at the end, and emit exactly `frames` frames.
4. Converts each frame to native units (PCM: scale, round, clamp and count the clamped samples; float: unchanged) for a writer that now takes a `NativeFrames` source instead of `&Audio`.
5. Feeds the same frames, normalised from their quantised values, into a streaming stats accumulator, a `PyramidBuilder` and a clip detector, so the saved file's analysis comes from the same single pass. `stats::analyse` is split into an accumulator with `push(chunk, full_scale)` plus the existing driver.
6. On success: renames, reopens as today with an identity edit list and the analysis from step 5, clears the chain, closes its editors and releases all slots on the UI thread. It then returns `clampedSamples` in the outcome for the notice.
7. On failure or cancel: reconfigures the slots back to realtime and keeps everything.

Progress is the frame count of that single pass.

Reusing the live instances avoids keeping a second copy of every plugin, and avoids copying state between instances. The live instances are discarded after a successful save anyway.

### Native editor windows owned by the host
The host creates its own top-level window per editor rather than a Tauri window, because:
- Tauri windows carry a webview, or need the `unstable` feature for a bare window;
- on Linux, Tauri windows are GTK and may be Wayland, but VST3 needs an X11 window ID.

Per platform:
- **Windows:** a Win32 window owned by the main HWND, so it stays above it and minimises with it. The plugin is attached with `kPlatformTypeHWND`. DPI comes from `GetDpiForWindow` and goes to `IPlugViewContentScaleSupport`; `WM_DPICHANGED` is handled.
- **macOS:** an `NSWindow` added as a child window of the main window, with its content `NSView` attached via `kPlatformTypeNSView`.
- **Linux:** an X11 top-level window created with `x11rb`. Its `WM_TRANSIENT_FOR` is set to the main window when GTK is on X11. The plugin is attached with `kPlatformTypeX11EmbedWindowID`.
  - `IRunLoop` file descriptors and timers are registered as GLib sources on the main context, which Tauri's GTK loop runs.
  - The X connection's file descriptor is also a GLib source, for configure and close events.
  - With no `DISPLAY`, native editors are reported as unavailable.

Window behaviour:
- `IPlugFrame::resizeView` resizes the window and then calls `onSize`. User resizes go through `checkSizeConstraint`.
- Closing the window calls `removed()` on the view, releases it and emits `fx-editor-closed`.
- `set_active_file` hides the editors of inactive tabs and shows the active tab's editors, remembering their positions.

### Generic controls in the webview
- `get_fx_params(fileId, slotId)` returns the automatable, visible, writable parameters except bypass:
  ```text
  { id, title, units, stepCount, defaultValue, value, display }
  ```
  The controller converts each value to its display string.
- `set_fx_param` sets the parameter on the controller and on the processor's pending map, and returns the new display string.
- Changes coming from the native editor are emitted as a `fx-param-changed` Tauri event (at most 30 Hz per slot), so open generic controls follow along.
- Tauri events are used here, rather than per-call channels as elsewhere, because the source is the plugin, not a call.

### IPC surface
| Command | Purpose |
| --- | --- |
| `list_plugins` | Plugins with status, layouts and scan progress |
| `plugin_folders` / `set_plugin_folders` | Standard folders (read-only) and extra folders |
| `rescan_plugins(all)` | Changed plugins only, or all |
| `set_plugin_enabled(classId, enabled)` | Enable or disable a plugin |
| `take_startup_notices` | Crash-disabled plugins |
| `get_chain(fileId)` | The tab's chain |
| `add_effect(fileId, classId)` | Add an effect; returns an `EditOutcome` |
| `remove_effect(fileId, slotId)` | Remove an effect; returns an `EditOutcome` |
| `move_effect(fileId, slotId, index)` | Reorder; returns an `EditOutcome` |
| `set_effect_bypass(fileId, slotId, on)` | Bypass one effect |
| `set_bypass_all(fileId, on)` | Bypass the whole chain |
| `open_editor(fileId, slotId, generic)` | Returns `{ native, reason? }` |
| `close_editor(fileId, slotId)` | Close an effect's editor |
| `set_active_file(fileId \| null)` | Show and hide editor windows by tab |
| `get_fx_params` / `set_fx_param` | Generic controls |

Events: `plugins-changed`, `fx-param-changed`, `fx-params-reset`, `fx-editor-closed`. Changes to existing payloads:
- `EditOutcome` gains `chain`, `audioChanged` and `clampedSamples`;
- the play channel gains `overload`.

All of this is documented in `docs/ipc.md`.

### Frontend
- **Tab state:** `Tab` gains `chain: ChainInfo | null`. `run()` in `editActions.ts` handles `audioChanged: false` outcomes without touching peaks or analysis.
- **New components:**
  - `EffectsPanel.svelte`: at the top of the details aside. Rows support drag reorder and Alt+Up/Down, with bypass, controls and remove buttons, plus Bypass All and the "not drawn or analysed" note.
  - `EffectBrowser.svelte`: search, grouped by vendor, incompatible effects greyed out with the reason, scan progress.
  - `PluginManager.svelte`: plugin list, folders, Rescan All, enable/disable.
  - `GenericControls.svelte`: a floating, non-modal panel.
- **Menu:** a new Effects menu with Add Effect…, Bypass All Effects (a check item) and Manage Plugins…. `ToggleId` is extended.
- **Tab switching** calls `set_active_file`.

### Testing with real plugins
A workspace crate `src-tauri/test-plugins/hd-test-fx`, built with the same `vst3` crate, provides:
- **HD Test Gain:** a gain parameter in dB, a 64-sample delay reported as latency, any layout from 1 to 8 channels, no editor;
- **HD Test Stereo:** stereo only;
- **HD Test Synth:** an instrument class, to test filtering.

Two more bundles test the scanner:
- `hd-test-crash` aborts in `GetPluginFactory`;
- `hd-test-hang` sleeps. The scan timeout can be overridden in tests.

A test helper builds these with Cargo and assembles `.vst3` bundle directories in a temporary folder.

Rust tests cover:
- scanning, the cache and crash isolation;
- processing through `LiveChain` driven by `Engine::render`, without a device;
- latency alignment;
- the save render (+6 dB doubles samples, the length is unchanged, clamped samples are counted);
- history, undo and dirtiness in `tests/state.rs`.

Native editors and the Linux run loop are covered by manual checks in `docs/acceptance.md`, using a few common free plugins on each OS.

## Risks / Trade-offs

- [A plugin crashes in-process during playback or in its editor and takes the app down] → Scans are isolated; the crash marker disables plugins that crash while loading or opening; unsaved edits are lost, as with any crash. Out-of-process hosting is left for a later change.
- [Plugins that violate threading rules or misbehave on `numSamples` edge cases] → Strict UI-thread and processing-thread separation; block sizes are always between 1 and the configured maximum; modules are never unloaded.
- [The offline render differs from what was heard (plugins with higher-quality offline modes, or random modulation)] → This is accepted, because VST3 defines offline mode for exactly this purpose. It is documented in the README.
- [Parameter-change latency of about 0.2 s is noticeable when tweaking] → The ring fill is lowered to 0.15 s with an active chain. A future change could process in the device callback.
- [Chain changes during playback cause a short gap because queued audio is flushed and latency frames are dropped] → The existing 5 ms declick covers it, and the gap is shorter than one chunk plus `L`.
- [Wayland sessions without XWayland can't show native editors] → Generic controls are used, with an explanation (spec).
- [macOS code signing later: hardened runtime library validation would block third-party plugins] → Note in `docs/releasing.md` that signing must add `com.apple.security.cs.disable-library-validation`.
- [The VST name and trademark] → Don't use the VST logo; the README carries Steinberg's trademark notice.
- [`panic = "abort"` means a panic in a host callback aborts] → Host callbacks avoid `unwrap` and return `kResultFalse` on unexpected input.
- [32-bit integer files lose precision when processed in f32] → Any processing changes those samples anyway. Untouched files keep the bit-exact path.

## Migration Plan

None for user data: chains live in memory, and the new settings and cache files are created on first use. Rollback is reverting the change; leftover `plugins.json` and `plugin-settings.json` files are harmless. This change modifies `wav-export` as it stands after `add-live-editing` was archived.

## Open Questions

- Tuning of the 0.15 s ring fill with a chain, the scan concurrency (4) and the overload threshold. None of these change the specs.
- Which free plugins to use for the manual acceptance pass on each OS. To be decided when writing `docs/acceptance.md`.
