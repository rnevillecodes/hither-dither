# Tasks

## 1. Make the existing checks pass

- [x] 1.1 Apply clippy's suggested fixes in `src-tauri/src/wav/parser.rs` (collapse the nested `if` into the `match`), `wav/source.rs` (2× `chunks_exact(N)` → `as_chunks::<N>()`) and `wav/synth.rs` (1×), without changing behaviour. Verify that `npm run lint:rust` and `npm run test:rust` both pass locally.

## 2. Version check

- [x] 2.1 Add `scripts/check-version.mjs` (design Decision 7): with a tag argument it compares the tag against `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`; without one it checks the three agree. Verify that `node scripts/check-version.mjs v0.1.0` exits 0, `node scripts/check-version.mjs v0.2.0` exits 1 and lists all three files with `0.1.0`, and `node scripts/check-version.mjs` exits 0, and `v0.1.0-rc.1` exits 1 explaining that pre-release suffixes must be numeric.

## 3. Workflows

- [x] 3.1 Add `.github/workflows/build.yml` as a reusable workflow (design Decisions 1–5): matrix of `windows-latest`, `macos-latest` (universal target) and `ubuntu-22.04` with `fail-fast: false`; Node 24 + npm cache, Rust stable + rust-cache, Linux `apt` packages; `npm ci`, `npm run check`, `npm test`, `npm run test:rust` on every OS and `npm run lint:rust` on Linux only; the per-OS `tauri build` command with explicit `--bundles`; upload `HitherDither-<version>-<os>` artifacts with a `retention-days` input. Verify the YAML parses (`npx --yes yaml-lint .github/workflows/*.yml` or equivalent).
- [ ] 3.2 Add `.github/workflows/ci.yml` (pull requests and pushes to `main`, `contents: read`, cancel-in-progress concurrency, calls `build.yml` with 14-day retention). Verify by pushing the branch and opening a PR: all three platform jobs pass, and the run page offers three artifacts containing the `.exe`, `.msi`, universal `.dmg`, `.AppImage`, `.deb` and `.rpm`.
- [ ] 3.3 Add `.github/workflows/release.yml` (design Decisions 1, 6 and 9): on `v*.*.*` tags, `version-check` → `build` (1-day retention) → `release` (`contents: write`, download all artifacts, `gh release create --draft`, `--prerelease` for tags containing `-`). Verify with a pre-release tag per the design's Migration Plan step 3 (ask before pushing any tag): the draft release "HitherDither v0.1.0-1" is marked pre-release and has all six installers. Then push a mismatched tag and confirm the run fails in `version-check` with no release. Delete the test drafts and tags afterwards.
- [ ] 3.4 Check failure handling: on a throwaway branch, make the Linux job fail (for example a failing `cargo` step) and confirm the CI run is marked failed while the other platforms still finish. A second push to the same PR cancels the first run. Revert the throwaway change.

## 4. Documentation

- [ ] 4.1 Add a "Download" section to `README.md`: where installers come from (Releases for tagged versions, PR/`main` run artifacts for test builds), which file to pick per OS, and how to open the unsigned builds on Windows (SmartScreen "More info → Run anyway") and macOS (right-click → Open, or System Settings → Privacy & Security → Open Anyway). Add a CI status badge. Verify the README renders on GitHub with the badge showing the `ci.yml` status.
- [ ] 4.2 Add `docs/releasing.md`: bump the version in the three files, run `node scripts/check-version.mjs`, commit, tag `vX.Y.Z`, push the tag, review and publish the draft release; how pre-release tags behave; and how to retry a failed release (delete the tag, fix, re-tag). Verify by following it for the rc test in 3.3.
- [ ] 4.3 Verify a downloaded build works on each OS: install the CI-built `.exe` or `.msi` on Windows, the universal `.dmg` on an Apple Silicon or Intel Mac (whichever is available), and the `.AppImage` or `.deb` on Linux (or WSL with WSLg). The app opens a WAV file and shows its waveform. Record the results in `docs/acceptance.md` under a new "build-pipeline" section, with a row per spec scenario.
