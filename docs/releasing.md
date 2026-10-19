# Releasing

Releases are built by GitHub Actions (`.github/workflows/release.yml`). Pushing a version tag builds every platform and creates a **draft** release with the installers attached. Nothing is public until you publish the draft.

## Steps

1. **Bump the version** in all four places to the same value (for example `0.2.0`):
   - `package.json` (`"version"`)
   - `src-tauri/Cargo.toml` (`version` under `[package]`)
   - `src-tauri/tauri.conf.json` (`"version"`)
   - `site/index.html` (both `<span data-version>` elements). The site shows this until the release is published, then switches to the published release's version.

   Then refresh the lock files so they don't drift:

   ```bash
   npm install --package-lock-only
   cargo update --manifest-path src-tauri/Cargo.toml --package hither-dither
   ```

2. **Check the versions agree:**

   ```bash
   node scripts/check-version.mjs v0.2.0
   ```

3. **Commit and merge** the bump to `main` (through a PR, so CI builds it first). CI also runs the version check without a tag, so a file the bump missed fails the PR.

4. **Tag and push** the merged commit:

   ```bash
   git switch main
   git pull
   git tag v0.2.0
   git push origin v0.2.0
   ```

5. **Watch the run** under Actions › Release. It checks that the tag matches the version, builds Windows, macOS and Linux, and then creates the draft.

6. **Review and publish:** open the draft on the Releases page, check that all six installers are attached (`.exe`, `.msi`, `.dmg`, `.AppImage`, `.deb`, `.rpm`), edit the generated notes if needed, and click **Publish release**.

## Pre-releases

A tag with a numeric suffix (for example `v0.2.0-1`) works the same way, but the draft is marked as a pre-release. Set the three files to the full version including the suffix (`0.2.0-1`) before tagging. Only numeric suffixes are allowed: the Windows `.msi` format rejects others such as `-rc.1`, so the version check fails those tags before building.

## If a release run fails

No release is created unless every platform builds, so there's nothing to clean up on GitHub except the tag:

```bash
git push --delete origin v0.2.0
git tag --delete v0.2.0
```

Fix the problem (through a PR to `main`), then tag the fixed commit again. If the version check failed, the tag and the files disagree. Bump the files or move the tag.

To throw away a draft you don't want, delete it on the Releases page and delete its tag as above.

## Code signing and VST3 plugins

The installers aren't signed yet. When macOS signing with the hardened runtime is added, the app must also have the `com.apple.security.cs.disable-library-validation` entitlement. Without it, macOS refuses to load third-party VST3 plugins, which are signed by other developers or not at all. The plugin scanner runs the app's own executable as a subprocess (`--vst3-scan`), so the entitlement covers both. Add it through `bundle.macOS.entitlements` in `src-tauri/tauri.conf.json`.
