# Spec Delta

## Purpose

The public GitHub Pages site where people learn what HitherDither is and download it: what it says about the current release and where its download links lead.

## ADDED Requirements

### Requirement: Version shown without the GitHub API
The site's HTML SHALL contain the app version from the commit it was deployed from, in the hero meta line and in the Download section's lead text, so the version is visible with JavaScript disabled or when the GitHub API can't be reached.

#### Scenario: API unreachable
- **WHEN** the site deployed from a commit whose app version is 0.3.0 is opened and the request to the GitHub releases API fails
- **THEN** the hero meta line reads "Version 0.3.0 · free for Windows, macOS and Linux" and the Download section's lead text starts "Version 0.3.0."

#### Scenario: JavaScript disabled
- **WHEN** the same site is opened with JavaScript disabled
- **THEN** both places show version 0.3.0 and every download link goes to the latest release page

### Requirement: Live release replaces the built-in version
When the GitHub releases API returns the latest published release, the site SHALL show that release's version and publish date in place of the built-in version, so the version shown always matches the release the download links point at.

#### Scenario: Bump merged before the release is published
- **WHEN** the site with built-in version 0.3.0 is opened while the latest published release is still v0.2.1
- **THEN** the hero meta line shows "Version 0.2.1 · released <date of v0.2.1> · free for Windows, macOS and Linux" and the download links point at the v0.2.1 installers

#### Scenario: Release published
- **WHEN** v0.3.0 has been published and the site is opened
- **THEN** the hero meta line shows version 0.3.0 with its release date

### Requirement: Site version kept in step with the app
The repository's version check SHALL fail when the version in the site's HTML differs from the app version in the other version files, and SHALL run on every pull request as well as on release tags.

#### Scenario: Site missed in a bump
- **WHEN** a pull request changes the app version to 0.3.0 everywhere except the site's HTML, which still says 0.2.1
- **THEN** the version check in that pull request's CI fails and names the site file with its version

#### Scenario: Consistent bump
- **WHEN** every version file and the site's HTML say 0.3.0
- **THEN** the version check passes, and on a `v0.3.0` tag the release's tag check passes too
