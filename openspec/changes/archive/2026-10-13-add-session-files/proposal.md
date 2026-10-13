# Proposal

## Why

Effect chains and unsaved edits live only in memory. Closing a tab, quitting the app, or switching to other work means losing every effect, its order and bypass, and every plugin setting, unless the user bakes them into the WAV with Save. There is no way to stop and come back later to the same effects and edits without changing the audio file. The VST3 change listed "remembering chains across restarts" as out of scope; this change adds it as an explicit, user-saved session file.

## What Changes

- **Session files:** a new file type, plain JSON named with the `.hdsession.json` extension so editors highlight it while the app can still tell it apart from other JSON files, that describes one open tab: which WAV it refers to, the unsaved audio edits (segments, silence, fades, crossfades, trim and clip fades), and the effect chain. For each effect it stores the plugin's identity (class ID, name, vendor, version), its bypass switch, and the plugin's full saved state, so the plugin comes back exactly as it was. The chain's Bypass All is stored too. The WAV itself is never written by saving a session.
- **Save Session:** File › Save Session (Ctrl/Cmd+Alt+S) and Save Session As… save the active tab's session. The first Save Session in a tab asks where to save it, defaulting to the WAV's folder and name. After that, Save Session overwrites the same file without asking.
- **Open Session:** File › Open… and drag and drop accept `.hdsession.json` files as well as WAVs. Opening a session opens its WAV in a new tab with the usual loading progress, then restores the edits, the effects and their settings.
- **Finding the WAV:** a session records the WAV's absolute path and its path relative to the session file, so a folder holding both can be moved or copied. It also records the WAV's format and length, so a session is not applied to a different or changed recording.
- **Partial restores:** an effect that is missing, disabled, incompatible or fails to load is left out, with a notice naming it, and the rest of the chain is restored in order. A plugin that rejects its saved state keeps its default settings, with a notice.
- **History and unsaved marker:** a restored session starts a fresh undo history at the restored state, like opening a file. The tab is marked unsaved when the restored edits or effects change the audio, as today.
- **Out of scope:**
  - sessions holding more than one tab;
  - reopening effect editor windows (they start closed);
  - undo history, selection, zoom, scroll and playhead;
  - automatic sidecar sessions, recent sessions, and restoring sessions at launch (the last-session restore keeps reopening WAVs only);
  - an "unsaved session" marker or prompt separate from the existing unsaved-changes prompt;
  - locating a missing WAV through a dialog;
  - embedding the audio in the session file.

Assumptions recorded from the request and the user's answers:
- "The currently loaded / open file" means one session per tab, referring to that tab's WAV.
- "Exact same state" covers the edits, the chain, bypass states and plugin settings, but not the editor windows.
- Sessions are saved and opened explicitly from the File menu.

## Capabilities

### New Capabilities
- `session-files`: saving a tab's edits and effect chain to a session file, the file's contents and how it refers to its WAV, opening a session to restore that tab, and what happens when the WAV or a plugin can't be restored as saved.

### Modified Capabilities
<!-- None: the related capabilities (file-tabs, wav-import, effect-chain, plugin-editors) are still in unarchived changes and have no published specs under openspec/specs/. Opening sessions from File › Open… and drag and drop is specified in session-files. -->

## Impact

- **Rust (`src-tauri`):**
  - New `session.rs`: the versioned session format (serde), conversion between `EditList` and its stored form with validation, base64 plugin state, and WAV path resolution.
  - `edit/mod.rs`: a checked constructor that rebuilds an `EditList` from stored parts.
  - `state.rs`: capture a file's current edits and chain (`getState` per slot on the UI thread, plus parameter changes not yet delivered), and restore a session as a new history base. Plugin loading in `add_effect` is factored out so restoring reuses it, including the crash marker.
  - `plugins/host.rs`: read the pending parameter changes without draining them.
  - `commands.rs`/`lib.rs`: `save_session`, `read_session` and `restore_session` commands.
  - New dependency: `base64` (MIT/Apache-2.0).
- **Frontend (`src`):** `api.ts` gains the session commands; `actions.ts` routes `.hdsession.json` paths from Open… and drag and drop; `editActions.ts` gains Save Session / Save Session As; `menu.ts` gains the two menu items and their enabled states; `Tab` gains `sessionPath`.
- **Docs:** `docs/ipc.md` (new commands), `README.md` (session files), `docs/acceptance.md` (a manual round trip with a real plugin).
- **Tests:** Rust round-trip tests with HD Test Gain and edit lists; frontend vitest tests for path routing.
