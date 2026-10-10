# Design

## Context

The repository is empty apart from OpenSpec scaffolding, so there's no existing code or conventions to follow (see proposal.md - Why). The stack was chosen with the user: Tauri v2 with a Rust backend and a TypeScript web frontend. The specs require files up to 2 GB to load on an 8 GB machine, interactive waveform redraws, selection stats within about 1 s, and playback with a playhead synced to within 50 ms. These targets drive most of the decisions below.

## Goals / Non-Goals

**Goals:**
- Keep all sample data and heavy computation in Rust. The frontend only receives small, render-ready summaries.
- Use bounded memory: never hold a full decoded copy of a large file.
- Keep the UI thread free: loading, analysis and audio output run on background threads and can be cancelled.
- Structure the code so spectrogram/FFT views can be added later as new consumers of the same sample source.

**Non-Goals:**
- Editing, writing or exporting audio.
- Plugin systems, theming beyond light/dark following the OS, or localisation.
- Code signing, notarisation and auto-update. Packaging produces unsigned installers only.

## Decisions

### 1. Custom WAV parser over `hound`
A small purpose-built RIFF parser reads the `fmt ` and `data` chunk locations and skips unknown chunks, including odd-size padding.
- *Why:* The specs need things `hound` does poorly or not at all: tolerating truncated `data` chunks (load the frames that exist, then warn), 64-bit float, and error messages that name the unsupported encoding (format tag → name). The parsing itself is about 200 lines and easy to test.
- *Alternative:* `hound`. Its API is fine, but it rejects truncated files and lacks f64 support. `symphonia` is heavier than we need and abstracts away the raw integer values that clipping detection needs.

### 2. Memory-mapped samples plus a peak pyramid, not a full decode
The file is memory-mapped (`memmap2`). A `SampleSource` decodes frames on demand from the mapped bytes into `f32`. While loading, one streaming pass builds a **min/max peak pyramid** per channel: level 0 holds min/max for each 256-frame block, and each higher level merges pairs of buckets. The same pass computes whole-file stats (Decision 5).
- *Why:* Decoding a 2 GB 16-bit file to `f32` would need about 4 GB. The pyramid costs well under 1% of the file size and answers any "min/max per pixel column" query in O(columns). This meets the zoomed-out peak-visibility requirement and the redraw-rate target. When zoomed in below 256 frames per column, the samples are read directly from the mmap.
- *Alternative:* Full decode into RAM. Simpler, but it fails the 2 GB / 8 GB requirement. Streaming the file without mmap would make random access for playback and zoomed views more complex.
- *Risk handled:* If the file changes on disk while mapped, reads become undefined. See Risks.

### 3. IPC: binary waveform tiles over Tauri commands
The frontend calls `get_peaks(channel, start_frame, end_frame, columns)`. It returns interleaved `[min, max]` `f32` pairs as a raw binary `tauri::ipc::Response`, not JSON. At sample level it returns raw samples instead. Other commands are `open_file(path)`, `get_metadata`, `compute_stats(range?)`, and the playback commands. Long-running operations report progress and results through `tauri::ipc::Channel`.
- *Why:* Binary responses avoid the cost of JSON-encoding thousands of floats on every scroll frame. A query sized to the viewport means the transfer stays proportional to screen width, not file length.
- *Alternative:* Send the whole pyramid to the frontend once. This simplifies scrolling, but for large files it's many MB of data, and the frontend would still have to fetch from the backend at sample-level zoom.

### 4. Frontend: Svelte 5 + TypeScript + Vite, waveform drawn on Canvas 2D
Svelte components handle the toolbar, stats panel, notices and layout. A single `<canvas>` per lane stack draws the waveform, ruler, selection, clip markers and playhead. All pixel↔frame mapping lives in a pure `Viewport` module (start frame, frames per pixel) that has unit tests.
- *Why:* Canvas 2D handles a few thousand vertical lines per frame with no trouble, and avoids WebGL complexity. Svelte keeps the panel/UI code small without the overhead of a heavier framework.
- *Alternatives:* Vanilla TS, which means more hand-written DOM plumbing for the panels. React works too but adds more runtime than this app needs. WebGL can be revisited if spectrograms need it.
- Redraws are coalesced with `requestAnimationFrame`. Peak requests already in flight are superseded by newer ones (latest-wins) so fast scrolling doesn't queue up stale work.

