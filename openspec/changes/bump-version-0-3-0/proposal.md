# Proposal

## Why

Main has gained features since 0.2.1 (effect groups, group presets, no effect-chain limit, session window title — #13), and grouped sessions are a new session file version that 0.2.1 can't open, so the next release should be 0.3.0. The download site only shows a version after it fetches the latest release from the GitHub API; when that call fails (offline, rate limited, blocked), visitors see no version at all.

## What Changes

- Bump the app version from 0.2.1 to 0.3.0 in `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` and `src-tauri/tauri.conf.json`.
- Write the version into the site's HTML (`site/index.html`) so the hero meta line and the Download section show it without JavaScript or the GitHub API.
- Keep the existing live fetch in `site/main.js`: when it succeeds, its published release version and date replace the static text, so the shown version always matches the files the download links point at.
- Extend `scripts/check-version.mjs` to also read the site's version, so a bump that misses the site fails the check.
- Run `scripts/check-version.mjs` (no tag) in CI on pull requests, so a missed file is caught before merge rather than only when the release tag is pushed.

Releasing itself (merging the bump, tagging `v0.3.0`, publishing the draft release) follows the existing PR-then-tag flow and is not part of this change's code.

## Capabilities

### New Capabilities
- `download-site`: the public GitHub Pages site — what it shows about the current release and how its download links resolve.

### Modified Capabilities
<!-- None: the app's own behavior is unchanged. -->

## Impact

- Version files: `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/tauri.conf.json`.
- Site: `site/index.html` (static version text); `site/main.js` unchanged in behavior beyond overriding the new static text.
- Tooling: `scripts/check-version.mjs` (new site source), `.github/workflows/ci.yml` (new version-consistency job).
- Merging to main redeploys the site (`pages.yml` triggers on `site/**`) before the 0.3.0 release is published; see design for how that window behaves.
