# Design

## Context

- **No CI today:** there is no `.github/` directory, and the remote is GitHub (`rnevillecodes/hither-dither`).
- **Build:** `npm run tauri build` runs `npm run build` (Vite → `dist/`), then builds the Rust crate in release mode (LTO, `codegen-units = 1`, stripped) and bundles it. In `tauri.conf.json`, `bundle.targets` is `"all"` and the icons are configured.
- **Version lives in three places:** `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json` (all `0.1.0`).
- **Linux system packages:** listed in the README (WebKitGTK 4.1, GTK 3, libsoup 3, appindicator, librsvg, ALSA headers).
- **Existing check scripts:** `npm run check`, `npm test`, `npm run lint:rust` (fmt + clippy `-D warnings`), `npm run test:rust`. No Rust test needs an audio device; the one ignored test is a release-only timing check.
- **Clippy:** fails on `main` with current stable Rust, with 4 errors in `wav/parser.rs`, `wav/source.rs` and `wav/synth.rs`. CI installs current stable, so it would fail the same way.

See proposal.md for why.

## Goals / Non-Goals

**Goals:**
- One build definition shared by PR, `main` and tag runs, so a green PR build means the release build will work.
- A release is all-or-nothing: no half-populated releases.
- Fast enough feedback: caching, and cancelling superseded runs.

**Non-Goals:**
- Code signing, notarization, and Tauri's auto-updater (they need certificates and a signing key; deferred).
- Publishing to app stores, Homebrew, winget or Flathub.
- ARM Linux or ARM Windows builds.
- Automatic version bumping or changelog generation.

## Decisions

### 1. Three workflow files around one reusable build
- `.github/workflows/build.yml` is a reusable workflow (`on: workflow_call`). It holds the platform matrix and does checkout → toolchains → caches → system packages → checks → `tauri build` → upload artifacts. It takes a `retention-days` input.
- `ci.yml` runs on `pull_request` and on `push` to `main`. It calls `build.yml` with 14-day retention. Permissions are `contents: read`.
- `release.yml` runs on `push` of tags `v*.*.*`. Its `version-check` job runs `scripts/check-version.mjs`. Then `build` (calls `build.yml`, `needs: version-check`, 1-day retention, since only the release job consumes the artifacts). Then a `release` job (`needs: build`, `permissions: contents: write`) downloads every artifact and creates the draft release.
- *Why reusable rather than one workflow with `if:` conditions:* each trigger's permissions stay minimal (only the release job can write), and the PR build and the release build can't drift apart.
- *Alternative:* `tauri-apps/tauri-action`. It can upload straight to a release per platform, but each matrix job then creates or updates the release itself. That breaks all-or-nothing, since a failed Linux build would leave a release holding only Windows and macOS files. It also doesn't upload workflow artifacts for PR runs. Plain `npm run tauri build` plus `upload-artifact` keeps both flows identical.

### 2. Platform matrix

| Runner | Build command | Bundles |
|---|---|---|
| `windows-latest` | `npm run tauri build -- --bundles nsis,msi` | `.exe`, `.msi` |
| `macos-latest` (Apple Silicon) | `npm run tauri build -- --target universal-apple-darwin --bundles dmg` | universal `.dmg` |
| `ubuntu-22.04` | `npm run tauri build -- --bundles appimage,deb,rpm` | `.AppImage`, `.deb`, `.rpm` |

- macOS installs both Rust targets (`aarch64-apple-darwin`, `x86_64-apple-darwin`), and Tauri combines them with `lipo`.
- `ubuntu-22.04`, not `ubuntu-latest`: binaries built against the older glibc 2.35 run on more distributions, which matters most for AppImage. Packages come from the README's `apt` line.
- `fail-fast: false`, so one platform's failure still shows whether the others built. The overall run still fails.

### 3. Checks inside every matrix job
Each matrix job runs `npm ci`, `npm run check`, `npm test` and `npm run test:rust` on its own OS before bundling, because file mapping, path handling and audio-device fallback differ per OS. `npm run lint:rust` runs on the Linux job only, since formatting and clippy results don't depend on the OS and that avoids paying for them three times. On private repos, macOS minutes cost 10× Linux minutes. If that becomes a problem, running only the checks on macOS for PRs and the full bundle only on `main` and tags is a contained follow-up.