### 5. Statistics engine in Rust, cancellable, raw-domain clipping
One streaming pass per range accumulates per-channel peak, sum of squares, sum (for DC), and clip runs, and feeds an `ebur128` meter (mode `I`) for integrated loudness. Clipping is detected on the **raw** sample values (integer min/max for PCM, |x| ≥ 1.0 for float) before conversion to `f32`, because a positive full-scale 16-bit sample is 32767/32768 < 1.0 after normalisation. Whole-file stats are computed during load. Selection stats run on a worker thread, and each new selection cancels the previous job through an `AtomicBool` token. Clip run positions are stored so the waveform view can mark them.
- *Why:* A single pass over 10 min of 48 kHz stereo (about 57 M samples) takes well under 1 s in Rust. `ebur128` is a maintained, BS.1770-4-compliant port of libebur128, which saves us implementing gating by hand.
- LUFS layout support: mono, stereo and 5.1 (L, R, C, LFE, Ls, Rs; LFE is excluded per BS.1770). Other layouts return `n/a`, as the spec requires.

### 6. Playback: `cpal` output stream with `rubato` resampling
A dedicated audio thread owns a `cpal` output stream on the default device. A feeder thread reads frames from the `SampleSource` into a lock-free ring buffer (`rtrb`). It applies channel mapping (mono → both, extra channels down-mixed to stereo, or mapped 1:1 when the device has enough channels), resamples with `rubato` when the file and device rates differ, and applies volume gain. The current frame position is published through an `AtomicU64`. A Tauri `Channel` streams the position to the frontend at about 60 Hz, and the frontend interpolates between updates for smooth drawing. The reported position is compensated by the stream's output latency (from `cpal` timestamps) to stay within the 50 ms sync target.
- *Why:* `cpal` gives direct control over device format and latency. `rodio` hides the latency information and makes position accounting less precise.
- *Alternative:* Play audio in the webview with the Web Audio API. That would mean shipping the whole decoded buffer to the frontend, which conflicts with Decision 2.

### 7. Project layout
```
src-tauri/            Rust crate (Tauri app)
  src/wav/            parser, format detection, SampleSource
  src/peaks.rs        peak pyramid build + query
  src/stats/          levels, clipping, loudness
  src/playback/       device, feeder, resampling, transport state
  src/commands.rs     Tauri command handlers
  tests/fixtures/     generated WAV test files (via build-time test helper)
src/                  Svelte + TS frontend
  lib/viewport.ts     pure pixel<->frame mapping
  lib/waveform/       canvas renderer
  components/         Toolbar, StatsPanel, MetadataPanel, Notices
```
Test fixtures are generated in the Rust tests (writing synthetic WAVs: sine, silence, clipped runs, truncated, ADPCM header) instead of being checked in as binaries.

## Risks / Trade-offs

- [File modified or deleted while mmapped → undefined reads or SIGBUS] → Open the file read-only and document that external edits while it's open aren't supported. On Windows the mapping prevents deletion. On Unix, check the file length and modification time before each playback start and show "file changed on disk, reload?" if they differ.
- [WebKitGTK on Linux/WSL has slower canvas and IPC than WebView2/WKWebView] → Viewport-sized binary requests and rAF coalescing keep the load small. Check the 30 fps target on Linux as soon as the renderer exists (task 6.6).
- [Audio device enumeration differs across ALSA/PulseAudio/PipeWire and fails in WSL] → Fall back to "no output device" mode as the spec requires. Analysis keeps working, so WSL development is still usable.
- [`ebur128` only accepts certain channel layouts] → Restrict LUFS to mono/stereo/5.1 and show `n/a` for everything else, as specified.
- [Down-mixing multichannel files to stereo can clip] → Scale the down-mix by 1/√N and apply a soft safety clamp. This is a playback-only effect; the stats aren't affected.
- [Pyramid build time for 2 GB files (I/O bound, a few seconds)] → Show progress (spec: shown for loads over 500 ms), build the pyramid on a worker thread, and keep the window interactive.

## Migration Plan

Greenfield project with nothing to migrate. Release is a local build with `cargo tauri build`. Rollback means not shipping the build.

## Open Questions

- Default zoom-wheel sensitivity and the colour palette for lanes, selection and clip markers can be tuned during implementation.
- Whether to show a channel-name mapping (L/R/C/LFE…) taken from the `WAVE_FORMAT_EXTENSIBLE` channel mask, or just "Ch 1…N", for non-stereo files. Either choice satisfies "channel label".
