# Proposal

## Why

There is no lightweight, cross-platform desktop tool in this project for quickly inspecting WAV files: seeing the waveform, checking format details and levels, and listening to a region. Building one from scratch (hither-dither is currently empty) gives a fast, focused analyser rather than a full DAW or editor.

## What Changes

- New Tauri desktop application (Rust backend, TypeScript frontend) for Windows, macOS and Linux.
- Open WAV files via a file dialog or drag-and-drop; decode 8/16/24/32-bit integer PCM and 32/64-bit IEEE float, mono through multichannel, including `WAVE_FORMAT_EXTENSIBLE` headers.
- Clear, non-crashing error reporting for unsupported or malformed files.
- Interactive waveform view: one lane per channel, time axis, zoom and scroll, click-drag time selection.
- Stats & metadata panel: file/format info (sample rate, bit depth, channels, duration, size) plus per-channel peak, RMS, integrated loudness (LUFS), DC offset and clipping detection, for the whole file or the current selection.
- Audio playback: play/pause/stop, seek by clicking, play the selection, with a playhead synced to the waveform.
- Out of scope for v1: spectrogram and FFT spectrum views, editing/saving audio, RF64/BWF and non-WAV formats, multi-file sessions.

## Capabilities

### New Capabilities
- `wav-import`: opening WAV files (dialog and drag-and-drop), parsing headers, decoding supported sample formats, and reporting errors for unsupported/invalid input.
- `waveform-view`: rendering per-channel waveforms with a time axis, zoom, scroll, and time-range selection.
- `audio-stats`: displaying file metadata and computing level statistics (peak, RMS, LUFS, DC offset, clipping) for the whole file or a selection.
- `audio-playback`: transport controls, seeking, selection playback, and a synchronised playhead.

### Modified Capabilities
<!-- None: no existing specs in this project. -->

## Impact

- New codebase: Tauri v2 project with a Rust crate (`src-tauri/`) and a TypeScript frontend (`src/`), plus build/packaging config.
- New dependencies: Tauri v2 and its dialog plugin; Rust crates for WAV parsing (`hound` or a custom parser), audio output (`cpal`/`rodio`), and loudness (`ebur128`); frontend build tooling (Vite).
- Requires the platform toolchains for Tauri (Rust, Node.js, WebView2 on Windows, WebKitGTK on Linux).
