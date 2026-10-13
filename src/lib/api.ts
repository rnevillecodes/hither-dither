// Typed wrappers for the Rust commands documented in docs/ipc.md.
import { Channel, invoke } from "@tauri-apps/api/core";
import { type UnlistenFn, listen } from "@tauri-apps/api/event";

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
  /** The VST3 speaker arrangement an effect needs among its layouts to process the file. */
  speaker_arrangement: number;
  /** The channel layout as messages name it: "mono", "stereo", "5.1"… */
  layout: string;
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
  /** Not analysed yet: only the source's runs that survive the edits unchanged. */
  pending: boolean;
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

/**
 * Resolves true once revision `revision` of `fileId` has been analysed in the background,
 * false if a newer revision replaced it or the file closed first.
 */
export function waitAnalysis(fileId: number, revision: number): Promise<boolean> {
  return invoke("wait_analysis", { fileId, revision });
}

/** Whole-file stats (`range` null) are null until the current edits have been analysed. */
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

export type PlaybackEvent =
  | { kind: "position"; frame: number }
  | { kind: "ended"; frame: number }
  /** The effects can't keep up with real time (at most once per playback). */
  | { kind: "overload" };

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

/** Change the range being played, carrying on from where it is. */
export function setPlayRange(start: number, end: number): Promise<void> {
  return invoke("set_play_range", { start, end });
}

export type EditOp = "crop" | "delete" | "silence" | "fadeIn" | "fadeOut";

export interface EditState {
  can_undo: boolean;
  can_redo: boolean;
  /** The audio differs from the file on disk. */
  dirty: boolean;
  /** Increases whenever the audio changes; cached peaks are stale after a change. */
  revision: number;
}

/** A fade's length in frames (0 = none) and curve bend (-1..1, 0 = linear). */
export interface FadeSetting {
  len: number;
  bend: number;
}

export interface JoinState {
  /** Identifies the join to `setCrossfade`. */
  index: number;
  /** Output frame of the join (outside the audio when trimmed away). */
  position: number;
  /** Crossfade length in frames (even, half each side; 0 = hard join). */
  len: number;
  bend: number;
  max_len: number;
}

/** What the clip handles show; see docs/ipc.md `get_clip`. */
export interface ClipInfo {
  untrimmed_frames: number;
  trim_start: number;
  trim_end: number;
  fade_in: FadeSetting;
  fade_out: FadeSetting;
  joins: JoinState[];
}

export interface EditOutcome {
  metadata: Metadata;
  edit_state: EditState;
  clip: ClipInfo;
  chain: ChainInfo;
  /** False for chain changes: the revision, peaks and analysis are unchanged. */
  audioChanged: boolean;
  /** From a save: processed samples clamped to fit an integer format (0 otherwise). */
  clampedSamples: number;
}

function progressChannel(onProgress: (p: LoadProgress) => void): Channel<LoadProgress> {
  const channel = new Channel<LoadProgress>();
  channel.onmessage = onProgress;
  return channel;
}

/** Apply `op` to frames `[start, end)`. Resolves at once; analysis follows in the background. */
export function applyEdit(fileId: number, op: EditOp, start: number, end: number, bend: number): Promise<EditOutcome> {
  return invoke("apply_edit", { fileId, op, start, end, bend });
}

export function getClip(fileId: number): Promise<ClipInfo | null> {
  return invoke("get_clip", { fileId });
}

function clipArg(clip: ClipInfo) {
  return { trimStart: clip.trim_start, trimEnd: clip.trim_end, fadeIn: clip.fade_in, fadeOut: clip.fade_out };
}

/** Replace the trim window and end fades (trim positions on the untrimmed timeline). */
export function setClip(fileId: number, clip: ClipInfo): Promise<EditOutcome> {
  return invoke("set_clip", { fileId, clip: clipArg(clip) });
}

export function setCrossfade(fileId: number, join: number, xfade: FadeSetting): Promise<EditOutcome> {
  return invoke("set_crossfade", { fileId, join, xfade });
}

export function undoEdit(fileId: number): Promise<EditOutcome> {
  return invoke("undo_edit", { fileId });
}

export function redoEdit(fileId: number): Promise<EditOutcome> {
  return invoke("redo_edit", { fileId });
}

/** A setting being tried out (a handle drag, or the fade curve dialog). */
export type Preview =
  | { kind: "clip"; clip: ClipInfo }
  | { kind: "crossfade"; join: number; xfade: FadeSetting }
  | { kind: "fade"; op: "fadeIn" | "fadeOut"; start: number; end: number; bend: number };

/** Hear `preview` if `fileId` is playing, without changing anything; null goes back. */
export function previewEdits(fileId: number, preview: Preview | null): Promise<void> {
  const arg = preview?.kind === "clip" ? { kind: "clip", clip: clipArg(preview.clip) } : preview;
  return invoke("preview_edits", { fileId, preview: arg });
}

/**
 * Write the file's audio to `path`, or over its own file when `path` is null. This renders
 * the edits (analysing them first if that hasn't finished); `onProgress` covers both.
 */
export function saveFile(fileId: number, path: string | null, onProgress: (p: LoadProgress) => void): Promise<EditOutcome> {
  return invoke("save_file", { fileId, path, onProgress: progressChannel(onProgress) });
}

// --- Plugins ---------------------------------------------------------------------------

export interface ScanProgress {
  /** Bundles scanned so far in the current or last scan. */
  done: number;
  /** Bundles that scan is checking; scanning while done < total. */
  total: number;
}

export interface Layout {
  channels: number;
  /** VST3 speaker arrangement bits. */
  arrangement: number;
}

