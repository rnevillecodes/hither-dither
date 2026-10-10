# hither-dither

A lightweight desktop app for inspecting WAV files. It shows the waveform, format metadata and level statistics, and plays back the file or a selection of it. Built with Tauri v2 (Rust backend) and Svelte 5 + TypeScript (frontend).

## Development

### Prerequisites

- **Rust** (stable, 1.85+ for edition 2024) via [rustup](https://rustup.rs)
- **Node.js** 20+ and npm
- Platform webview and audio libraries:
  - **Linux:** WebKitGTK 4.1 and ALSA headers, e.g. on Debian/Ubuntu:
    `sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev libayatana-appindicator3-dev librsvg2-dev libasound2-dev build-essential pkg-config`
  - **Windows:** WebView2 runtime (preinstalled on Windows 10/11) and the MSVC build tools
  - **macOS:** Xcode Command Line Tools

See the [Tauri prerequisites guide](https://tauri.app/start/prerequisites/) for details.

### Commands

| Command | Purpose |
| --- | --- |
| `npm install` | Install frontend dependencies |
| `npm run tauri dev` | Run the app with hot reload |
| `npm run tauri build` | Build a release installer for the host platform |
| `npm run check` | Type-check the Svelte/TypeScript frontend |
| `npm test` | Run frontend unit tests (vitest) |
| `npm run lint:rust` | `cargo fmt --check` and `cargo clippy -D warnings` |
| `npm run test:rust` | Run Rust unit and integration tests |

### Layout

- `src-tauri/`: Rust crate (WAV parsing, peak pyramid, statistics, playback, Tauri commands)
- `src/`: Svelte frontend (`lib/` holds pure logic and the canvas renderer, `components/` holds UI panels)
- `docs/`: IPC reference, performance and acceptance notes
