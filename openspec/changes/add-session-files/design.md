# Design

## Context

- **Edits:** `EditList` (`src-tauri/src/edit/mod.rs`) is the whole non-destructive edit state: segments of the source (`src_start`, `len`, `silent`, `ramps`, `xfade`), the trim window and the two clip fades. Its fields are private, and `Segment`/`Ramp` don't derive serde. `Curve`, `Dir`, `Crossfade` and `ClipFade` already do.
- **History:** `History` in `state.rs` is a list of `Snapshot { edits, chain: Vec<SlotId> }` with `slots: HashMap<SlotId, Arc<Slot>>` and `bypass_all`. `AppState::install` serves the current snapshot, bumping the revision and restarting analysis when the edits changed. Dirtiness is `!edits.is_identity() || chain.is_active()`.
- **Plugins:** `Instance::get_state` / `set_state` (UI thread) already round-trip the component and controller state as two byte vectors (`plugins::instance::State`). Parameter changes made while stopped sit in the slot's `PendingParams` until the next `process` call, so they aren't in the component state yet. `add_effect` loads a plugin inline (module, `Instance::create`, `configure`, crash marker) on the UI thread via `main_thread::run`.
- **Disk stamp:** `LoadedFile` keeps the WAV's size and modification time from when it was loaded or last saved (`disk_stamp`). `Metadata.frames` follows the edits, so the source's frame count comes from `EditList::source_frames()`.
- **Frontend open flow:** `openPaths` queues tabs and `loadIntoTab` calls `open_file`, then fetches stats, clip runs, clip and chain before marking the tab ready. Save uses a temporary file and `rename_with_retry`. The webview key handler treats any Ctrl/Cmd+S as Save.
- **No fs plugin:** the webview can't read or write files itself; file I/O goes through Rust commands.

## Goals / Non-Goals

**Goals:**
- A restored session gives bit-identical playback and Save output to the state that was saved, for plugins whose output depends only on their saved state.
- One code path for loading a plugin, used by Add Effect and by restoring, so the crash marker, compatibility check and disabled list apply to both.
- A format that is easy to inspect and can grow with optional fields without a version bump.