export type PluginStatus = "ready" | "failed" | "incompatible" | "disabled" | "noEffects";

export interface PluginEntry {
  /** The .vst3 bundle. */
  path: string;
  /** Null for a bundle that failed, is incompatible or has no effects. */
  classId: string | null;
  name: string;
  vendor: string;
  version: string;
  subCategories: string;
  hasEditor: boolean;
  layouts: Layout[];
  status: PluginStatus;
  reason: string | null;
}

export interface PluginList {
  plugins: PluginEntry[];
  progress: ScanProgress;
}

export interface PluginFolders {
  standard: string[];
  extra: string[];
}

export interface StartupNotice {
  classId: string;
  name: string;
  message: string;
}

export function listPlugins(): Promise<PluginList> {
  return invoke("list_plugins");
}

export function pluginFolders(): Promise<PluginFolders> {
  return invoke("plugin_folders");
}

export function setPluginFolders(extra: string[]): Promise<PluginFolders> {
  return invoke("set_plugin_folders", { extra });
}

/** Rescan new and changed plugins, or every plugin with `all`. Returns at once. */
export function rescanPlugins(all: boolean): Promise<void> {
  return invoke("rescan_plugins", { all });
}

export function setPluginEnabled(classId: string, enabled: boolean): Promise<void> {
  return invoke("set_plugin_enabled", { classId, enabled });
}

/** Plugins disabled at startup after a crash; each is returned once. */
export function takeStartupNotices(): Promise<StartupNotice[]> {
  return invoke("take_startup_notices");
}

/** Scan results arrived, or a plugin was enabled or disabled. */
export function onPluginsChanged(f: (progress: ScanProgress) => void): Promise<UnlistenFn> {
  return listen<ScanProgress>("plugins-changed", (e) => f(e.payload));
}

// --- Effect chains ---------------------------------------------------------------------

export interface EffectInfo {
  slotId: number;
  classId: string;
  name: string;
  vendor: string;
  hasEditor: boolean;
  /** Its own bypass switch (Bypass All is separate). */
  bypassed: boolean;
}

export interface ChainInfo {
  /** In processing order. */
  effects: EffectInfo[];
  bypassAll: boolean;
  /** Total processing delay in frames. */
  latency: number;
}

/** Most effects a chain can hold. */
export const MAX_EFFECTS = 8;

export function getChain(fileId: number): Promise<ChainInfo | null> {
  return invoke("get_chain", { fileId });
}

export function addEffect(fileId: number, classId: string): Promise<EditOutcome> {
  return invoke("add_effect", { fileId, classId });
}

export function removeEffect(fileId: number, slotId: number): Promise<EditOutcome> {
  return invoke("remove_effect", { fileId, slotId });
}

export function moveEffect(fileId: number, slotId: number, index: number): Promise<EditOutcome> {
  return invoke("move_effect", { fileId, slotId, index });
}

export function setEffectBypass(fileId: number, slotId: number, on: boolean): Promise<EditOutcome> {
  return invoke("set_effect_bypass", { fileId, slotId, on });
}

export function setBypassAll(fileId: number, on: boolean): Promise<EditOutcome> {
  return invoke("set_bypass_all", { fileId, on });
}

// --- Effect editors and generic controls -----------------------------------------------

export interface EditorOpened {
  /** The plugin's own editor is showing; otherwise show the generic controls. */
  native: boolean;
  /** Why the plugin's own editor isn't showing, to show with the generic controls. */
  reason?: string;
}

export interface FxParam {
  id: number;
  title: string;
  units: string;
  /** 0: continuous; otherwise the number of steps (1 for an on/off switch). */
  stepCount: number;
  defaultValue: number;
  /** Normalised, 0 to 1. */
  value: number;
  /** The value as the plugin shows it, without the units. */
  display: string;
}

export interface FxParamChanged {
  fileId: number;
  slotId: number;
  changes: { id: number; value: number; display: string }[];
}

export interface EffectRef {
  fileId: number;
  slotId: number;
}

/** Show an effect's own editor (or with `generic`, ask for the generic controls only). */
export function openEditor(fileId: number, slotId: number, generic: boolean): Promise<EditorOpened> {
  return invoke("open_editor", { fileId, slotId, generic });
}

export function closeEditor(fileId: number, slotId: number): Promise<void> {
  return invoke("close_editor", { fileId, slotId });
}

/** Show the active tab's editor windows and hide the others. */
export function setActiveFile(fileId: number | null): Promise<void> {
  return invoke("set_active_file", { fileId });
}

export function getFxParams(fileId: number, slotId: number): Promise<FxParam[]> {
  return invoke("get_fx_params", { fileId, slotId });
}

/** Set a parameter (normalised); resolves with its new display string. */
export function setFxParam(fileId: number, slotId: number, paramId: number, value: number): Promise<string> {
  return invoke("set_fx_param", { fileId, slotId, paramId, value });
}

/** An effect's editor window closed, by the user or because the effect went. */
export function onEditorClosed(f: (e: EffectRef) => void): Promise<UnlistenFn> {
  return listen<EffectRef>("fx-editor-closed", (e) => f(e.payload));
}

/** Parameters changed in a plugin's own editor. */
export function onFxParamChanged(f: (e: FxParamChanged) => void): Promise<UnlistenFn> {
  return listen<FxParamChanged>("fx-param-changed", (e) => f(e.payload));
}

/** A plugin's parameters all changed: fetch them again. */
export function onFxParamsReset(f: (e: EffectRef) => void): Promise<UnlistenFn> {
  return listen<EffectRef>("fx-params-reset", (e) => f(e.payload));
}
