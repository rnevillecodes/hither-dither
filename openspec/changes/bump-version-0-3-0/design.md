# Design

## Context

- The app version lives in five files (`package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/tauri.conf.json`); `scripts/check-version.mjs` compares the three hand-edited ones and, given a tag, the tag. It currently runs only in `release.yml`.
- `site/index.html` is static and copied as-is by `pages.yml`, which deploys on any push to main touching `site/**`. `site/main.js` fetches `releases/latest` and rewrites `#release-meta` and `#download-lede` with the release's version and date.
- `release.yml` creates a **draft** release, so `releases/latest` keeps returning v0.2.1 until the draft is published by hand. Merging this bump therefore deploys a site saying 0.3.0 while 0.2.1 is still the latest download.

## Goals / Non-Goals

**Goals:**
- One edit per file for a bump, with a check that catches a missed site.
- The site never shows a version that disagrees with its download links when the API is reachable.

**Non-Goals:**
- Generating the site version at deploy time (a build step in `pages.yml`).
- Changing the tag, draft-release or publish steps.
- Any app code or behavior change.

## Decisions

**Static version text in `site/index.html`, marked with `data-version`.** The two version spots become e.g. `Version <span data-version>0.3.0</span> · free for Windows, macOS and Linux` and `Version <span data-version>0.3.0</span>. Installers for every release are on the <a …>Releases page</a>.` `check-version.mjs` reads the site version with a regex on `data-version>([^<]+)<` and requires every match to agree.
- Alternative: inject the version in `pages.yml` from `package.json`. Rejected — adds a build step to a copy-only deploy and the site file no longer shows what's served; the bump flow is already "edit N files, run the check".
- Alternative: `<meta name="version">`. Rejected — not visible, so it'd still need text in the page; the span keeps one source per spot.

**Live data wins, unchanged.** `main.js` keeps replacing both elements' content on a successful fetch. That resolves the merge-before-publish window in favour of the release the links actually point at (spec: "Live release replaces the built-in version"). No change to `main.js` is needed beyond confirming it overwrites the new markup.

**Run the version check in CI.** Add a small `version-check` job to `ci.yml` running `node scripts/check-version.mjs` (no tag), parallel to the build. Without it, a missed site version only surfaces when the tag is pushed — after the wrong version is already deployed to Pages.

**0.3.0, not 0.2.2.** Chosen by the user: #13 adds features and a session file version 0.2.1 can't open.

## Risks / Trade-offs

- [Site says 0.3.0 between merge and publishing the draft] → only when the API fails; otherwise live data shows 0.2.1. Publish the draft promptly after the release build.
- [Regex parse of HTML is brittle] → the check fails loudly with "(no version found)" if the markup changes, rather than passing silently.
- [`package-lock.json`/`Cargo.lock` aren't covered by the check] → unchanged from today; refresh them with the commands in `docs/releasing.md` (`npm install --package-lock-only`, `cargo update --manifest-path src-tauri/Cargo.toml --package hither-dither`) rather than by hand.

## Migration Plan

1. PR on `chore/bump-version-0.3.0` with the bump, site text, check and CI job; CI must pass.
2. Merge; Pages redeploys.
3. Tag the merged commit `v0.3.0` and push the tag; `release.yml` builds and drafts the release.
4. Publish the draft; the site's live data then shows 0.3.0.

Rollback: revert the PR before tagging; after tagging, delete the draft release and the tag.
