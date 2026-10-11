<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" alt="HitherDither icon: waveform bars dissolving into dither dots" width="128" height="128">
</p>

<h1 align="center">HitherDither</h1>

<p align="center">
  <a href="https://github.com/rnevillecodes/hither-dither/actions/workflows/ci.yml"><img src="https://github.com/rnevillecodes/hither-dither/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI status"></a>
</p>

A lightweight desktop app for inspecting WAV files. It shows the waveform, format metadata and level statistics, and plays back the file or a selection of it. Several files can be open at once, each in its own tab. Built with Tauri v2 (Rust backend) and Svelte 5 + TypeScript (frontend).

## Download

Installers for released versions are on the [Releases page](https://github.com/rnevillecodes/hither-dither/releases). For a test build of a pull request or of `main`, open its run under [Actions › CI](https://github.com/rnevillecodes/hither-dither/actions/workflows/ci.yml) and download the artifact for your OS (kept for 14 days).

| OS | File | Notes |
| --- | --- | --- |
| Windows 10/11 (x64) | `HitherDither_<version>_x64-setup.exe` or `HitherDither_<version>_x64_en-US.msi` | The `.exe` is the usual installer; the `.msi` suits managed installs |
| macOS 11+ (Apple Silicon and Intel) | `HitherDither_<version>_universal.dmg` | One build for both chip types |
| Linux (x64) | `HitherDither_<version>_amd64.AppImage`, `.deb` or `.rpm` | AppImage runs without installing (`chmod +x` it first); `.deb` for Debian/Ubuntu, `.rpm` for Fedora/openSUSE |

### Opening unsigned builds

The installers aren't code-signed yet, so the OS warns the first time you open them:

- **Windows:** if SmartScreen says "Windows protected your PC", click **More info**, then **Run anyway**.
- **macOS:** if macOS says the app can't be opened, right-click (or Control-click) **HitherDither** in Applications and choose **Open**, then **Open** again. On newer macOS versions, open **System Settings › Privacy & Security** and click **Open Anyway** next to the HitherDither message.

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
- `scripts/gen-icons.mjs`: generates the icon source (`src-tauri/icons/app-icon.svg`) and the empty-screen imprint (`src/assets/logo-shadow.svg`). After changing it, run `node scripts/gen-icons.mjs` and then `npx tauri icon src-tauri/icons/app-icon.svg`, and delete the generated `android/`, `ios/` and `64x64.png`, which the app doesn't use.
