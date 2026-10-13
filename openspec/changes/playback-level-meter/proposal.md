# Proposal

## Why

While a file plays there is no way to see how loud it is at that moment. The Levels table gives whole-file or selection figures for the audio before effects, but nothing shows what an EQ, compressor or limiter in the chain is doing as you listen. A live peak and RMS meter, like a DAW's, fills that gap. It sits right beside the waveform, where your eyes already are.

## What Changes

- Add a vertical **level meter** between the waveform and the sidebar (or between the waveform and the activity bar when the sidebar is collapsed), shown while files are open.
- During playback it shows, for each of the file's channels:
  - a **peak** bar in dBFS;
  - an **RMS** bar drawn inside it;
  - a **peak-hold** tick;
  - a **clip light** that latches when a sample reaches full scale.
- It measures the audio **after the effect chain**, before the volume slider and before the audio is mapped to the output device or resampled. What it shows matches what you hear, but doesn't move when the volume slider does.
- A numeric readout above each channel shows the highest peak since playback started. Clicking the meter clears the readouts and clip lights.
- When playback pauses or stops, the bars fall back to silence. The held readouts stay until the next play or a click.
- The `position` playback event gains per-channel peak and RMS levels, aligned to the audio being heard.
- In narrow windows (the sidebar below the waveform), the meter runs horizontally between the waveform and the activity bar.

## Capabilities

### New Capabilities
- `level-meter`: the live playback meter — what it measures, how it is drawn and scaled, its ballistics, peak hold and clip indication, and where it sits.

### Modified Capabilities
<!-- None in openspec/specs/. The `details-sidebar` capability (unarchived change
     `tabbed-details-sidebar`, same branch) says the collapsed waveform fills the space up to the
     activity bar; with the meter in between, that scenario wording is adjusted in that change
     (see tasks.md). -->

## Impact

- **Rust** (`src-tauri/src/playback/`): the feeder measures peak and sum-of-squares per block on the processed (after effects) file-channel audio and queues them, tagged with their output position. The position reporter in `commands.rs` turns those blocks into the levels being heard and adds them to `PlaybackEvent::Position`. There are new unit and engine tests, including one through HD Test Gain.
- **Frontend**: a new `LevelMeter.svelte` (canvas) and pure `src/lib/meter.ts` (dB scale, ballistics, peak hold) with Vitest tests. `api.ts` and `transport.svelte.ts` carry the levels. `App.svelte` gets a new grid column.
- **Docs**: `docs/ipc.md` (the `position` event), `README.md`, `docs/acceptance.md`.
- No new dependencies. The IPC payload grows by two numbers per channel about every 16 ms.
