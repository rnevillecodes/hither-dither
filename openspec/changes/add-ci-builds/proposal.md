# Proposal

## Why

HitherDither builds only on a developer's own machine, so there are no installers for macOS or Linux and nothing checks pull requests automatically. A breaking change can merge unnoticed (the clippy errors on `main` are an example), and making a release means manual builds on three operating systems.

## What Changes

- New GitHub Actions workflows (the repo has no CI today).
- **Checks on every pull request and push to `main`:** frontend type check and unit tests, Rust format check, clippy and tests.
- **Builds on every pull request and push to `main`:** unsigned installers for Windows, macOS and Linux, uploaded as downloadable workflow artifacts that expire after 14 days.
- **Releases from version tags:** pushing a tag like `v0.2.0` builds all platforms and creates a **draft** GitHub Release with every installer attached. The draft is published by hand. The tag must match the app version, otherwise the run fails before building.
- Installer formats:
  - Windows: NSIS `.exe` and `.msi`
  - macOS: one universal `.dmg` (Apple Silicon and Intel)
  - Linux: `.AppImage`, `.deb` and `.rpm`
- Builds are unsigned for now. The workflow is laid out so signing can be added later. The README explains how to open unsigned builds on Windows and macOS.
- Fix the 4 clippy errors already on `main`, which a newer Rust toolchain introduced, so the new lint check starts green.
- A version-check script and release instructions (`docs/releasing.md`).

## Capabilities

### New Capabilities
- `build-pipeline`: automated checks, per-platform installer builds, build artifacts, and tag-driven draft releases, including the version-match rule and the formats produced for each OS.

### Modified Capabilities
<!-- None: no existing spec covers building or releasing. -->

## Impact

- New: `.github/workflows/build.yml` (reusable build matrix), `ci.yml` (PRs and `main`), `release.yml` (tags).
- New: `scripts/check-version.mjs`.
- New: `docs/releasing.md`.
- Changed: `src-tauri/src/wav/parser.rs`, `wav/source.rs` and `wav/synth.rs`, with behaviour-preserving clippy fixes.
- Changed: `README.md` gets a "Download" section and notes on unsigned builds.
- Repository settings: Actions must be allowed to create releases (`contents: write` for the release job only).
- GitHub Actions minutes: macOS runners bill at a higher rate on private repos, which the design addresses.
- No app behaviour changes.
