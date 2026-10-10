# IPC command reference

The frontend calls these Tauri commands with `invoke(name, args)` from `@tauri-apps/api/core`. Argument names are camelCase on the JavaScript side. Frame positions are integers (JS numbers are exact up to 2^53).

Commands that return `Result` reject the `invoke` promise with a human-readable string on error.

## File loading

### `open_file`

| Arg | Type | Notes |
| --- | --- | --- |
| `path` | string | Absolute path to a `.wav` file |
| `onProgress` | `Channel<LoadProgress>` | Receives `{ doneFrames, totalFrames }` at most every 50 ms, and once at completion |

Returns `Metadata` (below). On failure, rejects with a message naming the file and the reason (for example `Could not open “x.wav”: the IMA ADPCM (format tag 0x0011) encoding is not supported`), and the previously loaded file stays loaded. A newer `open_file` cancels one still in progress, and the older call then rejects with `Opening “…” was cancelled`.

### `get_metadata`

No arguments. Returns `Metadata | null`.

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
| `range` | `{ start, end } \| null` | Frame range; `null` returns the cached whole-file stats |

Returns `Stats | null`. The result is `null` when a newer `compute_stats` request superseded this one, and the caller should discard it.

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

No arguments. Returns the clipped runs in the whole file, used for waveform markers:

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
| `start` | number | Start of the play range; looping wraps back here |
| `end` | number | Frame to stop at (exclusive); the file length for "play to end" |
| `from` | number | Frame to begin at, within `[start, end]` (e.g. resuming a paused selection) |
| `looping` | boolean | Repeat `[start, end)` until stopped |
| `onEvent` | `Channel<PlaybackEvent>` | Position updates for this playback |

Replaces any current playback. Rejects if no device is available, the output can't be opened, or the file changed on disk since it was loaded.

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
