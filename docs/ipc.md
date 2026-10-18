# IPC command reference

The frontend calls these Tauri commands with `invoke(name, args)` from `@tauri-apps/api/core`. Argument names are camelCase on the JavaScript side. Frame positions are integers (JS numbers are exact up to 2^53).

Commands that return `Result` reject the `invoke` promise with a human-readable string on error.

Several files can be open at once (one per tab). Each is identified by a `fileId`: an unsigned 32-bit integer the frontend picks when it starts opening the file, so it can cancel the load before it finishes. Commands that act on a file take `fileId` and reject with `No file is loaded` if that ID isn't open.

## File loading

### `open_file`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | ID to open the file under |
| `path` | string | Absolute path to a `.wav` file |
| `onProgress` | `Channel<LoadProgress>` | Receives `{ doneFrames, totalFrames }` at most every 50 ms, and once at completion |

Returns `Metadata` (below). On failure, rejects with a message naming the file and the reason (for example `Could not open “x.wav”: the IMA ADPCM (format tag 0x0011) encoding is not supported`). Nothing is installed under `fileId`, and other open files are unaffected. Loads under different IDs are independent. A load cancelled with `cancel_open` or `close_file` (or replaced by another `open_file` with the same ID) rejects with `Opening “…” was cancelled`. On success, any current playback stops.

### `cancel_open`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | The load to cancel |

Cancels an `open_file` still running for `fileId`. Does nothing if there isn't one.

### `close_file`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | The file to close |

Forgets the file, releasing its memory mapping, and cancels its load if one is still running. If playback is playing this file, it stops first. Closing an ID that isn't open does nothing. Its effects' editor windows close first.

### `get_metadata`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |

