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

- **kind 0 (min/max)**: used when the span is longer than `columns` frames. For each channel there are `count` pairs `[min, max]`, so the payload is `channels × count × 2` floats. Column `i` covers frames `[start + span·i/count, start + span·(i+1)/count)`, and its values are exact (never hide a peak).
- **kind 1 (samples)**: used when the span is at most `columns` frames. For each channel there are `count` consecutive samples starting at `start`, so the payload is `channels × count` floats.

Sample values are normalised to −1.0…+1.0 (integer PCM divided by 2^(bits−1), so positive full scale is slightly below 1.0).

## Statistics

### `compute_stats`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `range` | `{ start, end } \| null` | Frame range; `null` returns the cached whole-file stats |

Returns `Stats | null`. The result is `null` when a newer `compute_stats` request (for any file) superseded this one, and the caller should discard it.

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
}
```

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

```ts
type PlaybackEvent =
  | { kind: "position"; frame: number } // audible file frame, about every 16 ms, latency-compensated
  | { kind: "ended"; frame: number };   // reached `end` (not sent when looping or stopped)
```

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

## Editing

Edits are non-destructive: the file on disk is untouched until it's saved. Each file keeps an undo history from when it was opened (or last saved). `get_metadata`, `get_peaks`, `compute_stats`, `get_clip_runs` and `play` always serve the current edited audio, with frame positions in the edited timeline.

After an edit, undo or redo the backend re-analyses the edited audio (peaks, whole-file stats and clip runs), much as `open_file` does. Only one edit, undo or redo can run per file at a time; another call for the same file meanwhile rejects with `The file is still being analysed`. `close_file` cancels it, and the call then rejects with `The edit was cancelled`. If the file is playing, playback stops first.

These commands return an `EditOutcome`:

```ts
interface EditOutcome {
  metadata: Metadata;      // frames and duration_secs reflect the edited audio
  edit_state: EditState;
  clip: ClipInfo;          // what the clip handles show (see get_clip)
}

interface EditState {
  can_undo: boolean;
  can_redo: boolean;
  dirty: boolean;          // the audio differs from the file on disk
  revision: number;        // increases whenever the audio changes; invalidate cached peaks
}
```

### `apply_edit`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `op` | `"crop" | "delete" | "silence" | "fadeIn" | "fadeOut"` | |
| `start` | number | First frame of the range |
| `end` | number | End of the range (exclusive) |
| `bend` | `number | null` | Fade curve, -1 to 1 (0 or null = linear); only used by `fadeIn`/`fadeOut` |
| `onProgress` | `Channel<LoadProgress>` | Re-analysis progress, as for `open_file` |

Applies the edit to `[start, end)` across all channels and discards the redo history. `delete` joins the two sides with a 10 ms linear crossfade (shortened to what the audio allows), which `set_crossfade` can change. Rejects with `Can't apply the edit: …` for an empty or out-of-range selection, a delete that would leave no audio, or a crop of the whole file.

### `undo_edit` / `redo_edit`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `onProgress` | `Channel<LoadProgress>` | Re-analysis progress |

Step back or forward one edit. Rejects with `Nothing to undo` / `Nothing to redo` at either end of the history.

### `save_file`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `path` | `string | null` | Where to write; `null` overwrites the file's own path |

Writes the current audio as a WAV file: the source's `fmt ` chunk followed by a `data` chunk, and no other chunks. Unedited samples are copied byte for byte. Scaled samples are converted in the file's own format (integers rounded to nearest and clamped). The file is written to a temporary file next to the target and then renamed over it, so a failure never leaves a partial file. If the file is playing, playback stops first.

Returns an `EditOutcome`. Afterwards the file is served from the saved copy (its `metadata.path` and `name` are the target's), the history is empty, and the peaks and stats are unchanged.

Rejects, leaving any existing file at the target and the edits as they were:
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
| `onProgress` | `Channel<LoadProgress>` | Re-analysis progress |

Replaces the trim window and end fades. Values are clamped rather than rejected: at least one frame stays, and the two fades together never exceed the trimmed length. Returns an `EditOutcome`. If the file is playing, playback stops first.

### `set_crossfade`

| Arg | Type | Notes |
| --- | --- | --- |
| `fileId` | number | |
| `join` | number | A join's `index` from `ClipInfo` |
| `xfade` | `FadeSetting` | Length in frames (clamped to `max_len`, rounded down to even) and bend |
| `onProgress` | `Channel<LoadProgress>` | Re-analysis progress |

Replaces one join's crossfade. A length of 0 makes a hard join (it stays a join). Rejects with `Can't change the crossfade: there is no join there` for an index that isn't a join. Returns an `EditOutcome`.