### 4. Toolchains and caching
- Node 24 (LTS) via `actions/setup-node@v4` with `cache: npm`, then `npm ci`. (Not 20: vitest 5 needs Node ≥ 22.12.)
- Rust stable via `dtolnay/rust-toolchain@stable`, with `components: rustfmt, clippy` and `targets` on macOS.
- `Swatinem/rust-cache@v2` with `workspaces: src-tauri`. Release builds with LTO are the slow part, and the cache keeps dependency builds warm.
- Actions are pinned to major versions. Pinning to commit SHAs can follow if supply-chain policy needs it.

### 5. Artifact naming and upload
After bundling, each job uploads `src-tauri/target/**/release/bundle/<format>/*.<ext>` with `actions/upload-artifact@v4`, named `HitherDither-<version>-<os>`. The version comes from `tauri.conf.json`, read with a one-line `node -p` step. Tauri already names files with the product name and version, for example `HitherDither_0.1.0_universal.dmg` and `HitherDither_0.1.0_x64-setup.exe`. Retention comes from the `build.yml` input: 14 days for CI, 1 day for release staging.

### 6. Release creation
The `release` job runs on `ubuntu-latest` and uses `actions/download-artifact@v4` with `merge-multiple: true`. It then runs the preinstalled `gh` CLI:
`gh release create "$TAG" --draft --title "HitherDither $TAG" --generate-notes [--prerelease] files…`
`--prerelease` is added when the tag contains `-`. `GH_TOKEN` is `${{ github.token }}`, which works because of the job's `contents: write`. The release is created only when every matrix job has passed, so a failed build means no release.

### 7. Version check script
`scripts/check-version.mjs <tag>` strips the leading `v` and reads the version from `package.json` (JSON), `src-tauri/tauri.conf.json` (JSON) and `src-tauri/Cargo.toml` (the first `version = "…"` under `[package]`, matched with a regex, no TOML dependency). If any version differs, it exits 1 and prints each file with the version it holds. With a tag, it first rejects anything other than `vX.Y.Z` or `vX.Y.Z-N` (numeric `N`), because WiX, which builds the `.msi`, accepts only numeric pre-release identifiers. Rejecting early fails in seconds instead of after a long Windows build. Run without a tag, it only checks that the three files agree, which is useful locally before tagging.

### 8. Clippy fixes first
Apply clippy's suggested fixes before adding CI: collapse the nested `if` in `wav/parser.rs`, and use `as_chunks::<N>()` instead of `chunks_exact(N)` in `wav/source.rs` (2 places) and `wav/synth.rs`. These are behaviour-preserving, and the existing parser and decode tests cover them. CI uses floating stable Rust, so new lints may appear later. Fixing them as they come is preferred over pinning an old toolchain.

### 9. Concurrency
`ci.yml` sets `concurrency: { group: ci-${{ github.ref }}, cancel-in-progress: true }`. `release.yml` uses `group: release-${{ github.ref }}` with `cancel-in-progress: false`, so a tag run is never cancelled.

## Risks / Trade-offs

- [Unsigned builds trigger SmartScreen and Gatekeeper warnings] → Documented in the README per the spec. Signing is a future change: secrets and `tauri.conf.json` signing settings slot into `build.yml` without restructuring.
- [`macos-latest` or `ubuntu-22.04` images change over time, or 22.04 is retired] → Runner labels sit in one matrix in `build.yml`, so updating them is a one-line change.
- [AppImage bundling downloads `linuxdeploy` at build time and depends on network] → Re-running the job usually resolves a transient failure. If it proves flaky, cache the tool.
- [Floating stable Rust introduces new clippy lints that break CI unexpectedly] → Accepted (Decision 8). The failure is visible and small to fix.
- [Rust tests take longer in CI because of the dev profile's `opt-level = 2` for dependencies] → Accepted, and rust-cache softens it.
- [Release minutes: three full release builds per tag] → Tags are rare, so this is acceptable.

## Migration Plan

1. Merge the clippy fixes and workflows through a PR. That PR's own run is the first end-to-end test.
2. Allow Actions to create releases in the repository settings if org or repo policy restricts `GITHUB_TOKEN`.
3. Try a release with a pre-release tag (for example `v0.1.0-1`, after setting the three files to that version on a branch). Check the draft, then delete the draft and the tag.

Rollback means deleting the workflow files. Nothing else depends on them.