Returns `Metadata | null` (`null` if `fileId` isn't open).

```ts
interface Metadata {
  path: string;
  name: string;            // file name
  size_bytes: number;
  encoding: "PCM" | "IEEE float";
  bit_depth: number;       // container bits: 8, 16, 24, 32 or 64
  valid_bits: number;      // significant bits (extensible headers may declare fewer)
  sample_rate: number;     // Hz
  channels: number;
  channel_labels: string[];// from WAVE_FORMAT_EXTENSIBLE mask, else Mono / L,R / Ch n
  speaker_arrangement: number; // the VST3 arrangement an effect needs in its layouts (see list_plugins)
  layout: string;          // the channel layout as messages name it: "mono", "stereo", "5.1", "3-channel"…
  frames: number;
  duration_secs: number;
  truncated: boolean;      // data chunk was cut short; show a warning
}
```

## Waveform

### `get_peaks`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `start` | number | First frame (inclusive), clamped to the file |
| `end` | number | Last frame (exclusive), clamped to the file |
| `columns` | number | Pixel columns, 1 to 16384 |

Returns a binary `ArrayBuffer` (raw `tauri::ipc::Response`, not JSON), little-endian:

| Offset | Type | Field |
| --- | --- | --- |
| 0 | u32 | `kind`: 0 = min/max, 1 = raw samples |
| 4 | u32 | `channels` |
| 8 | u32 | `count`: columns (kind 0) or frames (kind 1) |
| 12 | u32 | reserved (0) |
| 16 | f64 | `start`: first frame after clamping |
| 24 | f32[] | values, channel-major |

- **kind 0 (min/max)**: used when the span is longer than `columns` frames. For each channel there are `count` pairs `[min, max]`, so the payload is `channels × count × 2` floats. Column `i` covers frames `[start + span·i/count, start + span·(i+1)/count)`. Its values never hide a peak. They are exact once the current edit state has been analysed (see `wait_analysis`). Before that, they are drawn through the edits from the unedited file's peaks: exact for unchanged audio and wherever a column holds at most 4096 changed frames, and otherwise a bound that may overstate a peak inside a fade or crossfade.
- **kind 1 (samples)**: used when the span is at most `columns` frames. For each channel there are `count` consecutive samples starting at `start`, so the payload is `channels × count` floats.

Sample values are normalised to −1.0…+1.0 (integer PCM divided by 2^(bits−1), so positive full scale is slightly below 1.0).

## Statistics

### `compute_stats`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `range` | `{ start, end } \| null` | Frame range; `null` returns the cached whole-file stats |

Returns `Stats | null`. The result is `null` when a newer `compute_stats` request (for any file) superseded this one, and the caller should discard it. For `range: null` it is also `null` while the current edit state hasn't been analysed yet; show the figures as updating and fetch them again after `wait_analysis`. A range is always computed from the current edited audio.

```ts
interface Stats {
  start: number;
  end: number;
  channels: ChannelStats[];
  loudness:
    | { kind: "value"; lufs: number }
    | { kind: "silent" }                        // display as -inf LUFS
    | { kind: "unavailable"; reason: string };  // display "n/a" with reason as tooltip
}

interface ChannelStats {
  peak_linear: number;
  peak_dbfs: number | null;   // null = silence, display "-inf dBFS"
  rms_dbfs: number | null;    // null = silence
  dc_offset_percent: number;  // mean as a percentage of full scale
  crest_db: number | null;    // peak − RMS in dB; null for silence
  clip_runs: number;          // runs of ≥ 3 consecutive raw full-scale samples
}
```

### `get_clip_runs`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |

Returns the clipped runs in the whole file, used for waveform markers:

```ts
interface ClipRuns {
  channels: { start: number; len: number }[][]; // per channel, frame positions
  capped: boolean; // some channel had more than 10 000 runs; only the first 10 000 are listed
  pending: boolean; // not analysed yet: only the source's runs that survive the edits unchanged
}
```

While `pending`, runs in removed, silenced or gain-changed audio are already gone, and runs after a delete have moved with the audio. Runs that a hard join would create appear once the analysis finishes.

## Playback

Playback runs on the system's default output device. The file is mapped to the device's channels (mono to both sides, extra channels down-mixed with a soft limiter) and resampled when the device cannot run at the file's rate. None of this affects displayed waveforms or statistics.

### `playback_info`

No arguments. Returns:

```ts
interface PlaybackInfo {
  available: boolean;
  reason: string | null;   // why playback is unavailable (e.g. no output device)
  device: { name: string; sample_rate: number; channels: number } | null;
}
```

### `play`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | The file to play |
| `start` | number | Start of the play range; looping wraps back here |
| `end` | number | Frame to stop at (exclusive); the file length for "play to end" |
| `from` | number | Frame to begin at, within `[start, end]` (e.g. resuming a paused selection) |
| `looping` | boolean | Repeat `[start, end)` until stopped |
| `onEvent` | `Channel<PlaybackEvent>` | Position updates for this playback |

Replaces any current playback, whichever file it was playing (only one file plays at a time). Rejects if no device is available, the output can't be opened, or the file changed on disk since it was loaded.

The audio is heard through the file's effect chain (see *Effect chains*), at the file's sample rate before any resampling. Each play starts with the effects' tails and delay lines cleared. Their processing delay is compensated: the first audio heard is at `from`, `position` frames match what is heard, and the end of the range is heard in full before `ended`.

```ts
type PlaybackEvent =
  | { kind: "position"; frame: number; levels: [number, number][] } // audible file frame, about every 16 ms, latency-compensated; levels below
  | { kind: "ended"; frame: number }    // reached `end` (not sent when looping or stopped)
  | { kind: "overload" };               // the effects can't keep up with real time
```

`levels` holds `[peak, rms]` for each of the file's channels (not the device's), as linear amplitudes (1.0 is full scale), of the audio heard since the previous `position`: after the effect chain, before the volume, channel mapping and resampling. `peak` is the largest sample magnitude among the newly heard audio; `rms` is taken over the most recent 300 ms heard (a full-scale sine reads 0.707, as in the Levels statistics). Levels are measured in 256-frame blocks and reported once each block has been heard, so they are in step with `frame` to within a block. Audio skipped unheard when playback follows an edit is never reported. `levels` is empty when nothing more has been heard (before output starts, and after the end).

`overload` is sent at most once per playback, when the output runs dry three or more times within 2 s in the middle of the range while effects are processing. The frontend shows it as a notice that the effects are too demanding to play smoothly; saving still renders them correctly.

### `stop_playback`

No arguments. Stops output and returns the audible frame at that moment (`number | null` if nothing was playing). Used for both Pause (keep the returned position) and Stop (the frontend returns the playhead to where playback began).

### `set_volume`

| Arg | Type | Notes |
| --- | --- | --- |
| `volume` | number | Linear gain 0.0 to 1.0, applied immediately |

### `set_loop`

| Arg | Type | Notes |
| --- | --- | --- |
| `enabled` | boolean | Toggle looping of the current playback range |

### `set_play_range`

| Arg | Type | Notes |
| --- | --- | --- |
| `start` | number | New start of the play range (frames of the audio now playing) |
| `end` | number | New end (exclusive), clamped to the audio |

Changes the range of the current playback without restarting it: playing carries on from the same point, wrapping or ending at the new range. Used when an edit clears or changes the selection being played. Does nothing if nothing is playing.

### Playback follows edits

An edit, undo, redo, clip change or preview of the file that is playing doesn't stop playback. Within about 0.25 s it switches to the new audio with a 5 ms crossfade, carrying on from the same source audio. If that audio was removed, it continues from the join that replaced it, or ends (an `ended` event) if nothing after it is heard. The play range is mapped the same way, and a range covering the whole file becomes the whole new file. `position` events then report frames of the new audio.

