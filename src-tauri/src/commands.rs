//! Tauri command handlers. Heavy work runs on blocking worker threads; see `docs/ipc.md`.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::ipc::{Channel, Response};
use tauri::{AppHandle, Emitter, Runtime, State};

use crate::fx::chain::{ChainInfo, SlotId};
use crate::fx::live::LiveChain;
use crate::peaks::query_edited;
use crate::playback::{PlayRequest, PlaybackInfo, Player};
use crate::plugins::ClassId;
use crate::plugins::cache::{Folders, PluginCache, PluginList, StartupNotice, standard_folders};
use crate::plugins::editor::{EditorOpened, Editors};
use crate::plugins::instance::Param;
use crate::plugins::scan::Scanner;
use crate::session::SessionFile;
use crate::state::{
    AppState, ClipInfo, ClipRuns, ClipSettings, EditOp, EditOutcome, FadeSetting, FileId,
    LoadedFile, Metadata, Preview, RestoreOutcome,
};
use crate::stats::Stats;

pub type SharedState = Arc<AppState>;
pub type SharedPlayer = Arc<Player>;

/// Interval between playhead position events (~60 Hz).
const POSITION_INTERVAL: Duration = Duration::from_millis(16);

/// Largest column count `get_peaks` accepts.
pub const MAX_COLUMNS: u32 = 16_384;
pub const PEAKS_KIND_MINMAX: u32 = 0;
pub const PEAKS_KIND_SAMPLES: u32 = 1;
pub const PEAKS_HEADER_BYTES: usize = 24;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadProgress {
    pub done_frames: u64,
    pub total_frames: u64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct FrameRange {
    pub start: u64,
    pub end: u64,
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("Worker failed: {e}"))
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum PlaybackEvent {
    /// Audible file frame (latency-compensated).
    Position { frame: f64 },
    /// Playback reached the end of the requested range.
    Ended { frame: f64 },
    /// The effects are too demanding to play smoothly (sent at most once per playback).
    Overload,
}

/// Forward `(done, total)` progress to `channel` at most every 50 ms, and at completion.
fn throttled(channel: Channel<LoadProgress>) -> impl FnMut(u64, u64) {
    let mut last = Instant::now();
    move |done, total| {
        if done == total || last.elapsed() >= Duration::from_millis(50) {
            last = Instant::now();
            let _ = channel.send(LoadProgress {
                done_frames: done,
                total_frames: total,
            });
        }
    }
}

#[tauri::command]
pub async fn open_file(
    state: State<'_, SharedState>,
    player: State<'_, SharedPlayer>,
    file_id: FileId,
    path: PathBuf,
    on_progress: Channel<LoadProgress>,
) -> Result<Metadata, String> {
    let state = state.inner().clone();
    let player = player.inner().clone();
    blocking(move || {
        state
            .open(file_id, &path, throttled(on_progress))
            .inspect(|_| {
                player.stop();
            })
            .map_err(|e| e.to_string())
    })
    .await?
}

/// Stop playback if it is playing `file_id`, before its file is replaced.
fn stop_playing(state: &AppState, player: &Player, file_id: FileId) {
    if let Some(file) = state.get(file_id) {
        player.stop_if_playing(&file.audio);
    }
}

/// If `file_id` is playing, switch playback to its current audio and effects.
pub fn follow_playing(state: &AppState, player: &Player, file_id: FileId) {
    if let Some(file) = state.get(file_id) {
        player.follow(&file.audio, LiveChain::of(&file.chain));
    }
}

/// Run a change to `file_id` and let playback follow it.
async fn edit(
    state: State<'_, SharedState>,
    player: State<'_, SharedPlayer>,
    file_id: FileId,
    change: impl FnOnce(&AppState) -> Result<EditOutcome, String> + Send + 'static,
) -> Result<EditOutcome, String> {
    let state = state.inner().clone();
    let player = player.inner().clone();
    blocking(move || {
        let outcome = change(&state)?;
        follow_playing(&state, &player, file_id);
        Ok(outcome)
    })
    .await?
}

/// Apply `op` to frames `[start, end)` of `file_id`. `bend` shapes fades (default
/// linear). Returns at once; the result is analysed in the background.
#[tauri::command]
#[allow(clippy::too_many_arguments)] // Tauri maps each argument to an IPC field.
pub async fn apply_edit(
    state: State<'_, SharedState>,
    player: State<'_, SharedPlayer>,
    file_id: FileId,
    op: EditOp,
    start: u64,
    end: u64,
    bend: Option<f64>,
) -> Result<EditOutcome, String> {
    let bend = bend.unwrap_or(0.0);
    edit(state, player, file_id, move |s| {
        s.apply_edit(file_id, op, start, end, bend)
    })
    .await
}

/// Replace `file_id`'s trim window and end fades, as set by the clip handles.
#[tauri::command]
pub async fn set_clip(
    state: State<'_, SharedState>,
    player: State<'_, SharedPlayer>,
    file_id: FileId,
    clip: ClipSettings,
) -> Result<EditOutcome, String> {
    edit(state, player, file_id, move |s| s.set_clip(file_id, clip)).await
}

/// Replace the crossfade at join `join` of `file_id`.
#[tauri::command]
pub async fn set_crossfade(
    state: State<'_, SharedState>,
    player: State<'_, SharedPlayer>,
    file_id: FileId,
    join: usize,
    xfade: FadeSetting,
) -> Result<EditOutcome, String> {
    edit(state, player, file_id, move |s| {
        s.set_crossfade(file_id, join, xfade)
    })
    .await
}

/// Hear `preview` (a handle being dragged, or the fade curve dialog) if `file_id` is
/// playing, without changing the file or its history. `None` goes back to the file as
/// it is.
#[tauri::command]
pub async fn preview_edits(
    state: State<'_, SharedState>,
    player: State<'_, SharedPlayer>,
    file_id: FileId,
    preview: Option<Preview>,
) -> Result<(), String> {
    let state = state.inner().clone();
    let player = player.inner().clone();
    blocking(move || {
        match preview {
            Some(p) => player.follow(&state.preview(file_id, &p)?, state.live_chain(file_id)),
            None => follow_playing(&state, &player, file_id),
        }
        Ok(())
    })
    .await?
}

#[tauri::command]
pub fn get_clip(state: State<'_, SharedState>, file_id: FileId) -> Option<ClipInfo> {
    state.clip(file_id)
}

#[tauri::command]
pub async fn undo_edit(
    state: State<'_, SharedState>,
    player: State<'_, SharedPlayer>,
    editors: State<'_, SharedEditors>,
    file_id: FileId,
) -> Result<EditOutcome, String> {
    chain_change(state, player, editors, file_id, move |s| s.undo(file_id)).await
}

#[tauri::command]
pub async fn redo_edit(
    state: State<'_, SharedState>,
    player: State<'_, SharedPlayer>,
    editors: State<'_, SharedEditors>,
    file_id: FileId,
) -> Result<EditOutcome, String> {
    chain_change(state, player, editors, file_id, move |s| s.redo(file_id)).await
}

/// Write `file_id`'s audio to `path`, or over its own file when `path` is null.
#[tauri::command]
pub async fn save_file(
    state: State<'_, SharedState>,
    player: State<'_, SharedPlayer>,
    editors: State<'_, SharedEditors>,
    file_id: FileId,
    path: Option<PathBuf>,
    on_progress: Channel<LoadProgress>,
) -> Result<EditOutcome, String> {
    let state = state.inner().clone();
    let player = player.inner().clone();
    let editors = editors.inner().clone();
    blocking(move || {
        // Playback holds the file's mapping, which must be released to replace it.
        stop_playing(&state, &player, file_id);
        let saved = state.save(file_id, path.as_deref(), throttled(on_progress));
        // A successful save empties the chain.
        close_removed_editors(&state, &editors, file_id);
        saved
    })
    .await?
}

/// Save `file_id`'s edits and effects, with every plugin's settings, as a session file at
/// `path`. The WAV is not written. Resolves false, writing nothing, if `path` exists and
/// `replace` is false.
#[tauri::command]
pub async fn save_session(
    state: State<'_, SharedState>,
    file_id: FileId,
    path: PathBuf,
    replace: bool,
) -> Result<bool, String> {
    let state = state.inner().clone();
    blocking(move || state.save_session(file_id, &path, replace)).await?
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    /// The WAV to open for the session: where it was, or next to the session if moved.
    pub audio_path: PathBuf,
}

/// Check the session file at `path` and find its WAV. Nothing is opened.
#[tauri::command]
pub async fn read_session(path: PathBuf) -> Result<SessionInfo, String> {
    blocking(move || {
        let session = SessionFile::read(&path).map_err(|e| e.to_string())?;
        let audio_path = session.resolve_audio(&path).map_err(|e| e.to_string())?;
        Ok(SessionInfo { audio_path })
    })
    .await?
}

/// Apply the session at `path` to `file_id`, just opened from the session's WAV.
#[tauri::command]
pub async fn restore_session(
    state: State<'_, SharedState>,
    plugins: State<'_, SharedPlugins>,
    file_id: FileId,
    path: PathBuf,
) -> Result<RestoreOutcome, String> {
    let (state, plugins) = (state.inner().clone(), plugins.inner().clone());
    blocking(move || state.restore_session(file_id, &path, &plugins)).await?
}

/// Cancel an `open_file` still running for `file_id`; that call then rejects as cancelled.
#[tauri::command]
pub fn cancel_open(state: State<'_, SharedState>, file_id: FileId) {
    state.cancel_open(file_id);
}

/// Close `file_id` (cancelling its load if one is running), stopping playback first if
/// it is playing that file so the mapping is released.
#[tauri::command]
pub async fn close_file(
    state: State<'_, SharedState>,
    player: State<'_, SharedPlayer>,
    editors: State<'_, SharedEditors>,
    file_id: FileId,
) -> Result<(), String> {
    let state = state.inner().clone();
    let player = player.inner().clone();
    let editors = editors.inner().clone();
    blocking(move || {
        let _ = editors.retain(file_id, Vec::new());
        if let Some(file) = state.close(file_id) {
            player.stop_if_playing(&file.audio);
        }
    })
    .await
}

/// Resolve once revision `revision` of `file_id` has been analysed (`true`), or when a
/// newer revision replaces it or the file closes (`false`).
#[tauri::command]
pub async fn wait_analysis(
    state: State<'_, SharedState>,
    file_id: FileId,
    revision: u64,
) -> Result<bool, String> {
    let state = state.inner().clone();
    blocking(move || state.wait_analysis(file_id, revision)).await
}

#[tauri::command]
pub fn get_metadata(state: State<'_, SharedState>, file_id: FileId) -> Option<Metadata> {
    state.get(file_id).map(|f| f.metadata.clone())
}

#[tauri::command]
pub async fn get_peaks(
    state: State<'_, SharedState>,
    file_id: FileId,
    start: u64,
    end: u64,
    columns: u32,
) -> Result<Response, String> {
    let file = state.get(file_id).ok_or("No file is loaded")?;
    let bytes = blocking(move || encode_peaks(&file, start, end, columns)).await?;
    Ok(Response::new(bytes))
}

#[tauri::command]
pub async fn compute_stats(
    state: State<'_, SharedState>,
    file_id: FileId,
    range: Option<FrameRange>,
) -> Result<Option<Stats>, String> {
    let state = state.inner().clone();
    blocking(move || state.compute_stats(file_id, range.map(|r| (r.start, r.end)))).await?
}

#[tauri::command]
pub fn get_clip_runs(state: State<'_, SharedState>, file_id: FileId) -> Result<ClipRuns, String> {
    state.clip_runs(file_id)
}

#[tauri::command]
pub fn playback_info(player: State<'_, SharedPlayer>) -> PlaybackInfo {
    player.info()
}

/// Play from `from` to `end`; with `looping`, wrap back to `start` until stopped.
#[tauri::command]
#[allow(clippy::too_many_arguments)] // Tauri maps each argument to an IPC field.
pub async fn play(
    state: State<'_, SharedState>,
    player: State<'_, SharedPlayer>,
    file_id: FileId,
    start: u64,
    end: u64,
    from: u64,
    looping: bool,
    on_event: Channel<PlaybackEvent>,
) -> Result<(), String> {
    let file = state.get(file_id).ok_or("No file is loaded")?;
    let state = state.inner().clone();
    let player = player.inner().clone();
    blocking(move || {
        if file.changed_on_disk() {
            return Err(format!(
                "“{}” changed on disk since it was opened. Reopen it to play.",
                file.metadata.name
            ));
        }
        let fx = state.start_live_chain(file_id);
        let shared = player.play(
            file.audio.clone(),
            PlayRequest { start, end, from },
            looping,
            fx,
        )?;
        std::thread::spawn(move || {
            use std::sync::atomic::Ordering;
            while !shared.stop.load(Ordering::Relaxed) {
                if shared.take_overload() && on_event.send(PlaybackEvent::Overload).is_err() {
                    break;
                }
                if shared.finished.load(Ordering::Acquire) {
                    player.finish(&shared);
                    let _ = on_event.send(PlaybackEvent::Ended {
                        frame: shared.range().1 as f64,
                    });
                    break;
                }
                let event = PlaybackEvent::Position {
                    frame: shared.position(),
                };
                if on_event.send(event).is_err() {
                    break;
                }
                std::thread::sleep(POSITION_INTERVAL);
            }
        });
        Ok(())
    })
    .await?
}

/// Stop playback (used for both pause and stop). Returns the audible frame, if playing.
#[tauri::command]
pub async fn stop_playback(player: State<'_, SharedPlayer>) -> Result<Option<f64>, String> {
    let player = player.inner().clone();
    blocking(move || player.stop()).await
}

#[tauri::command]
pub fn set_volume(player: State<'_, SharedPlayer>, volume: f32) {
    player.set_volume(volume);
}

/// Change the range being played to `[start, end)`, carrying on from where it is.
#[tauri::command]
pub fn set_play_range(player: State<'_, SharedPlayer>, start: u64, end: u64) {
    player.set_play_range(start, end);
}

#[tauri::command]
pub fn set_loop(player: State<'_, SharedPlayer>, enabled: bool) {
    player.set_looping(enabled);
}

/// Encode a waveform tile for frames `[start, end)` (see `docs/ipc.md` for the layout).
/// Spans of at most `columns` frames are sent as raw samples; longer spans as per-column
/// min/max pairs.
pub fn encode_peaks(file: &LoadedFile, start: u64, end: u64, columns: u32) -> Vec<u8> {
    let frames = file.audio.frames();
    let end = end.min(frames);
    let start = start.min(end);
    let columns = columns.clamp(1, MAX_COLUMNS) as usize;
    let ch = file.audio.channels();
    let span = (end - start) as usize;

    let (kind, count, values) = if span <= columns {
        let mut interleaved = Vec::new();
        file.audio.read(start, span, &mut interleaved);
        let mut planar = Vec::with_capacity(interleaved.len());
        for c in 0..ch {
            planar.extend(interleaved.iter().skip(c).step_by(ch));
        }
        (PEAKS_KIND_SAMPLES, span, planar)
    } else {
        let mm = match &file.analysis {
            Some(a) => a.pyramid.query(&file.audio, start, end, columns),
            // Not analysed yet: drawn through the edits from the source's peaks.
            None => query_edited(&file.source_pyramid, &file.audio, start, end, columns),
        };
        let values = mm.iter().flat_map(|m| [m.min, m.max]).collect();
        (PEAKS_KIND_MINMAX, columns, values)
    };

    let mut out = Vec::with_capacity(PEAKS_HEADER_BYTES + values.len() * 4);
    out.extend_from_slice(&kind.to_le_bytes());
    out.extend_from_slice(&(ch as u32).to_le_bytes());
    out.extend_from_slice(&(count as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(start as f64).to_le_bytes());
    for v in values {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

// --- Plugins -------------------------------------------------------------------------------

pub type SharedPlugins = Arc<PluginCache>;

/// Sent with the scan progress whenever the plugin list changes.
pub const PLUGINS_CHANGED: &str = "plugins-changed";

/// Opens the plugin cache in the app data directory, scanning with this executable.
pub fn open_plugin_cache(dir: PathBuf) -> std::io::Result<SharedPlugins> {
    let scanner = Scanner::new(std::env::current_exe()?);
    Ok(Arc::new(PluginCache::open(
        dir,
        standard_folders(),
        scanner,
    )))
}

/// Rescans in the background, emitting `plugins-changed` as results arrive.
pub fn start_plugin_scan<R: Runtime>(app: &AppHandle<R>, plugins: &SharedPlugins, all: bool) {
    let app = app.clone();
    plugins.start_rescan(all, move |progress| {
        let _ = app.emit(PLUGINS_CHANGED, progress);
    });
}

#[tauri::command]
pub fn list_plugins(plugins: State<'_, SharedPlugins>) -> PluginList {
    plugins.list()
}

#[tauri::command]
pub fn plugin_folders(plugins: State<'_, SharedPlugins>) -> Folders {
    plugins.folders()
}

#[tauri::command]
pub fn set_plugin_folders<R: Runtime>(
    app: AppHandle<R>,
    plugins: State<'_, SharedPlugins>,
    extra: Vec<PathBuf>,
) -> Result<Folders, String> {
    plugins
        .set_extra_folders(extra)
        .map_err(|e| format!("Couldn't save the plugin folders: {e}"))?;
    start_plugin_scan(&app, &plugins, false);
    Ok(plugins.folders())
}

#[tauri::command]
pub fn rescan_plugins<R: Runtime>(app: AppHandle<R>, plugins: State<'_, SharedPlugins>, all: bool) {
    start_plugin_scan(&app, &plugins, all);
}

#[tauri::command]
pub fn set_plugin_enabled<R: Runtime>(
    app: AppHandle<R>,
    plugins: State<'_, SharedPlugins>,
    class_id: ClassId,
    enabled: bool,
) -> Result<(), String> {
    plugins
        .set_enabled(class_id, enabled)
        .map_err(|e| format!("Couldn't save the plugin settings: {e}"))?;
    let _ = app.emit(PLUGINS_CHANGED, plugins.progress());
    Ok(())
}

#[tauri::command]
pub fn take_startup_notices(plugins: State<'_, SharedPlugins>) -> Vec<StartupNotice> {
    plugins.take_notices()
}

// --- Effect chains -------------------------------------------------------------------------

/// Run a change that may change `file_id`'s chain on a worker thread (plugin calls wait for
/// the UI thread), let playback hear it at once, and close the editors of effects that left
/// the chain.
async fn chain_change(
    state: State<'_, SharedState>,
    player: State<'_, SharedPlayer>,
    editors: State<'_, SharedEditors>,
    file_id: FileId,
    change: impl FnOnce(&AppState) -> Result<EditOutcome, String> + Send + 'static,
) -> Result<EditOutcome, String> {
    let state = state.inner().clone();
    let player = player.inner().clone();
    let editors = editors.inner().clone();
    blocking(move || {
        let outcome = change(&state)?;
        follow_playing(&state, &player, file_id);
        close_removed_editors(&state, &editors, file_id);
        Ok(outcome)
    })
    .await?
}

/// Closes the editors of `file_id`'s effects that are no longer in its chain.
fn close_removed_editors(state: &AppState, editors: &SharedEditors, file_id: FileId) {
    let keep = state
        .get_chain(file_id)
        .map_or_else(Vec::new, |c| c.effects.iter().map(|e| e.slot_id).collect());
    let _ = editors.retain(file_id, keep);
}

#[tauri::command]
pub fn get_chain(state: State<'_, SharedState>, file_id: FileId) -> Option<ChainInfo> {
    state.get_chain(file_id)
}

#[tauri::command]
pub async fn add_effect(
    state: State<'_, SharedState>,
    player: State<'_, SharedPlayer>,
    editors: State<'_, SharedEditors>,
    plugins: State<'_, SharedPlugins>,
    file_id: FileId,
    class_id: ClassId,
) -> Result<EditOutcome, String> {
    let plugins = plugins.inner().clone();
    chain_change(state, player, editors, file_id, move |s| {
        s.add_effect(file_id, &plugins, class_id)
    })
    .await
}

#[tauri::command]
pub async fn remove_effect(
    state: State<'_, SharedState>,
    player: State<'_, SharedPlayer>,
    editors: State<'_, SharedEditors>,
    file_id: FileId,
    slot_id: SlotId,
) -> Result<EditOutcome, String> {
    chain_change(state, player, editors, file_id, move |s| {
        s.remove_effect(file_id, slot_id)
    })
    .await
}

#[tauri::command]
pub async fn move_effect(
    state: State<'_, SharedState>,
    player: State<'_, SharedPlayer>,
    editors: State<'_, SharedEditors>,
    file_id: FileId,
    slot_id: SlotId,
    index: usize,
) -> Result<EditOutcome, String> {
    chain_change(state, player, editors, file_id, move |s| {
        s.move_effect(file_id, slot_id, index)
    })
    .await
}

#[tauri::command]
pub async fn set_effect_bypass(
    state: State<'_, SharedState>,
    player: State<'_, SharedPlayer>,
    editors: State<'_, SharedEditors>,
    file_id: FileId,
    slot_id: SlotId,
    on: bool,
) -> Result<EditOutcome, String> {
    chain_change(state, player, editors, file_id, move |s| {
        s.set_effect_bypass(file_id, slot_id, on)
    })
    .await
}

#[tauri::command]
pub async fn set_bypass_all(
    state: State<'_, SharedState>,
    player: State<'_, SharedPlayer>,
    editors: State<'_, SharedEditors>,
    file_id: FileId,
    on: bool,
) -> Result<EditOutcome, String> {
    chain_change(state, player, editors, file_id, move |s| {
        s.set_bypass_all(file_id, on)
    })
    .await
}

// --- Effect editors ------------------------------------------------------------------------

pub type SharedEditors = Arc<Editors>;

/// Sent when an effect's editor window closes, by the user or because its effect went.
pub const FX_EDITOR_CLOSED: &str = "fx-editor-closed";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorClosed {
    pub file_id: FileId,
    pub slot_id: SlotId,
}

/// Show an effect's controls: its own editor window, or with `generic` (or when that editor
/// can't be shown) `native: false` for the frontend's generic controls.
#[tauri::command]
pub async fn open_editor(
    state: State<'_, SharedState>,
    editors: State<'_, SharedEditors>,
    plugins: State<'_, SharedPlugins>,
    file_id: FileId,
    slot_id: SlotId,
    generic: bool,
) -> Result<EditorOpened, String> {
    let state = state.inner().clone();
    let editors = editors.inner().clone();
    let plugins = plugins.inner().clone();
    blocking(move || {
        let (slot, file_name) = state.effect(file_id, slot_id)?;
        if generic {
            return Ok(EditorOpened {
                native: false,
                reason: None,
            });
        }
        let title = format!("{} — {file_name}", slot.name());
        editors
            .open(file_id, slot, title, plugins)
            .map_err(|e| e.to_string())
    })
    .await?
}

#[tauri::command]
pub async fn close_editor(
    editors: State<'_, SharedEditors>,
    file_id: FileId,
    slot_id: SlotId,
) -> Result<(), String> {
    let editors = editors.inner().clone();
    blocking(move || editors.close(file_id, slot_id).map_err(|e| e.to_string())).await?
}

// --- Generic controls ----------------------------------------------------------------------

/// Sent, at most 30 times a second per effect, with parameters changed in a plugin's own
/// editor.
pub const FX_PARAM_CHANGED: &str = "fx-param-changed";
/// Sent when a plugin says its parameters' values or titles all changed.
pub const FX_PARAMS_RESET: &str = "fx-params-reset";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamsReset {
    pub file_id: FileId,
    pub slot_id: SlotId,
}

#[tauri::command]
pub async fn get_fx_params(
    state: State<'_, SharedState>,
    file_id: FileId,
    slot_id: SlotId,
) -> Result<Vec<Param>, String> {
    let state = state.inner().clone();
    blocking(move || state.fx_params(file_id, slot_id)).await?
}

#[tauri::command]
pub async fn set_fx_param(
    state: State<'_, SharedState>,
    file_id: FileId,
    slot_id: SlotId,
    param_id: u32,
    value: f64,
) -> Result<String, String> {
    let state = state.inner().clone();
    blocking(move || state.set_fx_param(file_id, slot_id, param_id, value)).await?
}

/// Show the editors of the active tab's file and hide the others (`null`: none is active).
#[tauri::command]
pub async fn set_active_file(
    editors: State<'_, SharedEditors>,
    file_id: Option<FileId>,
) -> Result<(), String> {
    let editors = editors.inner().clone();
    blocking(move || editors.set_active_file(file_id).map_err(|e| e.to_string())).await?
}
