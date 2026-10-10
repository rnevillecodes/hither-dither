// Typed wrappers for the Rust commands documented in docs/ipc.md.
import { Channel, invoke } from "@tauri-apps/api/core";

export interface Metadata {
  path: string;
  name: string;
  size_bytes: number;
  encoding: "PCM" | "IEEE float";
  bit_depth: number;
  valid_bits: number;
  sample_rate: number;
  channels: number;
  channel_labels: string[];
  frames: number;
  duration_secs: number;
  truncated: boolean;
}

export interface LoadProgress {
  doneFrames: number;
  totalFrames: number;
}

export interface ChannelStats {
  peak_linear: number;
  peak_dbfs: number | null;
  rms_dbfs: number | null;
  dc_offset_percent: number;
  crest_db: number | null;
  clip_runs: number;
}

export type Loudness =
  | { kind: "value"; lufs: number }
  | { kind: "silent" }
  | { kind: "unavailable"; reason: string };

export interface Stats {
  start: number;
  end: number;
  channels: ChannelStats[];
  loudness: Loudness;
}

export interface ClipRun {
  start: number;
  len: number;
}

export interface ClipRuns {
  channels: ClipRun[][];
  capped: boolean;
}

export const PEAKS_KIND_MINMAX = 0;
export const PEAKS_KIND_SAMPLES = 1;

/** A decoded `get_peaks` tile. */
export interface PeakTile {
  kind: typeof PEAKS_KIND_MINMAX | typeof PEAKS_KIND_SAMPLES;
  channels: number;
  /** Columns (min/max) or frames (samples). */
  count: number;
  /** First frame covered. */
  start: number;
  /** Frames requested after clamping; columns divide this span evenly. */
  span: number;
  /** Channel-major values: per channel `count` [min,max] pairs, or `count` samples. */
  values: Float32Array;
}

export function decodePeaks(buf: ArrayBuffer, span: number): PeakTile {
  const view = new DataView(buf);
  const kind = view.getUint32(0, true) as PeakTile["kind"];
  const channels = view.getUint32(4, true);
  const count = view.getUint32(8, true);
  const start = view.getFloat64(16, true);
  const values = new Float32Array(buf, 24, (buf.byteLength - 24) / 4);
  return { kind, channels, count, start, span: kind === PEAKS_KIND_SAMPLES ? count : span, values };
}

export function openFile(fileId: number, path: string, onProgress: (p: LoadProgress) => void): Promise<Metadata> {
  const channel = new Channel<LoadProgress>();
  channel.onmessage = onProgress;
  return invoke<Metadata>("open_file", { fileId, path, onProgress: channel });
}

/** Cancel an `openFile` still running for `fileId`. */
export function cancelOpen(fileId: number): Promise<void> {
  return invoke("cancel_open", { fileId });
}

/** Close `fileId`, stopping playback first if it is playing that file. */
export function closeFile(fileId: number): Promise<void> {
  return invoke("close_file", { fileId });
}

export function getMetadata(fileId: number): Promise<Metadata | null> {
  return invoke("get_metadata", { fileId });
}

export async function getPeaks(
  fileId: number,
  start: number,
  end: number,
  columns: number,
  totalFrames: number,
): Promise<PeakTile> {
  const s = Math.max(0, Math.min(Math.floor(start), totalFrames));
  const e = Math.max(s, Math.min(Math.ceil(end), totalFrames));
  const buf = await invoke<ArrayBuffer>("get_peaks", { fileId, start: s, end: e, columns });
  return decodePeaks(buf, e - s);
}

export function computeStats(fileId: number, range: { start: number; end: number } | null): Promise<Stats | null> {
  return invoke("compute_stats", { fileId, range });
}

export function getClipRuns(fileId: number): Promise<ClipRuns> {
  return invoke("get_clip_runs", { fileId });
}

export interface PlaybackInfo {
  available: boolean;
  reason: string | null;
  device: { name: string; sample_rate: number; channels: number } | null;
}

export type PlaybackEvent = { kind: "position"; frame: number } | { kind: "ended"; frame: number };

export function playbackInfo(): Promise<PlaybackInfo> {
  return invoke("playback_info");
}

export function play(
  fileId: number,
  start: number,
  end: number,
  from: number,
  looping: boolean,
  onEvent: (e: PlaybackEvent) => void,
): Promise<void> {
  const channel = new Channel<PlaybackEvent>();
  channel.onmessage = onEvent;
  return invoke("play", { fileId, start, end, from, looping, onEvent: channel });
}

export function stopPlayback(): Promise<number | null> {
  return invoke("stop_playback");
}

export function setVolume(volume: number): Promise<void> {
  return invoke("set_volume", { volume });
}

export function setLoop(enabled: boolean): Promise<void> {
  return invoke("set_loop", { enabled });
}