Chain changes to the file that is playing (adding, removing, moving or bypassing an effect, Bypass All, or undo and redo of these) switch the same way, with the same audio and range, so they are heard within about 0.25 s. So does a plugin reporting a new processing delay. Bypassing a plugin that has its own bypass parameter, and parameter changes, reach it without a switch, at the start of its next processed block. While effects are processing, less audio is queued ahead (0.15 s instead of 0.2 s) to keep parameter changes within 0.25 s.

## Editing

Edits are non-destructive: the app never writes the file on disk; `export_file` writes the edited audio to a new file. Each file keeps an undo history from when it was opened (or its session restored). `get_metadata`, `get_peaks`, `compute_stats`, `get_clip_runs` and `play` always serve the current edited audio, with frame positions in the edited timeline.

Edits, undo and redo return at once: nothing is analysed or written before they return, and the next edit can follow straight away. If the file is playing, playback follows the change (see *Playback follows edits*). The edited audio is analysed in the background (exact peaks, whole-file stats and clip runs), starting 300 ms after the last change; a further change cancels and restarts it. Until it finishes, `compute_stats(null)` returns `null`, `get_clip_runs` is `pending`, and `get_peaks` draws through the edits. Edits made while the file is being saved reject with `The file is being saved`.

These commands return an `EditOutcome`:

```ts
interface EditOutcome {
  metadata: Metadata;      // frames and duration_secs reflect the edited audio
  edit_state: EditState;
  clip: ClipInfo;          // what the clip handles show (see get_clip)
  chain: ChainInfo;        // the file's effect chain (see get_chain)
  audioChanged: boolean;   // false for chain changes: the revision, peaks and analysis are unchanged,
                           // so don't invalidate peaks or call wait_analysis
}

interface EditState {
  can_undo: boolean;
  can_redo: boolean;
  dirty: boolean;          // unsaved: something changed since the session was last saved or the tab
                           // was opened (edits, effects, bypass, plugin settings)
  edited: boolean;         // the audio is edited (it differs from the WAV on disk), even with nothing to undo
  revision: number;        // increases whenever the audio changes; invalidate cached peaks
}
```

The undo history holds edits and chain changes (adding, removing and moving effects) as one list of steps. It lasts from when the file was opened (or its session restored); saving a session or exporting doesn't clear it.

### `wait_analysis`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `revision` | number | An `edit_state.revision` |

Resolves `true` once that revision's background analysis has finished (at once if it already has, as for a file just opened). Resolves `false` if a newer revision replaces it first or the file is closed. After `true`, fetch the whole-file stats, clip runs and peaks again.

### `apply_edit`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `op` | `"crop" | "delete" | "silence" | "fadeIn" | "fadeOut"` | |
| `start` | number | First frame of the range |
| `end` | number | End of the range (exclusive) |
| `bend` | `number | null` | Fade curve, -1 to 1 (0 or null = linear); only used by `fadeIn`/`fadeOut` |

Applies the edit to `[start, end)` across all channels and discards the redo history. `delete` joins the two sides with a 10 ms linear crossfade (shortened to what the audio allows), which `set_crossfade` can change. Rejects with `Can't apply the edit: …` for an empty or out-of-range selection, a delete that would leave no audio, or a crop of the whole file.

### `undo_edit` / `redo_edit`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |

Step back or forward one edit. Rejects with `Nothing to undo` / `Nothing to redo` at either end of the history.

### `export_file`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `path` | string | Where to write the WAV; never the file's own path |
| `onProgress` | `Channel<LoadProgress>` | Progress of the export, as for `open_file`; `totalFrames` is the length |

This is the only command that renders the edited audio. It writes the current audio as a WAV file: the source's `fmt ` chunk followed by a `data` chunk, and no other chunks. Unedited samples are copied byte for byte. Scaled samples are converted in the file's own format (integers rounded to nearest and clamped). The file is written to a temporary file next to the target and then renamed over it, so a failure never leaves a partial file. Only one export can run per file at a time, and the file can't be changed meanwhile. If the file is playing, playback stops first.

