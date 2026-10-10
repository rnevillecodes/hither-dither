# Tasks

## 1. Project scaffolding

- [x] 1.1 Scaffold a Tauri v2 app with Svelte 5 + TypeScript + Vite frontend (`src/`) and Rust crate (`src-tauri/`) per design §7 layout; verify `cargo tauri dev` opens an empty window
- [x] 1.2 Add Rust deps (`memmap2`, `ebur128`, `cpal`, `rubato`, `rtrb`, `thiserror`, `tauri-plugin-dialog`) and frontend dev deps (`vitest`); verify `cargo build` and `npm run build` succeed
- [x] 1.3 Configure lint/format/test scripts (`cargo fmt --check`, `cargo clippy -D warnings`, `npm run check`, `npm test`) and a README "Development" section listing prerequisites (Rust, Node, WebKitGTK/WebView2); verify each script runs cleanly on the empty project

## 2. WAV parsing and sample source (Rust)

- [x] 2.1 Add a test helper that writes synthetic WAVs (any format/bit depth/channels, sine/silence/clipped runs, extra `LIST`/`bext` chunks, odd-size chunks, truncated `data`, ADPCM/µ-law headers, non-RIFF bytes); verify helper round-trips a simple 16-bit file read by a test
- [x] 2.2 Implement RIFF chunk walker and `fmt ` parsing (PCM, IEEE float, EXTENSIBLE subformat, 1–32 channels, 1 Hz–768 kHz) with typed errors naming the reason/encoding; verify unit tests for each accepted and rejected case in the wav-import spec
- [x] 2.3 Implement mmap-backed `SampleSource` decoding u8/i16/i24/i32/f32/f64 to `f32` for arbitrary frame ranges, plus raw-domain full-scale checks; verify decoded-value tests for every format and 24-bit precision
- [x] 2.4 Handle truncated `data` chunks (whole frames only + truncation warning flag); verify the truncated-file scenario test

## 3. Peak pyramid and whole-file stats pass (Rust)

- [x] 3.1 Implement per-channel min/max pyramid (256-frame base, 2× levels) built in one streaming pass, with `query(channel, start, end, columns)`; verify tests that a single full-scale sample in silence appears at the correct column at every zoom level, and query results match brute-force min/max on random data
- [x] 3.2 Implement stats accumulator: peak (dBFS + linear), RMS, DC offset %, crest factor, `-inf` for silence; verify the full-scale-sine and silent-channel scenarios (±0.1 tolerances)
- [x] 3.3 Implement clip-run detection (≥3 consecutive raw full-scale samples) recording run positions; verify two-runs and isolated-sample scenarios for 16-bit PCM and float
- [x] 3.4 Integrate `ebur128` integrated loudness for mono/stereo/5.1 (LFE excluded), `n/a` for other layouts or ranges <400 ms; verify -18 dBFS stereo 1 kHz sine → -18.0 LUFS ±0.1 and the 200 ms "n/a" case
- [x] 3.5 Implement range-scoped stats job on a worker thread with cancellation token (newer request cancels older); verify a test that 10 min stereo 48 kHz selection stats complete in <1 s in release mode and that cancellation stops an in-progress job

## 4. Backend commands and loading

- [x] 4.1 Implement `open_file` command: parse, mmap, build pyramid + whole-file stats on a worker thread, stream progress via `tauri::ipc::Channel`, keep previous file on failure; verify a Rust integration test that a failed open leaves prior state intact
- [x] 4.2 Implement `get_metadata`, `get_peaks` (binary `ipc::Response`, raw samples at sample-level zoom), `compute_stats(range?)` and `get_clip_runs` commands; verify command-level tests for response shape and byte length
- [ ] 4.3 Document the command API (inputs, outputs, binary layout) in `docs/ipc.md`; verify each documented command name matches the registered handlers

## 5. Frontend shell and file opening

