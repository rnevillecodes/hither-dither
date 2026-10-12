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

Forgets the file, releasing its memory mapping, and cancels its load if one is still running. If playback is playing this file, it stops first. Closing an ID that isn't open does nothing.

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
  | { kind: "position"; frame: number } // audible file frame, about every 16 ms, latency-compensated
  | { kind: "ended"; frame: number }    // reached `end` (not sent when looping or stopped)
  | { kind: "overload" };               // the effects can't keep up with real time
```

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

Edits are non-destructive: the file on disk is untouched until it's saved. Each file keeps an undo history from when it was opened (or last saved). `get_metadata`, `get_peaks`, `compute_stats`, `get_clip_runs` and `play` always serve the current edited audio, with frame positions in the edited timeline.

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
  clampedSamples: number;  // from save_file: processed samples clamped to fit an integer format
                           // (0 otherwise); show a notice when above 0
}

interface EditState {
  can_undo: boolean;
  can_redo: boolean;
  dirty: boolean;          // unsaved: the audio is edited, or an effect that isn't bypassed is in the
                           // chain while Bypass All is off
  revision: number;        // increases whenever the audio changes; invalidate cached peaks
}
```

The undo history holds edits and chain changes (adding, removing and moving effects) as one list of steps.

### `wait_analysis`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `revision` | number | An `edit_state.revision` |

Resolves `true` once that revision's background analysis has finished (at once if it already has, as for a file just opened or saved). Resolves `false` if a newer revision replaces it first or the file is closed. After `true`, fetch the whole-file stats, clip runs and peaks again.

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

### `save_file`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `path` | `string | null` | Where to write; `null` overwrites the file's own path |
| `onProgress` | `Channel<LoadProgress>` | Progress of the save, as for `open_file` |

This is the only command that renders the edited audio. Without effects, if the current edit state hasn't been analysed yet, it is analysed first, so the saved tab has exact peaks and stats; progress then covers both passes (`totalFrames` is twice the length). Only one save can run per file at a time. Writes the current audio as a WAV file: the source's `fmt ` chunk followed by a `data` chunk, and no other chunks. Unedited samples are copied byte for byte. Scaled samples are converted in the file's own format (integers rounded to nearest and clamped). The file is written to a temporary file next to the target and then renamed over it, so a failure never leaves a partial file. If the file is playing, playback stops first.

With an active effect chain (an effect that isn't bypassed, and Bypass All off), the audio is rendered through the effects instead:
- the effects are switched to offline rendering, and parameter changes not yet heard reach them first;
- the chain's latency is removed, so every frame lines up with the input and the length is unchanged;
- every sample is the effects' output, converted to the file's format: integers are rounded and clamped, and `clampedSamples` counts the clamped samples, while floats are written as they are, above full scale included;
- the written samples are analysed in the same pass, so there is no separate analysis and progress covers one pass (`totalFrames` is the length).

With a bypassed chain, the save is the same as with no effects, so unedited samples stay bit-exact.

Returns an `EditOutcome`. Afterwards the file is served from the saved copy (its `metadata.path` and `name` are the target's), and the history and the effect chain are empty: the effects are released, and their editors closed. Without effects, the peaks and stats are unchanged. With effects, `audioChanged` is true as always for a save, and the new revision's peaks, stats and clip runs, which describe the processed audio, are already exact.

Rejects, leaving any existing file at the target, the edits, and the chain with its settings as they were (effects go back to real-time processing):
- with `“x.wav” changed on disk since it was opened…` when overwriting a file that changed since it was loaded;
- with `Could not save “x.wav”: the result is too large for a WAV file (over 4 GiB)`;
- with `Could not save “x.wav”: …` for I/O errors such as a read-only folder.

The frontend asks for confirmation before the first overwrite and rejects targets that are open in another tab.

## Clip handles

The whole file behaves like a clip: a trim window over the edited audio, a fade at each end, and a crossfade at every join that `delete` made. These are settings rather than one-off edits: each call replaces the previous setting and adds one undo step. Calls that leave everything as it was add nothing. Saving bakes them into the file, after which the clip state is empty again.

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

No arguments. Returns the plugins disabled at startup because the app closed unexpectedly while loading one of them into a chain or opening its editor. Each notice is returned once.

```ts
interface StartupNotice {
  classId: string;
  name: string;
  message: string;  // e.g. "X was disabled because HitherDither closed unexpectedly while loading it. You can enable it again in Manage Plugins."
}
```

## Effect chains

Each open file has a chain of up to 8 effects, processed in order. The chain is non-destructive: it doesn't change the edited audio, its revision, peaks or statistics. Adding, removing and moving effects are undo steps, in the same history as edits (see *Editing*); undoing a removal brings back the same effect with its settings. Bypassing is not an undo step. Effects kept only for undo are released when a new step discards them, and all of a file's effects are released when it closes.

The chain commands below return an `EditOutcome` with `audioChanged: false` and an unchanged `edit_state.revision`, and reject with `The file is being saved` during a save, or `That effect isn't in the chain` for a `slotId` that isn't in the chain.

```ts
interface ChainInfo {
  effects: EffectInfo[];   // in processing order
  bypassAll: boolean;
  latency: number;         // total processing delay in frames of the effects that process
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

Loads the effect with its default settings and adds it to the end of the chain. Rejects, leaving the chain unchanged:
- with `The effect chain is full` when the chain has 8 effects;
- with `That effect isn't available. …` for a class that isn't in the plugin list or failed its scan, or `X is disabled. …` for a disabled one;
- with `X doesn't support 5.1 audio` (or `mono`, `stereo`, `quad`, `5.0`, `7.1`, `n-channel`) when none of its layouts matches the file's channels;
- with `“X” could not be loaded: …` when the plugin fails to load or set up.

Loading writes the crash marker the first time a plugin is loaded in a session (see `take_startup_notices`).

### `remove_effect`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `slotId` | number | |

Removes the effect from the chain. It is kept for undo.

### `move_effect`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `slotId` | number | |
| `index` | number | Its new position; past the end means last |

Moves the effect within the chain. Moving it to where it already is adds no undo step.

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
