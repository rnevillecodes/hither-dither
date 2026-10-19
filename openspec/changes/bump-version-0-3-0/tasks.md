# Tasks

## 1. Site carries the version

- [x] 1.1 In `site/index.html`, make `#release-meta` read `Version <span data-version>0.3.0</span> · free for Windows, macOS and Linux` and `#download-lede` read `Version <span data-version>0.3.0</span>. Installers for every release are on the <a …>Releases page</a>.`; verify by opening the page with the API request blocked (or JS disabled) that both show 0.3.0
- [x] 1.2 Confirm `site/main.js` still replaces both elements on a successful fetch (no code change expected); verify by opening the page normally that it shows the latest published release (v0.2.1 until 0.3.0 is published) with its date

## 2. Version check covers the site and runs in CI

- [x] 2.1 Extend `scripts/check-version.mjs` to read every `data-version>…<` value in `site/index.html`, report it as `site/index.html` (or "(no version found)" / a mismatch if the spans disagree), and update its header comment; verify `node scripts/check-version.mjs` fails with the site still at 0.2.1 and the other files at 0.3.0, and passes once all agree
- [ ] 2.2 Add a `version-check` job to `.github/workflows/ci.yml` running `node scripts/check-version.mjs` (Node 24, checkout only); verify the job appears and passes on the PR
- [x] 2.3 Update `docs/releasing.md` step 1 to list `site/index.html` (both `data-version` spans) among the places to bump, and note that CI checks they agree; verify the documented steps match the files touched in this change

## 3. Bump to 0.3.0

- [x] 3.1 Set version `0.3.0` in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`
- [x] 3.2 Refresh lock files with `npm install --package-lock-only` and `cargo update --manifest-path src-tauri/Cargo.toml --package hither-dither`; verify `package-lock.json` (both root entries) and `src-tauri/Cargo.lock` (`hither-dither` package) say 0.3.0 and no other dependency versions changed in the diff
- [x] 3.3 Run `node scripts/check-version.mjs v0.3.0`; verify it passes listing all four sources

## Workflow follow-up

- Open the PR from a `chore/bump-version-0.3.0` branch (main is PR-protected); merge once CI passes.
- Tag the merged commit `v0.3.0`, push the tag, then review and publish the draft release per `docs/releasing.md`.
- Archive this change after the release is published.