- [ ] 5.1 Build app layout (toolbar, waveform area, metadata + stats side panel, notice/toast area) with OS light/dark theme; verify `npm run check` passes and the window renders the empty state
- [ ] 5.2 Wire Open menu item + Ctrl/Cmd+O to the dialog plugin (`.wav` filter), handling cancel; verify manually that cancel keeps the current file and a valid file loads
- [ ] 5.3 Implement drag-and-drop loading (first file only + "one file at a time" notice) and error/warning toasts (invalid, unsupported encoding, truncated); verify manually with generated fixtures for each scenario
- [ ] 5.4 Show progress indicator for loads exceeding 500 ms; verify manually with a ~1 GB generated file that progress shows and the window remains movable

## 6. Waveform view

- [ ] 6.1 Implement pure `Viewport` module (frames-per-pixel, start frame, zoom-around-point, clamp, fit) and ruler tick generation (h:m:s down to samples); verify vitest unit tests including zoom-under-pointer invariance
- [ ] 6.2 Implement canvas renderer: per-channel lanes with labels and ±1.0 scale, min/max columns, sample point+line drawing at high zoom, rAF-coalesced redraws with latest-wins peak fetching; verify the single-spike fixture shows a full-scale spike when zoomed to fit
- [ ] 6.3 Implement zoom (Ctrl/Cmd+wheel, +/-, zoom-to-fit) and scroll (wheel, Shift+wheel, scrollbar, overview strip drag); verify manually against each waveform-view zoom/scroll scenario
- [ ] 6.4 Implement time selection (drag, Shift+click extend, Escape/click clear) with highlight and start/end/duration readout; verify vitest tests for selection math and manual check of the 1.0–2.5 s scenario
- [ ] 6.5 Draw clip-run markers from `get_clip_runs`; verify manually with the clipped fixture
- [ ] 6.6 Measure redraw performance while scrolling a 1-hour stereo 48 kHz file on Linux (WebKitGTK) and Windows/macOS if available; verify ≥30 fps via devtools performance trace and record results in `docs/performance.md`

## 7. Metadata and stats panel

- [ ] 7.1 Implement metadata panel (name, size, encoding, bit depth, rate, channels, frames, h:mm:ss.mmm duration); verify vitest formatter tests and the 44.1 kHz/16-bit/10 s scenario manually
- [ ] 7.2 Implement per-channel stats table (peak dBFS/linear, RMS, DC %, crest, LUFS with n/a tooltip, clip count) with "Whole file"/"Selection" scope label, re-requesting on selection change; verify the silent-selection and clear-selection scenarios manually

## 8. Playback (Rust + UI)

- [ ] 8.1 Implement playback engine: default `cpal` device, feeder thread with `rtrb` ring buffer, channel mapping (mono→stereo, down-mix 1/√N with safety clamp), `rubato` resampling, volume gain, atomic frame position with latency compensation; verify Rust unit tests for channel mapping and resampler output length/pitch on a sine
- [ ] 8.2 Implement transport commands (play from position/range, pause, stop-returns-to-start, seek, loop flag, volume) and position streaming via `ipc::Channel` at ~60 Hz; verify a Rust test with a null/mock output that selection playback stops at range end and loop wraps
- [ ] 8.3 Handle no-device case: disable playback UI with message while analysis works; verify by running in an environment without audio (e.g. WSL without PulseAudio)
- [ ] 8.4 Implement UI: Play/Pause (Space), Stop, Home/End, click-to-seek, loop toggle, volume slider, playhead line with interpolated drawing and auto-scroll suppressed after manual scroll; verify manually against every audio-playback scenario, including 96 kHz file on 48 kHz device

## 9. Integration and packaging

- [ ] 9.1 Run an end-to-end manual acceptance pass over every scenario in the four spec files using generated fixtures, recording pass/fail in `docs/acceptance.md`; verify all scenarios pass
- [ ] 9.2 Produce release builds with `cargo tauri build` for the host platform; verify the installer installs and the app opens and plays a WAV file
