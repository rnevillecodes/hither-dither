# Tasks

## 1. Level measurement (Rust)

- [ ] 1.1 Add `src-tauri/src/playback/levels.rs`:
  - `LevelBlock` (`out_end`, per-channel `peak` and `sum_sq`, `frames`);
  - `measure(planar, out_end)` for one 256-frame block;
  - `Meter`, with `push_block`, `drop_after(out_index)` and `advance_to(audible_out) -> Option<Vec<[f32; 2]>>`. It takes the max peak since the last call and the RMS over the most recent 300 ms of heard blocks, and caps the queue at 1 s.

  Unit tests:
  - a 0 dBFS sine reads peak 1.0 and RMS −3.01 dB (±0.05);
  - a single-sample transient between two reports is kept;
  - nothing is reported before any block is heard;
  - `drop_after` removes flushed blocks;
  - the queue cap holds.

  Verify `npm run test:rust` passes.
- [ ] 1.2 Wire it into the engine. The feeder measures the processed (after `Live`, before `ChannelMap`) file-channel audio in 256-frame blocks, tagging each with its `written` output index, into a `Mutex<Meter>` on `Shared`. On a flush it calls `drop_after(flush_to)`. The engine publishes the audible output index alongside `audible`/`stamp_nanos`. Add `Shared::levels()` returning the newly heard levels.

  Engine tests in `engine.rs` and `engine/fx_tests.rs`:
  - a mono 0 dBFS sine reports peak about 1.0 on one channel, though the device has two;
  - lowering `volume` does not change reported levels;
  - through HD Test Gain (+6.02 dB, 64 frames latency) a −12 dBFS sine reports about −6 dBFS;
  - after a retarget mid-playback, no levels from the discarded audio appear.

  Verify `npm run test:rust` and `npm run lint:rust` pass.
- [ ] 1.3 Extend `PlaybackEvent::Position` in `commands.rs` with `levels: Vec<[f32; 2]>` filled from `Shared::levels()` (empty when nothing new). Update the `PlaybackEvent` section of `docs/ipc.md` (shape, linear `[peak, rms]` per file channel, post-effects and pre-volume, aligned with `frame`). Verify `npm run test:rust` passes and the documented shape matches the serialized one (add a serde test asserting the JSON for one event).

## 2. Meter logic (TypeScript)

- [ ] 2.1 Add `src/lib/meter.ts`:
  - `dbfs`, and `levelToFraction` (−60…0 dBFS onto 0…1, clamped);
  - the scale marks;
  - `stepBallistics` (instant rise, 20 dB/s fall, 1.5 s hold then fall, max-peak readout, clip latch at linear ≥ 1.0);
  - `clearReadouts`;
  - `formatReadout` (one decimal, "–∞" for silence).

  Add `src/lib/meter.test.ts` covering:
  - the scale ends and marks;
  - rise and fall timing ("Peak falls back": −1 dBFS to empty in about 3 s);
  - hold for 1.5 s;
  - clip at exactly 1.0 but not at 0.94 (−0.5 dBFS);
  - readouts surviving idle decay;
  - clearing.

  Verify `npm test` passes.
- [ ] 2.2 Add `src/lib/meterFeed.ts` (non-reactive latest levels and sequence number). Update `api.ts` `PlaybackEvent` with `levels: [number, number][]`. In `transport.svelte.ts`, feed position levels into it and reset it (and signal a readout clear) when playback starts. Verify `npm run check` and `npm test` pass.

## 3. Meter UI

- [ ] 3.1 Add `src/components/LevelMeter.svelte`:
  - a DPR-aware canvas with a per-channel column (30px), a scale column and colours from CSS variables;
  - peak bar (accent, warning from −6 dBFS), inner RMS bar, hold tick and clip light;
  - DOM readouts above each column with channel labels;
  - a rAF loop only while playing or decaying;
  - focusable, with `aria-label="Level meter"`; click, Enter or Space clears readouts and clip lights.

  Verify `npm run check` passes.
- [ ] 3.2 Place it in `src/App.svelte`:
  - a `meter` grid area between the waveform and the sidebar or activity bar (`1fr auto 340px 44px` / `1fr auto 44px`), only while files are open;
  - keyed on the active tab id;
  - narrow layout rows `wave / meter / bar / side`, with the meter drawn horizontally.

  Update the "Collapse by clicking the active button" scenario in `openspec/changes/tabbed-details-sidebar/specs/details-sidebar/spec.md` to "up to the meter". Verify `npm run check` passes and, by hand, the "Beside the sidebar", "Sidebar collapsed", "No files open", "Stereo file", "5.1 file" and "Narrow window" scenarios.

## 4. Documentation and integration checks

- [ ] 4.1 Update `README.md` (a sentence on the live meter: after effects, before volume) and `docs/acceptance.md`: add a `level-meter` section with a row for every scenario in `specs/level-meter/spec.md` (naming the automated tests in Evidence where they exist). Update row SB8 for the meter, and list this change in the intro paragraph. Verify every spec scenario has a row.
- [ ] 4.2 Run `npm run tauri dev` and walk the `level-meter` rows by hand, including "In step with the sound" with `beep-every-second-mono-60s.wav`, "Bypass All" with an effect that adds gain, "Volume does not move the meter", pausing, switching tabs and keyboard clearing. Verify each passes, or record its failure as a follow-up task here.
- [ ] 4.3 Run `npm run check`, `npm test`, `npm run test:rust`, `npm run lint:rust` and `openspec validate playback-level-meter --strict`; verify all pass.

## Workflow follow-up

- This change shares `feat/tabbed-sidebar` with `tabbed-details-sidebar`; both go in the same PR.
- Archive after `tabbed-details-sidebar`.