**Non-Goals:**
- Matching a missing plugin by name or vendor instead of class ID.
- Migrating plugin state between plugin versions (that is the plugin's job under VST3).
- Protecting against hostile session files beyond strict parsing: the plugin state blob is handed to an installed, enabled plugin, as any DAW project does.

## Decisions

### File format: versioned JSON
```jsonc
{
  "format": "hitherdither-session",
  "version": 1,
  "audio": {
    "path": "C:\\Takes\\take.wav",      // absolute, as opened
    "relativePath": "take.wav",          // from the session's folder, "/"-separated; null across drives
    "sampleRate": 48000, "channels": 2, "channelMask": 3,
    "encoding": "PCM", "bitDepth": 24, "validBits": 24,
    "frames": 1234567,                   // source frames as loaded
    "sizeBytes": 7407446, "modified": 1760342400000   // ms since epoch, or null
  },
  "edits": {
    "sourceFrames": 1234567,
    "segments": [{ "srcStart": 0, "len": 48000, "silent": false,
                   "ramps": [{ "t0": 0.0, "t1": 1.0, "dir": "out", "curve": { … } }],
                   "xfade": null }],
    "trim": [0, 48000],
    "fadeIn": { "len": 0, "curve": { … } }, "fadeOut": { … }
  },
  "chain": {
    "bypassAll": false,
    "effects": [{
      "classId": "0123…CDEF", "name": "HD Test Gain", "vendor": "HitherDither", "version": "1.0.0",
      "bypassed": false,
      "state": { "component": "<base64>", "controller": "<base64>" },
      "pendingParams": [[0, 0.75]]
    }]
  }
}
```
- Readers ignore unknown fields, so later minor additions don't need a new version. A `version` above the supported one is rejected with the "newer version" message; a wrong `format` or any parse failure is "not a valid session file".
- Plugin state is base64 (`base64` crate, standard alphabet). It adds a third to the size, which is fine next to the clarity of a text format.
- `f64` ramp positions round-trip exactly through `serde_json`'s shortest representation, which the bit-identical goal relies on. A test asserts `EditList` equality after a round trip.

*Alternatives:* a binary format (bincode/CBOR) is smaller but opaque and harder to version by hand; embedding state as a separate sidecar folder complicates moving and copying.

### Stored edits are validated, not trusted
`session.rs` holds plain serde structs mirroring `EditList`. `EditList::from_parts(source_frames, segments, trim, fade_in, fade_out) -> Result<EditList, EditError>` in `edit/mod.rs` checks the invariants the edit operations maintain: every segment non-empty and inside the source, silent segments without ramps, ramp positions within 0..=1, crossfades even and no longer than either neighbour, `trim` within the untrimmed length, clip fades fitting inside the trimmed audio. It then returns the list through `normalized()`. Any failure is "not a valid session file". The session's `edits.sourceFrames` must also equal `audio.frames`.

*Alternative:* deriving serde on `EditList` directly would expose its internals as the file format and accept inconsistent lists that panic later in rendering.

### Capturing a session (`save_session`)
`AppState::capture_session(id) -> SessionData`:
1. Under the history lock, clone the current `edits`, the current chain's `Arc<Slot>`s and `bypass_all`; release the lock.
2. On the UI thread (`main_thread::run`), call `get_state()` on each slot and read its pending parameter changes with a new non-draining `PendingParams::values()`. A `None` state fails the save, naming the effect.
3. Fill the `audio` block from `Metadata`, `source_frames()` and `disk_stamp`.

The command serialises, writes `<target>.tmp` in the target's folder, and renames it over the target with the existing `rename_with_retry`. The WAV path and relative path are computed against the target. The relative path is a small lexical helper (common-prefix components, `..` for the rest, `None` across Windows drive prefixes), so there is no new dependency for it.

Capturing doesn't stop playback: `getState` is a UI-thread call that plugins support while processing.

### Opening a session: read, load the WAV, restore
The frontend drives three steps so the WAV load reuses the normal tab, queue and progress:
1. `read_session(path)` parses and validates the file and resolves the WAV (absolute path if it exists, else the relative one). It returns `{ audioPath }` or the user-facing error. Nothing is installed.
2. If a tab already has `audioPath`, activate it and show the "close it first" notice. Otherwise queue a tab for `audioPath` with `sessionPath` set. `loadIntoTab` calls `open_file` as usual.
3. Still inside `loadIntoTab`, before the stats/clip/chain fetches and before the tab is marked ready (so no edit can sneak in), call `restore_session(fileId, sessionPath)`. Then the existing fetches pick up the restored clip, chain and stats with no extra outcome handling. The tab takes `dirty`, `canUndo` and `canRedo` from the returned `EditState`. On error the tab is closed through the existing failed-load path, with a "Could not open session" notice instead of "Could not open file".

`restore_session` re-reads the file (cheap, and keeps the command stateless), then:
1. Compares the recorded format and source frames with the loaded file; a mismatch fails. A differing size or modification time adds a warning.
2. Rebuilds the `EditList`.
3. Loads each effect with the shared loader (below). A missing, disabled, incompatible or failing plugin is skipped with a warning.
4. Refuses if the file's history is no longer the fresh single state (defensive; the frontend never allows it).
5. Replaces the history with `History::restored(edits, chain, slots, bypass_all)`, a single snapshot, and calls `install`, which bumps the revision and starts the background analysis of the edited audio. It then calls `sync_bypass`.

It returns `{ editState, warnings: [{ effect, reason }] }`, and the frontend shows the warnings as one notice.

*Alternative:* one `open_session` command doing the WAV load as well. It would duplicate `open_file`'s progress channel and the tab queue, and lose the existing cancellation on tab close.

### Shared plugin loader and restore order
The body of `add_effect`'s UI-thread closure becomes `AppState::load_slot(plugins, class_id, config, state: Option<&StoredState>) -> Result<Slot, LoadFailure>`. It runs the disabled, availability and `supports` checks, takes the crash marker (`LoadStep::Create`), loads the module and creates the instance. With a state it then:
1. calls `set_state` **before** `configure`, the order DAWs use when loading a project, so the plugin sets up processing with its restored settings and the latency from `configure` is already correct. A `false` result keeps the instance at its defaults and records "settings could not be restored".
2. calls `configure`;
3. re-applies `pendingParams` with `Instance::set_param`, the path generic controls use, so they reach both the controller and the processor.

Then `set_bypassed` restores the slot's switch. `Slot` gains a way to force the next `apply_bypass` to send its value (reset `applied_bypass` to the opposite of the wanted value), because the restored component state may already contain any bypass value.

The crash marker covers creation only, not `set_state`. A crash while reading a bad state is a problem with that session, and disabling the plugin for every file would be the wrong response.

### Frontend
- `Tab` gains `sessionPath: string | null`, set when opened from a session or after Save Session As. It is kept when the WAV is saved or saved as, so the next Save Session records the new WAV path.
- New `src/lib/sessionFiles.ts`: `isSessionPath(path)` (name ends in `.hdsession.json`, ignoring case; other `.json` files are not sessions), `defaultSessionPath(tab)` (`sessionPath`, else the WAV path with `.wav` replaced, so `take.wav` → `take.hdsession.json`) and `withSessionExtension(path)` (`mix` and `mix.json` → `mix.hdsession.json`; a full suffix is kept), with vitest tests. `session.ts` (the launch-time tab restore) is unchanged and keeps storing WAV paths, so a tab opened from a session reopens at launch as its plain WAV.
- `openPaths` accepts both kinds of path. WAVs keep their current behaviour; session paths go through `read_session` first, in the same queue, so the order requested is kept.
- `openDialog` filters: "WAV audio and sessions" (default), "WAV audio", "HitherDither sessions". `main.ts` drag and drop already passes every path to `dropPaths`. The drop hint becomes "Drop WAV or session files to open them".
- `editActions.ts`: `saveSession()` and `saveSessionAs()` share a `writeSession(tab, path)`, with the save dialog filtered to `.hdsession.json`. Both use `duplicatePress`. The dialog result goes through `withSessionExtension`, because native dialogs handle a two-part extension differently (see Risks). If that changes the path and the new path already exists, a confirmation asks before replacing it, since the dialog only checked the name the user typed.
- `menu.ts`: `save-session` (CmdOrCtrl+Alt+S) and `save-session-as` items after Save As, with new `ToggleId`s, enabled when the active tab is ready and not `editing`. The webview key handler checks `altKey` so Ctrl/Cmd+Alt+S isn't taken as Save.

### IPC
| Command | Purpose |
| --- | --- |
| `save_session(fileId, path)` | Capture the tab and write the session file |
| `read_session(path)` | Validate a session and resolve its WAV: `{ audioPath }` |
| `restore_session(fileId, path)` | Apply a session to a freshly opened file: `{ editState, warnings }` |

Errors are user-facing strings, as for the other commands. All three are documented in `docs/ipc.md`.

## Risks / Trade-offs

- [Native dialogs treat the two-part `.hdsession.json` extension differently: Windows and GTK filter on the `*.hdsession.json` pattern, but macOS filters by the last component only, so its Open dialog also lists other `.json` files, and save dialogs may append only `.json` or nothing] → Routing never relies on the dialog: `isSessionPath` checks the full suffix, other JSON files fail like any unreadable file, and `withSessionExtension` fixes the saved name. Check it by hand on each OS during acceptance.

- [A plugin's output depends on more than its saved state (random seeds, time-based modulation, external sample files moved)] → The round trip is exact for the state the plugin reports; anything else is outside the host's reach, and the README says so.
- [Large plugin states (convolution reverbs embedding impulse responses) make big session files] → Acceptable; base64 JSON stays linear in the state size and is written once per save.
- [A session refers to the WAV by path, so renaming the WAV alone breaks it] → The relative path covers moving both together; a locate dialog is a possible follow-up.
- [Same-length re-render of the WAV passes the format check, so edits apply to different audio] → The size/modification-time warning tells the user; hashing the audio was rejected because it means reading the whole file on every save and open.
- [Ctrl+Alt is AltGr on some keyboard layouts, so Ctrl+Alt+S may type a character in inputs] → The handler only acts outside text inputs, as for the other plain shortcuts, and the menu item remains.
- [Restoring a plugin's state may crash it in-process] → The same risk as any plugin call. The crash marker isn't used here, so the plugin isn't disabled for other files.

## Migration Plan

None: the format is new, and nothing existing is changed on disk. Rollback is reverting the change; `.hdsession.json` files left behind are ignored by older builds (their Open dialog only shows WAVs).
