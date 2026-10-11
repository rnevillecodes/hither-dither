# Spec Delta

## Purpose

Automatically checks every change and produces installable HitherDither builds for Windows, macOS and Linux. Version tags publish those builds as a draft GitHub Release.

## ADDED Requirements

### Requirement: Checks on pull requests and main
For every pull request and every push to `main`, the pipeline SHALL run the frontend type check, the frontend unit tests, the Rust format check, Rust clippy with warnings treated as errors, and the Rust tests. Any failure SHALL mark the run as failed and SHALL be visible on the pull request.

#### Scenario: Failing test blocks a green check
- **WHEN** a pull request introduces a failing frontend or Rust test
- **THEN** the pull request shows a failed check naming the step that failed

#### Scenario: Clean change
- **WHEN** a pull request passes all checks and builds
- **THEN** the pull request shows all checks as passed

### Requirement: Installers for every platform
Each pipeline run SHALL build the app in release mode on Windows, macOS and Linux and produce these installers: a Windows NSIS `.exe` and an `.msi`; one universal macOS `.dmg` that runs natively on Apple Silicon and Intel; and Linux `.AppImage`, `.deb` and `.rpm` packages. If any platform fails to build, the run SHALL fail.

#### Scenario: All platforms build
- **WHEN** a pipeline run completes successfully
- **THEN** all six installer files exist, named with the app name and version

#### Scenario: Universal macOS build
- **WHEN** the macOS `.dmg` from a run is opened on an Apple Silicon Mac and on an Intel Mac
- **THEN** the app starts natively on both

### Requirement: Downloadable build artifacts
For pull requests and pushes to `main`, the installers SHALL be attached to the run as downloadable artifacts, one per platform, and kept for 14 days. These runs SHALL NOT create or change GitHub Releases.

#### Scenario: Testing a pull request build
- **WHEN** a pull request's run has finished
- **THEN** its summary page offers Windows, macOS and Linux artifacts containing that run's installers

### Requirement: Draft release from a version tag
Pushing a tag of the form `vMAJOR.MINOR.PATCH`, optionally with a numeric pre-release suffix such as `-1`, SHALL build all platforms and then create a draft GitHub Release for that tag with all six installers attached. Tags with a pre-release suffix SHALL be marked as pre-releases. The release SHALL be created only after every platform has built; if any build fails, no release SHALL be created.

#### Scenario: Release tag
- **WHEN** the maintainer pushes tag `v0.2.0` and every build succeeds
- **THEN** a draft release named "HitherDither v0.2.0" exists with the six installers attached, and nothing is published until the maintainer publishes it

#### Scenario: One platform fails
- **WHEN** a release tag is pushed and the Linux build fails
- **THEN** the run fails and no release is created for that tag

### Requirement: Tag must match the app version
Before building, a tag run SHALL check that the tag is `vMAJOR.MINOR.PATCH` or `vMAJOR.MINOR.PATCH-N` with a numeric `N` (the Windows MSI format accepts only numeric pre-release identifiers), and that the tag's version equals the version in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`. On a mismatch it SHALL fail without building, with a message naming each file and the version it holds.

#### Scenario: Forgotten version bump
- **WHEN** the maintainer pushes tag `v0.2.0` while the files still say `0.1.0`
- **THEN** the run fails before any build, and the message lists the three files with `0.1.0` and the expected `0.2.0`

#### Scenario: Non-numeric pre-release tag
- **WHEN** the maintainer pushes tag `v0.2.0-rc.1`
- **THEN** the run fails before any build, with a message saying pre-release suffixes must be numeric (for example `v0.2.0-1`)

### Requirement: Unsigned builds are documented
Until code signing is added, the README SHALL say that the installers are unsigned and explain how to open them on Windows (SmartScreen "More info → Run anyway") and macOS (right-click → Open, or System Settings → Privacy & Security → Open Anyway).

#### Scenario: First launch on macOS
- **WHEN** a user downloads the `.dmg` and macOS blocks the app
- **THEN** the README's download section tells them how to open it

### Requirement: Superseded runs are cancelled
A new push to the same pull request or branch SHALL cancel that branch's run that is still in progress. Tag runs SHALL NOT be cancelled this way.

#### Scenario: Quick follow-up push
- **WHEN** the author pushes twice to a pull request within a minute
- **THEN** the first run is cancelled and only the second runs to completion
