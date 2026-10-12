//! Tauri command handlers. Heavy work runs on blocking worker threads; see `docs/ipc.md`.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::State;
use tauri::ipc::{Channel, Response};

use crate::peaks::query_edited;
use crate::playback::{PlayRequest, PlaybackInfo, Player};
use crate::state::{
    AppState, ClipInfo, ClipRuns, ClipSettings, EditOp, EditOutcome, FadeSetting, FileId,
    LoadedFile, Metadata, Preview,
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

/// If `file_id` is playing, switch playback to its current audio.
fn follow_playing(state: &AppState, player: &Player, file_id: FileId) {
    if let Some(file) = state.get(file_id) {
        player.follow(&file.audio);
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
            Some(p) => player.follow(&state.preview(file_id, &p)?),
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
    file_id: FileId,
) -> Result<EditOutcome, String> {
    edit(state, player, file_id, move |s| s.undo(file_id)).await
}

#[tauri::command]
pub async fn redo_edit(
    state: State<'_, SharedState>,
    player: State<'_, SharedPlayer>,
    file_id: FileId,
) -> Result<EditOutcome, String> {
    edit(state, player, file_id, move |s| s.redo(file_id)).await
}

/// Write `file_id`'s audio to `path`, or over its own file when `path` is null.
#[tauri::command]
pub async fn save_file(
    state: State<'_, SharedState>,
    player: State<'_, SharedPlayer>,
    file_id: FileId,
    path: Option<PathBuf>,
    on_progress: Channel<LoadProgress>,
) -> Result<EditOutcome, String> {
    let state = state.inner().clone();
    let player = player.inner().clone();
    blocking(move || {
        // Playback holds the file's mapping, which must be released to replace it.
        stop_playing(&state, &player, file_id);
        state.save(file_id, path.as_deref(), throttled(on_progress))
    })
    .await?
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
    file_id: FileId,
) -> Result<(), String> {
    let state = state.inner().clone();
    let player = player.inner().clone();
    blocking(move || {
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
    let player = player.inner().clone();
    blocking(move || {
        if file.changed_on_disk() {
            return Err(format!(
                "“{}” changed on disk since it was opened. Reopen it to play.",
                file.metadata.name
            ));
        }
        let shared = player.play(
            file.audio.clone(),
            PlayRequest { start, end, from },
            looping,
        )?;
        std::thread::spawn(move || {
            use std::sync::atomic::Ordering;
            while !shared.stop.load(Ordering::Relaxed) {
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