With an active effect chain (an effect that isn't bypassed, and Bypass All off), the audio is rendered through the effects:
- the effects are switched to offline rendering, and parameter changes not yet heard reach them first;
- the chain's latency is removed, so every frame lines up with the input and the length is unchanged;
- every sample is the effects' output, converted to the file's format: integers are rounded and clamped, and `clampedSamples` counts the clamped samples, while floats are written as they are, above full scale included;
- afterwards the effects go back to real-time processing, whether the export succeeded or not.

With a bypassed chain, the export is the same as with no effects, so unedited samples stay bit-exact.

The file is left exactly as it was: the same source, edits, clip settings, chain and plugin settings, history, edit state, peaks and statistics. Nothing describes the exported audio; open it to inspect it.

```ts
interface ExportOutcome {
  clampedSamples: number;  // processed samples clamped to fit an integer format; show a notice when above 0
}
```

Rejects, leaving any existing file at the target as it was:
- with `“x.wav” is the audio being edited, so it can't be overwritten. Export to another name.` when `path` is the file's own path;
- with `Could not export “x.wav”: the result is too large for a WAV file (over 4 GiB)`;
- with `Could not export “x.wav”: …` for I/O errors such as a read-only folder;
- with `The file is still being exported` if an export of the file is already running.

The frontend also refuses targets that are open in another tab.

## Clip handles

The whole file behaves like a clip: a trim window over the edited audio, a fade at each end, and a crossfade at every join that `delete` made. These are settings rather than one-off edits: each call replaces the previous setting and adds one undo step. Calls that leave everything as it was add nothing. Exporting writes them into the exported file and leaves them in place.

A fade curve's `bend` runs from -1 to 1: 0 is linear, positive values bow the fade line upward (louder through the fade), negative downward. The gain at position t (0..1) of a fade-in is t^(4^-bend); a fade-out is its mirror image.

### `get_clip`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |

Returns `ClipInfo | null`:

```ts
interface FadeSetting { len: number; bend: number }  // len in frames; 0 = no fade

interface ClipInfo {
  untrimmed_frames: number; // length before trimming; trim positions are on this timeline
  trim_start: number;
  trim_end: number;         // exclusive; metadata.frames == trim_end - trim_start
  fade_in: FadeSetting;     // from the start of the trimmed audio
  fade_out: FadeSetting;    // ending at the end of the trimmed audio
  joins: {
    index: number;          // identifies the join to set_crossfade
    position: number;       // output frame of the join (outside 0..frames if trimmed away)
    len: number;            // crossfade length in frames (even; half each side; 0 = hard join)
    bend: number;
    max_len: number;        // longest crossfade the audio on both sides allows
  }[];
}
```

### `set_clip`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `clip` | `{ trimStart, trimEnd, fadeIn: FadeSetting, fadeOut: FadeSetting }` | Trim positions on the untrimmed timeline |

Replaces the trim window and end fades. Values are clamped rather than rejected: at least one frame stays, and the two fades together never exceed the trimmed length. Returns an `EditOutcome`. If the file is playing, playback follows the change.

### `set_crossfade`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `join` | number | A join's `index` from `ClipInfo` |
| `xfade` | `FadeSetting` | Length in frames (clamped to `max_len`, rounded down to even) and bend |

Replaces one join's crossfade. A length of 0 makes a hard join (it stays a join). Rejects with `Can't change the crossfade: there is no join there` for an index that isn't a join. Returns an `EditOutcome`.

### `preview_edits`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `preview` | `Preview \| null` | A setting to hear; `null` goes back to the file as it is |

```ts
type Preview =
  | { kind: "clip"; clip: { trimStart, trimEnd, fadeIn: FadeSetting, fadeOut: FadeSetting } }
  | { kind: "crossfade"; join: number; xfade: FadeSetting }
  | { kind: "fade"; op: "fadeIn" | "fadeOut"; start: number; end: number; bend: number };
```

If `fileId` is playing, playback switches to the current edits with `preview` applied, exactly as `set_clip`, `set_crossfade` or `apply_edit` would apply it. Nothing else changes: not the history, the revision, the unsaved state, the peaks or the stats. Used while a handle is dragged and while the fade curve dialog is open. The committing command (or `null`, on cancel) follows. Does nothing if the file isn't playing. Rejects for a `fade` with another `op`, or a crossfade `join` that isn't a join.

## Plugins

VST3 effect plugins are found in the standard folders and the user's extra folders, then scanned in the background, each in its own process. The scan starts when the app starts. Results are remembered in `plugins.json` in the app data folder, so on later launches only new or changed plugins are scanned. `plugin-settings.json` holds the extra folders and the disabled plugins.

### Event `plugins-changed`

A Tauri event (listen with `listen` from `@tauri-apps/api/event`), sent as scan results arrive and when a plugin is enabled or disabled. The payload is the scan progress; call `list_plugins` for the list.

```ts
interface ScanProgress {
  done: number;   // bundles scanned so far in the current or last scan
  total: number;  // bundles that scan is checking; scanning while done < total
}
```

### `list_plugins`

No arguments. Returns every plugin found, sorted by name, and the scan progress. Plugins already scanned are listed while a scan is still running.

```ts
interface PluginList {
  plugins: PluginEntry[];
  progress: ScanProgress;
}

interface PluginEntry {
  path: string;              // the .vst3 bundle
  classId: string | null;    // 32 hex digits; null for a bundle that failed, is incompatible or has no effects
  name: string;              // the effect, or the bundle's file name when classId is null
  vendor: string;
  version: string;
  subCategories: string;     // e.g. "Fx|EQ"
  hasEditor: boolean;        // the plugin has its own editor for this platform
  layouts: Layout[];         // channel layouts it accepts; empty unless ready or disabled
  status: "ready" | "failed" | "incompatible" | "disabled" | "noEffects";
  reason: string | null;     // why it failed or is incompatible, e.g. "Crashed while being scanned",
                             // "Timed out while being scanned", "No version for this Mac's processor",
                             // or "No usable effects" (status "noEffects", e.g. an instrument)
}

interface Layout {
  channels: number;
  arrangement: number;       // VST3 speaker arrangement bits
}
```

A file can use an effect when one of its layouts has the file's arrangement: mono for one channel, the WAV channel mask when it has one bit per channel (5.x with side surrounds counts as back surrounds), stereo for two channels, otherwise the lowest `channels` bits. Only `ready` effects are offered in Add Effect. A bundle with several effects has one entry per effect.

### `plugin_folders`

No arguments. Returns `{ standard: string[]; extra: string[] }`: the standard folders for this OS, which can't be removed (folders that don't exist are listed but skipped), and the user's extra folders.

### `set_plugin_folders`

| Arg | Type | Notes |
| --- | --- | --- |
| `extra` | `string[]` | The new list of extra folders; duplicates and standard folders are dropped |

Saves the folders, starts a rescan and returns the new `{ standard, extra }`. Plugins in a removed folder disappear from the list once the rescan finishes; effects already in a chain keep working. Rejects if the settings can't be saved.

### `rescan_plugins`

| Arg | Type | Notes |
| --- | --- | --- |
| `all` | boolean | `true` scans every plugin again (Rescan All); `false` only new and changed ones |

Starts a rescan in the background and returns at once. A rescan requested while one runs starts when it finishes.

### `set_plugin_enabled`

| Arg | Type | Notes |
| --- | --- | --- |
| `classId` | string | The effect |
| `enabled` | boolean | |

Enables or disables an effect, remembered across restarts. Disabling doesn't remove the effect from chains it is already in. Sends `plugins-changed`. Rejects if the settings can't be saved.

### `take_startup_notices`

No arguments. Returns the plugins disabled at startup because the app closed unexpectedly while loading one of them into a chain or opening its editor, and a notice when the effect presets could not be read (see *Effect presets*). Each notice is returned once.

```ts
interface StartupNotice {
  classId?: string; // the disabled plugin; absent for the presets notice
  name: string;     // the plugin's name; empty for the presets notice
  title: string;    // e.g. "X was disabled", "Effect presets could not be read"
  message: string;  // e.g. "X was disabled because HitherDither closed unexpectedly while loading it. You can enable it again in Manage Plugins."
}
```

## Effect chains

Each open file has a chain of effects, processed in order, with no limit on how many. Neighbouring effects can be in a named group, which never holds another group; grouping doesn't change processing, except that a bypassed group's effects are bypassed. The chain is non-destructive: it doesn't change the edited audio, its revision, peaks or statistics. Adding, removing and moving effects, and grouping, ungrouping, renaming, moving and removing groups, are undo steps, in the same history as edits (see *Editing*); undoing a removal brings back the same effect, or group, with its settings. Bypassing is not an undo step. Effects kept only for undo are released when a new step discards them, and all of a file's effects are released when it closes.

The chain commands below return an `EditOutcome` with `audioChanged: false` and an unchanged `edit_state.revision`, and reject with `The file is being exported` during an export, `That effect isn't in the chain` for a `slotId` that isn't in the chain, or `That group isn't in the chain` for a `groupId` that isn't. A change that leaves the chain as it was adds no undo step.

```ts
interface ChainInfo {
  effects: EffectInfo[];   // in processing order
  bypassAll: boolean;
  latency: number;         // total processing delay in frames of the effects that process
  groups: GroupInfo[];     // in order; each group's effects are neighbours in `effects`
}

interface GroupInfo {
  groupId: number;         // identifies this group in the group commands
  name: string;
  bypassed: boolean;       // the group's own switch; its effects keep their own
  slotIds: number[];       // its effects, in processing order (never empty)
}

interface EffectInfo {
  slotId: number;          // identifies this effect in the chain commands
  classId: string;
  name: string;
  vendor: string;
  hasEditor: boolean;
  bypassed: boolean;       // its own bypass switch (Bypass All is separate)
}
```

### `get_chain`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |

Returns `ChainInfo | null` (`null` if `fileId` isn't open).

### `add_effect`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `classId` | string | A `ready` effect from `list_plugins` |

Loads the effect with its default settings and adds it to the end of the chain, outside any group. Rejects, leaving the chain unchanged:
- with `That effect isn't available. …` for a class that isn't in the plugin list or failed its scan, or `X is disabled. …` for a disabled one;
- with `X doesn't support 5.1 audio` (or `mono`, `stereo`, `quad`, `5.0`, `7.1`, `n-channel`) when none of its layouts matches the file's channels;
- with `“X” could not be loaded: …` when the plugin fails to load or set up.

Loading writes the crash marker the first time a plugin is loaded in a session (see `take_startup_notices`).

### `remove_effect`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `slotId` | number | |

Removes the effect from the chain, and its group if it was the last effect in it. It is kept for undo.

### `move_effect`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `slotId` | number | |
| `group` | number \| null | The group to move it into, or `null` for the top level |
| `index` | number | Its new position among that group's effects, or among the top-level items (effects and groups), counted after it is taken out; past the end means last |

Moves the effect within the chain, into or out of a group. Moving the last effect out of a group removes the group. Moving it to where it already is adds no undo step.

### `group_effects`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `slotIds` | number[] | At least one, in any order |

Puts the effects in a new group named "Group N" (the lowest N no group of the chain is called), in processing order, where the first of them was. Effects already in a group leave it, and a group left empty is removed. If the first effect was in a group that keeps other effects, the new group goes just before that group when none of them came before it, and just after otherwise. Rejects with `No effects are selected` for an empty list.

### `ungroup`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `groupId` | number | |

Removes the group, leaving its effects where it was, in order, with their own bypass switches.

### `rename_group`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `groupId` | number | |
| `name` | string | Trimmed; 1 to 64 characters |

Renames the group. Rejects with `A group name must be 1 to 64 characters`.

### `move_group`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `groupId` | number | |
| `index` | number | Its new position among the top-level items, counted after it is taken out; past the end means last |

Moves the group with its effects.

### `remove_group`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `groupId` | number | |

Removes the group and its effects. They are kept for undo, which brings back the group with its name, bypass switch and effects.

### `set_group_bypass`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `groupId` | number | |
| `on` | boolean | |

Bypasses every effect in the group, keeping each one's own bypass switch and settings. An effect processes only if neither Bypass All, its group's switch nor its own is on. Not an undo step.

### `set_effect_bypass`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `slotId` | number | |
| `on` | boolean | |

Bypasses one effect, keeping its settings. A plugin with a bypass parameter bypasses itself, keeping its latency; otherwise the effect is skipped. Not an undo step.

### `set_bypass_all`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `on` | boolean | |

Bypasses the whole chain, keeping each effect's own bypass switch. Not an undo step.

## Effect presets

A preset is a named group of effects, each with its plugin's complete settings and its own bypass switch, kept by the app in `effect-presets.json` in its data folder, and available in every tab and after restarting. Every change rewrites the file through a temporary file, so it is replaced only once the new contents are complete; a change that can't be written rejects with `The presets could not be saved: …` and changes nothing. A file that can't be read gives an empty library and a startup notice (see `take_startup_notices`), and is left alone until the next change replaces it.

Preset names are 1 to 64 characters after trimming, and are compared ignoring case. A preset keeps its id when renamed. Inserting a preset copies it: the inserted group isn't linked to the preset, either way.

```ts
interface PresetInfo {
  id: number;
  name: string;
  effectCount: number;
}

interface SavePresetOutcome {
  saved: boolean;           // false when another preset has the name and `replace` was false:
                            // nothing changed; ask whether to replace it
  presets: PresetInfo[];    // the library afterwards
}
```

### `list_presets`

No arguments. Returns `PresetInfo[]`, sorted by name ignoring case.

### `save_group_preset`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `groupId` | number | |
| `name` | string | |
| `replace` | boolean | Overwrite a preset that has the name |

Saves the group's effects, in order, as a preset. Doesn't change the file, its history or its unsaved state. Returns `SavePresetOutcome`. Rejects, saving nothing:
- with `That group isn't in the chain`;
- with `A preset name must be 1 to 64 characters`;
- with `“X” could not save its settings` when an effect can't report its state.

### `rename_preset`

| Arg | Type | Notes |
| --- | --- | --- |
| `presetId` | number | |
| `name` | string | |
| `replace` | boolean | Delete another preset that has the name |

Returns `SavePresetOutcome`. Rejects with `That preset no longer exists`, or for an invalid name as `save_group_preset` does. Doesn't change any tab.

### `delete_preset`

| Arg | Type | Notes |
| --- | --- | --- |
| `presetId` | number | |

Returns the remaining `PresetInfo[]`. Rejects with `That preset no longer exists`. Doesn't change any tab; groups inserted from the preset stay as they are.

### `insert_preset`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `presetId` | number | |

Loads the preset's effects, each with its saved state restored before processing is set up, then its unprocessed parameter changes and its bypass switch, and adds them to the end of the chain as a new group named after the preset, not bypassed, as one undo step.

```ts
interface InsertOutcome {
  outcome: EditOutcome;
  warnings: { effect: string; reason: string }[];   // show as one notice
}
```

Effects are left out, and kept at their defaults, as by `restore_session`. Rejects, leaving the chain unchanged:
- with `That preset no longer exists`;
- with `“X” could not be inserted. …`, followed by each effect and the reason it was left out, when none of its effects can be loaded;
- with `The edit was cancelled` if the file was reopened while the effects loaded;
- with `The file is being exported` during an export.

## Effect editors

An effect's controls are shown either in the plugin's own editor, in a separate window the backend creates, or in generic controls the frontend draws. Each effect has at most one editor window, titled "<effect> — <file name>", sized to the plugin's editor and resizable only if the plugin allows. It stays above the main window without blocking it:
- Windows: a window owned by the main window, which follows the display's scale;
- macOS: a child window of the main window;
- Linux: an X11 window (also under XWayland), kept above the main window when the app itself runs on X11.

Editor windows close when their effect leaves the chain (`remove_effect`, `remove_group`, or an undo or redo that takes it out), when their file closes, and when the app quits. Exporting leaves them open. Opening an editor writes the crash marker the first time that plugin opens an editor in a session (see `take_startup_notices`).

### Event `fx-editor-closed`

A Tauri event sent whenever an editor window closes, whether the user closed it or it closed for one of the reasons above.

```ts
interface EditorClosed {
  fileId: number;
  slotId: number;
}
```

### `open_editor`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `slotId` | number | An effect in the file's chain |
| `generic` | boolean | `true` asks for the generic controls only |

Opens the effect's editor window, or brings it to the front if it is already open. Returns:

```ts
interface EditorOpened {
  native: boolean;   // the plugin's own editor is showing
  reason?: string;   // why it isn't, to show with the generic controls
}
```

With `generic: true`, returns `{ native: false }` without a reason and leaves any open editor window as it is. Otherwise `native: false` comes with a `reason` when the plugin has no editor for this platform, its editor fails to open, or (Linux) no X11 display is available. Rejects with `No file is loaded` or `That effect isn't in the chain`.

### `close_editor`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `slotId` | number | |

Closes the effect's editor window if it is open (sending `fx-editor-closed`).

### `set_active_file`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number \| null | The active tab's file; `null` when no tab is active |

Shows the active file's editor windows and hides the others. A hidden window comes back where it was when its tab is active again. Call it on every tab switch.

## Generic controls

Effect parameters drawn by the frontend, for plugins without an editor or on request. Values are normalised: 0 to 1, whatever the parameter's range.

### `get_fx_params`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `slotId` | number | An effect in the file's chain |

Returns the parameters a user can set: those the plugin marks automatable, except hidden, read-only and bypass parameters, in the plugin's order.

```ts
interface FxParam {
  id: number;
  title: string;
  units: string;         // e.g. "dB"; may be empty
  stepCount: number;     // 0: continuous; otherwise the number of steps (1 for an on/off switch)
  defaultValue: number;
  value: number;
  display: string;       // the value as the plugin shows it, without the units, e.g. "-18.0"
}
```

Rejects with `No file is loaded` or `That effect isn't in the chain`.

### `set_fx_param`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `slotId` | number | |
| `paramId` | number | |
| `value` | number | Normalised; clamped to 0–1 |

Sets the parameter on the plugin's controller, so its own editor follows, and for its processor, which hears it from the next block it processes (during playback, within 0.25 s). Returns the new display string. Not an undo step, and doesn't change the edit state. Rejects as `get_fx_params`.

### Event `fx-param-changed`

A Tauri event sent when parameters change in a plugin's own editor, at most 30 times a second per effect, with the latest value of each parameter that changed since the last one. Changes made with `set_fx_param` aren't echoed.

```ts
interface FxParamChanged {
  fileId: number;
  slotId: number;
  changes: { id: number; value: number; display: string }[];
}
```

### Event `fx-params-reset`

A Tauri event sent when a plugin says its parameters' values or titles all changed (for example after loading one of its own presets). Fetch them again with `get_fx_params`.

```ts
interface FxParamsReset {
  fileId: number;
  slotId: number;
}
```

## Session files

A session file (`*.hdsession.json`) holds one tab's state: which WAV it refers to, its unsaved edits, Bypass All, and its effects in order with their bypass switches and complete plugin settings. Saving a session never writes the WAV. The file is versioned JSON; readers ignore fields they don't know. Opening a session takes three calls: `read_session` to find its WAV, `open_file` for that WAV as usual, then `restore_session` before the tab can be edited.

### `save_session`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `path` | string | Where to write the session, ending in `.hdsession.json` |
| `replace` | boolean | Whether an existing file at `path` may be replaced |

Captures the file's current edits and chain and writes them to `path`: each plugin's saved state (`getState`), plus parameter changes it hasn't processed yet (made while stopped). The WAV is described by its absolute path, its path relative to the session's folder, its format and length, and its size and modification time. The session is written to a temporary file next to the target and then renamed over it, so a failure leaves any existing file intact. Playback continues. Once written, the tab counts as saved: its `edit_state.dirty` is measured against the state that was captured, so a change made while saving still counts as unsaved. The undo history is kept.

```ts
interface SessionSaved {
  written: boolean;      // false, writing nothing, if a file exists at `path` and `replace` is false
                         // (the frontend then asks before trying again with `replace`)
  editState: EditState;  // the tab's edit state afterwards
}
```

Rejects:
- with `The file is being exported` while the file is being exported;
- with `“Effect” could not save its settings` if a plugin fails to report its state, writing nothing;
- with `Could not save “mix.hdsession.json”: …` for I/O errors such as a missing or read-only folder.

### `read_session`

| Arg | Type | Notes |
| --- | --- | --- |
| `path` | string | A session file |

Checks the session and finds its WAV, opening nothing. The WAV is the recorded absolute path if that file exists, otherwise the recorded relative path from the session's current folder.

```ts
interface SessionInfo {
  audioPath: string | null;  // open this with open_file, then call restore_session; null when
                             // neither path exists: offer to choose another WAV (relink)
  audioName: string;         // the recorded WAV's file name, for messages
}
```

Rejects:
- with `“x.hdsession.json” is not a valid session file.` for anything that isn't a session this app wrote, including edits that don't fit the recorded length;
- with `“x.hdsession.json” was made by a newer version of HitherDither.` for a higher format version;
- with `Could not read “x.hdsession.json”: …` for I/O errors.

### `restore_session`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | The file just opened from `audioPath`, or from a WAV the user chose |
| `path` | string | The session file |
| `relink` | boolean | True for a WAV the user chose instead of the session's own |

Reads the session again and applies it to the file: the edits, Bypass All, and each effect, loaded with its saved state restored before processing is set up, then its unprocessed parameter changes and its bypass switch. The file's history is replaced by a single state, the restored one, so there is nothing to undo. When the restored edits change the audio, the revision increases and the background analysis starts as after an edit (use `wait_analysis`). Effect editors start closed. Afterwards, fetch the clip, chain, stats and clip runs as after `open_file`.

With `relink`, the edits are restored only if the chosen WAV has the recorded sample rate, channels and layout, sample format and length (it was moved or renamed). Otherwise only Bypass All and the effects are restored, and `editsDropped` says whether the session had edits. Either way the tab is unsaved afterwards, because the session now belongs to another WAV.

```ts
interface RestoreOutcome {
  applied: boolean;                // false (without relink) when the WAV no longer matches: nothing
                                   // changed; close the file and offer to choose another WAV
  editState: EditState;            // dirty follows the usual rule
  wavChanged: boolean;             // the WAV's size or modification time differ: warn
  editsDropped: boolean;           // relink onto a different recording left the edits out: say so
  warnings: { effect: string; reason: string }[];   // show as one notice
}
```

An effect is left out, with a warning, when its plugin isn't available, is disabled, doesn't support the file's channels, or fails to load. A plugin that rejects its saved state stays in the chain at its defaults, with the warning `Its settings could not be restored, so it has its defaults.`

Rejects, leaving the file as opened:
- with the `read_session` messages if the session became unreadable meanwhile;
- with `The edit was cancelled` if the file was edited or reopened while the effects loaded.

### `replace_audio`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `path` | string | The WAV to use instead of the file's own |
| `onProgress` | `Channel<LoadProgress>` | Progress of loading the new WAV, as for `open_file` |

Replaces the file's audio with the WAV at `path`, keeping its effect chain: every effect in order, with its settings (including parameter changes not yet heard) and bypass switch, and Bypass All. The effects are new instances set up for the new audio, so their editors close. The edits are dropped and the history starts again at the new audio, which counts as unsaved; a session the tab belongs to refers to the new WAV once it is saved. Playback stops first. Afterwards, fetch the clip, chain, stats and clip runs as after `open_file`.

```ts
interface ReplaceOutcome {
  metadata: Metadata;      // the new WAV's
  editState: EditState;    // dirty, with nothing to undo
  warnings: { effect: string; reason: string }[];   // effects left out (e.g. the new layout isn't
                                                     // supported) or at their defaults; one notice
}
```

Rejects, leaving the file, its edits, history and effects as they were:
- with `“take.wav” is already this tab's audio` when `path` is the file's own WAV;
- with the `open_file` errors when the new WAV can't be opened;
- with `The file is being exported` during an export.

The frontend also refuses WAVs open in another tab.
