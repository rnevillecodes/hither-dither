//! Open-file state, independent of the Tauri runtime so it can be tested directly.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use serde::{Deserialize, Serialize};

use crate::edit::{Audio, EditList};
use crate::peaks::{PeakPyramid, PyramidBuilder};
use crate::stats::{self, Analysis, ClipRun, MAX_STORED_RUNS, Stats, StatsRunner};
use crate::wav::{SampleFormat, SampleSource, WavError, WavInfo, check_size, write_wav};

/// Everything known about one open file.
pub struct LoadedFile {
    pub metadata: Metadata,
    /// Size and modification time when loaded, to detect external changes.
    disk_stamp: (u64, Option<std::time::SystemTime>),
    /// The audio as currently edited.
    pub audio: Arc<Audio>,
    pub pyramid: Arc<PeakPyramid>,
    pub whole: Analysis,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Metadata {
    pub path: PathBuf,
    pub name: String,
    pub size_bytes: u64,
    /// "PCM" or "IEEE float".
    pub encoding: &'static str,
    /// Container bits per sample.
    pub bit_depth: u16,
    pub valid_bits: u16,
    pub sample_rate: u32,
    pub channels: u16,
    pub channel_labels: Vec<String>,
    pub frames: u64,
    pub duration_secs: f64,
    /// The `data` chunk was cut short; only whole frames that exist were loaded.
    pub truncated: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("Could not open “{name}”: {source}")]
    Wav { name: String, source: WavError },
    #[error("Opening “{0}” was cancelled")]
    Cancelled(String),
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ClipRuns {
    /// Runs per channel, at most `MAX_STORED_RUNS` each.
    pub channels: Vec<Vec<ClipRun>>,
    /// Some channel had more runs than were stored.
    pub capped: bool,
}

const NO_FILE: &str = "No file is loaded";
/// Error of an edit, undo or redo stopped because its file was closed.
pub const CANCELLED: &str = "The edit was cancelled";

/// Identifies an open file. Assigned by the frontend so a load can be cancelled
/// before it finishes.
pub type FileId = u32;

/// An edit applied to a frame range of the edited audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditOp {
    Crop,
    Delete,
    Silence,
    FadeIn,
    FadeOut,
}

impl EditOp {
    fn apply(self, list: &EditList, start: u64, end: u64) -> Result<EditList, String> {
        let result = match self {
            EditOp::Crop => list.crop(start, end),
            EditOp::Delete => list.delete(start, end),
            EditOp::Silence => list.silence(start, end),
            EditOp::FadeIn => list.fade_in(start, end),
            EditOp::FadeOut => list.fade_out(start, end),
        };
        result.map_err(|e| format!("Can't apply the edit: {e}"))
    }
}

/// Undo/redo availability and whether the audio differs from the file on disk.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct EditState {
    pub can_undo: bool,
    pub can_redo: bool,
    pub dirty: bool,
    /// Increases whenever the audio changes (edit, undo, redo, save).
    pub revision: u64,
}

/// Result of an edit, undo, redo or save: the file's new metadata and edit state.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EditOutcome {
    pub metadata: Metadata,
    pub edit_state: EditState,
}

/// Every edit state of one file since it was opened or saved.
struct History {
    states: Vec<Arc<EditList>>,
    pos: usize,
    /// The state that matches the file on disk.
    saved_pos: Option<usize>,
    revision: u64,
}

impl History {
    fn new(list: Arc<EditList>) -> Self {
        Self {
            states: vec![list],
            pos: 0,
            saved_pos: Some(0),
            revision: 0,
        }
    }

    fn current(&self) -> &Arc<EditList> {
        &self.states[self.pos]
    }

    fn state(&self) -> EditState {
        EditState {
            can_undo: self.pos > 0,
            can_redo: self.pos + 1 < self.states.len(),
            dirty: self.saved_pos != Some(self.pos),
            revision: self.revision,
        }
    }
}

#[derive(Default)]
pub struct AppState {
    files: RwLock<HashMap<FileId, Arc<LoadedFile>>>,
    /// Cancel flags of loads and re-analyses in progress.
    loading: Mutex<HashMap<FileId, Arc<AtomicBool>>>,
    history: Mutex<HashMap<FileId, History>>,
    pub stats: StatsRunner,
}

impl AppState {
    pub fn get(&self, id: FileId) -> Option<Arc<LoadedFile>> {
        self.files.read().unwrap().get(&id).cloned()
    }

    fn require(&self, id: FileId) -> Result<Arc<LoadedFile>, String> {
        self.get(id).ok_or_else(|| NO_FILE.to_string())
    }

    /// Parse, map and analyse `path`, then install it under `id`. On failure nothing
    /// is installed and other files are untouched. Loads of different ids run
    /// independently; `cancel_open`/`close` (or re-opening the same id) cancel one.
    /// `progress(done, total)` is called with frames analysed so far.
    pub fn open(
        &self,
        id: FileId,
        path: &Path,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<Metadata, OpenError> {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(old) = self.loading.lock().unwrap().insert(id, cancel.clone()) {
            old.store(true, Ordering::Relaxed);
        }
        let result = load(path, &name, &cancel, &mut progress);

        // Only install the result if the load was not cancelled meanwhile.
        let mut loading = self.loading.lock().unwrap();
        if loading.get(&id).is_some_and(|c| Arc::ptr_eq(c, &cancel)) {
            loading.remove(&id);
        }
        if cancel.load(Ordering::Relaxed) {
            return Err(OpenError::Cancelled(name));
        }
        let loaded = result?;
        let metadata = loaded.metadata.clone();
        self.history
            .lock()
            .unwrap()
            .insert(id, History::new(loaded.audio.edits().clone()));
        self.files.write().unwrap().insert(id, loaded);
        Ok(metadata)
    }

    /// Cancel a load still in progress for `id`; it then fails with `Cancelled`.
    pub fn cancel_open(&self, id: FileId) {
        if let Some(c) = self.loading.lock().unwrap().remove(&id) {
            c.store(true, Ordering::Relaxed);
        }
    }

    /// Forget `id`, cancelling its load if one is running. Returns the file that was open.
    pub fn close(&self, id: FileId) -> Option<Arc<LoadedFile>> {
        self.cancel_open(id);
        self.history.lock().unwrap().remove(&id);
        self.files.write().unwrap().remove(&id)
    }

    pub fn edit_state(&self, id: FileId) -> Option<EditState> {
        self.history.lock().unwrap().get(&id).map(History::state)
    }

    /// Apply `op` to frames `[start, end)` of file `id` and re-analyse the result.
    /// `progress(done, total)` reports the re-analysis. Discards the redo history.
    pub fn apply_edit(
        &self,
        id: FileId,
        op: EditOp,
        start: u64,
        end: u64,
        progress: impl FnMut(u64, u64),
    ) -> Result<EditOutcome, String> {
        let token = self.begin_analysis(id)?;
        let result = (|| {
            let next = {
                let history = self.history.lock().unwrap();
                let h = history.get(&id).ok_or(NO_FILE)?;
                Arc::new(op.apply(h.current(), start, end)?)
            };
            let file = self.analyse_edits(id, next.clone(), &token, progress)?;
            self.commit(id, file, |h| {
                h.states.truncate(h.pos + 1);
                h.states.push(next);
                h.pos += 1;
            })
        })();
        self.end_analysis(id, &token);
        result
    }

    /// Revert the most recent edit of file `id`.
    pub fn undo(&self, id: FileId, progress: impl FnMut(u64, u64)) -> Result<EditOutcome, String> {
        self.step(id, true, progress)
    }

    /// Reapply the most recently undone edit of file `id`.
    pub fn redo(&self, id: FileId, progress: impl FnMut(u64, u64)) -> Result<EditOutcome, String> {
        self.step(id, false, progress)
    }

    fn step(
        &self,
        id: FileId,
        back: bool,
        progress: impl FnMut(u64, u64),
    ) -> Result<EditOutcome, String> {
        let token = self.begin_analysis(id)?;
        let result = (|| {
            let (target, pos) = {
                let history = self.history.lock().unwrap();
                let h = history.get(&id).ok_or(NO_FILE)?;
                let pos = match back {
                    true if h.pos > 0 => h.pos - 1,
                    false if h.pos + 1 < h.states.len() => h.pos + 1,
                    true => return Err("Nothing to undo".to_string()),
                    false => return Err("Nothing to redo".to_string()),
                };
                (h.states[pos].clone(), pos)
            };
            let file = self.analyse_edits(id, target, &token, progress)?;
            self.commit(id, file, |h| h.pos = pos)
        })();
        self.end_analysis(id, &token);
        result
    }

    /// Write file `id`'s audio to `target`, or over its own file when `None`. Afterwards the
    /// file is served from the saved copy, with an empty history; its analysis is kept since
    /// the audio is unchanged. On failure any existing file at the target is untouched and
    /// the edits are kept.
    pub fn save(&self, id: FileId, target: Option<&Path>) -> Result<EditOutcome, String> {
        let token = self.begin_analysis(id)?;
        let result = self.save_inner(id, target);
        self.end_analysis(id, &token);
        result
    }

    fn save_inner(&self, id: FileId, target: Option<&Path>) -> Result<EditOutcome, String> {
        // Range-stats workers hold the audio (and so the mapping) while they run.
        self.stats.cancel();
        let file = self.require(id)?;
        let target = target.map_or_else(|| file.metadata.path.clone(), Path::to_path_buf);
        let overwrite = same_file(&target, &file.metadata.path);
        if overwrite && file.changed_on_disk() {
            return Err(format!(
                "“{}” changed on disk since it was opened, so it wasn't overwritten. Use Save As to keep your edits in a new file.",
                file.metadata.name
            ));
        }
        let target_name = file_name(&target);
        let fail = |e: &dyn std::fmt::Display| format!("Could not save “{target_name}”: {e}");
        check_size(&file.audio).map_err(|e| fail(&e))?;

        let temp = temp_path(&target);
        if let Err(e) = write_file(&file.audio, &temp) {
            let _ = std::fs::remove_file(&temp);
            return Err(fail(&e));
        }

        // Saving doesn't change the audio, so the analysis carries over to the saved copy.
        let pyramid = file.pyramid.clone();
        let whole = file.whole.clone();
        let mut history = self.history.lock().unwrap();
        let mut files = self.files.write().unwrap();
        if overwrite {
            // Windows can't replace a file that is still memory-mapped: release ours first.
            files.remove(&id);
        }
        let old_meta = file.metadata.clone();
        let old_stamp = file.disk_stamp;
        let edits = file.audio.edits().clone();
        drop(file);

        if let Err(e) = rename_with_retry(&temp, &target) {
            let _ = std::fs::remove_file(&temp);
            if overwrite {
                // The original is unchanged: map it again with the same edits.
                match SampleSource::open(&old_meta.path) {
                    Ok(src) => {
                        let audio = Arc::new(Audio::with_edits(Arc::new(src), edits));
                        files.insert(
                            id,
                            Arc::new(LoadedFile {
                                metadata: old_meta,
                                disk_stamp: old_stamp,
                                audio,
                                pyramid,
                                whole,
                            }),
                        );
                    }
                    Err(_) => {
                        history.remove(&id);
                    }
                }
            }
            return Err(fail(&e));
        }

        let opened = std::fs::metadata(&target)
            .map_err(WavError::from)
            .and_then(|m| Ok((m, SampleSource::open(&target)?)));
        let (fs_meta, src) = match opened {
            Ok(v) => v,
            Err(e) => {
                files.remove(&id);
                history.remove(&id);
                return Err(format!(
                    "Saved “{target_name}”, but could not reopen it: {e}"
                ));
            }
        };
        let audio = Arc::new(Audio::new(Arc::new(src)));
        let size = fs_meta.len();
        let metadata = Metadata::new(&target, target_name, size, audio.info());
        let revision = history.get(&id).map_or(0, |h| h.revision) + 1;
        let mut fresh = History::new(audio.edits().clone());
        fresh.revision = revision;
        let edit_state = fresh.state();
        history.insert(id, fresh);
        files.insert(
            id,
            Arc::new(LoadedFile {
                metadata: metadata.clone(),
                disk_stamp: (size, fs_meta.modified().ok()),
                audio,
                pyramid,
                whole,
            }),
        );
        Ok(EditOutcome {
            metadata,
            edit_state,
        })
    }

    /// Register a cancellable re-analysis of `id`; fails while one is already running.
    fn begin_analysis(&self, id: FileId) -> Result<Arc<AtomicBool>, String> {
        let mut loading = self.loading.lock().unwrap();
        if loading.contains_key(&id) {
            return Err("The file is still being analysed".to_string());
        }
        let token = Arc::new(AtomicBool::new(false));
        loading.insert(id, token.clone());
        Ok(token)
    }

    fn end_analysis(&self, id: FileId, token: &Arc<AtomicBool>) {
        let mut loading = self.loading.lock().unwrap();
        if loading.get(&id).is_some_and(|t| Arc::ptr_eq(t, token)) {
            loading.remove(&id);
        }
    }

    /// Analyse file `id`'s source as seen through `edits` (stops early if cancelled).
    fn analyse_edits(
        &self,
        id: FileId,
        edits: Arc<EditList>,
        cancel: &AtomicBool,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<Arc<LoadedFile>, String> {
        let file = self.require(id)?;
        let audio = Arc::new(Audio::with_edits(file.audio.source().clone(), edits));
        let total = audio.frames();
        let mut builder = PyramidBuilder::new(audio.channels(), total);
        let whole = stats::analyse(&audio, 0, total, cancel, Some(&mut builder), |done| {
            progress(done, total)
        })
        .map_err(|_| CANCELLED.to_string())?;
        let mut metadata = file.metadata.clone();
        metadata.frames = total;
        metadata.duration_secs = audio.info().duration_secs();
        Ok(Arc::new(LoadedFile {
            metadata,
            disk_stamp: file.disk_stamp,
            audio,
            pyramid: Arc::new(builder.finish()),
            whole,
        }))
    }

    /// Install `file` as the new state of `id` after `change`s its history.
    fn commit(
        &self,
        id: FileId,
        file: Arc<LoadedFile>,
        change: impl FnOnce(&mut History),
    ) -> Result<EditOutcome, String> {
        let mut history = self.history.lock().unwrap();
        // Closed meanwhile (closing also cancels, but the analysis may have just finished).
        let h = history.get_mut(&id).ok_or(CANCELLED)?;
        let mut files = self.files.write().unwrap();
        if !files.contains_key(&id) {
            return Err(CANCELLED.to_string());
        }
        change(h);
        h.revision += 1;
        let metadata = file.metadata.clone();
        files.insert(id, file);
        Ok(EditOutcome {
            metadata,
            edit_state: h.state(),
        })
    }

    /// Stats for `[start, end)` of file `id`, or its cached whole-file stats when
    /// `range` is `None`. Returns `Ok(None)` when superseded by a newer request.
    pub fn compute_stats(
        &self,
        id: FileId,
        range: Option<(u64, u64)>,
    ) -> Result<Option<Stats>, String> {
        let file = self.require(id)?;
        let Some((start, end)) = range else {
            self.stats.cancel();
            return Ok(Some(file.whole.stats.clone()));
        };
        let (tx, rx) = std::sync::mpsc::channel();
        self.stats
            .spawn(file.audio.clone(), start, end, move |r| {
                let _ = tx.send(r);
            })
            .join()
            .map_err(|_| "Statistics worker failed".to_string())?;
        Ok(rx.recv().ok().flatten().map(|a| a.stats))
    }

    pub fn clip_runs(&self, id: FileId) -> Result<ClipRuns, String> {
        let file = self.require(id)?;
        let capped = file
            .whole
            .stats
            .channels
            .iter()
            .any(|c| c.clip_runs > MAX_STORED_RUNS as u64);
        Ok(ClipRuns {
            channels: file.whole.clip_runs.clone(),
            capped,
        })
    }
}

/// Parse, map and analyse `path` (stops early if `cancel` is set).
fn load(
    path: &Path,
    name: &str,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(u64, u64),
) -> Result<Arc<LoadedFile>, OpenError> {
    let wav_err = |source| OpenError::Wav {
        name: name.to_string(),
        source,
    };
    let fs_meta = std::fs::metadata(path).map_err(|e| wav_err(e.into()))?;
    let size_bytes = fs_meta.len();
    let audio = Arc::new(Audio::new(Arc::new(
        SampleSource::open(path).map_err(wav_err)?,
    )));
    let total = audio.frames();
    let mut builder = PyramidBuilder::new(audio.channels(), total);
    let whole = stats::analyse(&audio, 0, total, cancel, Some(&mut builder), |done| {
        progress(done, total)
    })
    .map_err(|_| OpenError::Cancelled(name.to_string()))?;

    Ok(Arc::new(LoadedFile {
        metadata: Metadata::new(path, name.to_string(), size_bytes, audio.info()),
        disk_stamp: (size_bytes, fs_meta.modified().ok()),
        audio,
        pyramid: Arc::new(builder.finish()),
        whole,
    }))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Whether two paths name the same existing file.
fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// A unique hidden file next to `target`, so the final rename stays on one volume.
fn temp_path(target: &Path) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    target.with_file_name(format!(
        ".{}.{}-{nanos}.tmp",
        file_name(target),
        std::process::id()
    ))
}

/// Write `audio` to a new file at `path` and flush it to disk.
fn write_file(audio: &Audio, path: &Path) -> Result<(), crate::wav::WriteError> {
    let file = std::fs::File::create_new(path)?;
    write_wav(audio, &file)?;
    file.sync_all()?;
    Ok(())
}

/// Move `from` over `to`, retrying briefly: on Windows a reader finishing with the old
/// file (or a virus scanner) can hold it open for a moment.
fn rename_with_retry(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut attempt = 0;
    loop {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(_) if attempt < 20 => {
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => return Err(e),
        }
    }
}

impl LoadedFile {
    /// The file's size or modification time differs from when it was loaded. Reading a
    /// memory-mapped file that was changed or truncated is unsafe, so playback refuses.
    pub fn changed_on_disk(&self) -> bool {
        match std::fs::metadata(&self.metadata.path) {
            Ok(m) => (m.len(), m.modified().ok()) != self.disk_stamp,
            Err(_) => true,
        }
    }
}

impl Metadata {
    fn new(path: &Path, name: String, size_bytes: u64, info: &WavInfo) -> Self {
        Self {
            path: path.to_path_buf(),
            name,
            size_bytes,
            encoding: if info.format.is_float() {
                "IEEE float"
            } else {
                "PCM"
            },
            bit_depth: (info.format.bytes() * 8) as u16,
            valid_bits: if info.format == SampleFormat::U8 {
                8
            } else {
                info.valid_bits
            },
            sample_rate: info.sample_rate,
            channels: info.channels,
            channel_labels: channel_labels(info.channels, info.channel_mask),
            frames: info.frames,
            duration_secs: info.duration_secs(),
            truncated: info.truncated,
        }
    }
}

/// Speaker names from the `WAVE_FORMAT_EXTENSIBLE` channel mask, else positional names.
pub fn channel_labels(channels: u16, mask: Option<u32>) -> Vec<String> {
    const SPEAKERS: [&str; 18] = [
        "L", "R", "C", "LFE", "Ls", "Rs", "Lc", "Rc", "Cs", "Lsd", "Rsd", "Tc", "Tfl", "Tfc",
        "Tfr", "Tbl", "Tbc", "Tbr",
    ];
    let positional = || match channels {
        1 => vec!["Mono".to_string()],
        2 => vec!["L".to_string(), "R".to_string()],
        n => (1..=n).map(|i| format!("Ch {i}")).collect(),
    };
    let Some(mask) = mask.filter(|&m| m != 0) else {
        return positional();
    };
    let mut names: Vec<String> = (0..SPEAKERS.len())
        .filter(|bit| mask & (1 << bit) != 0)
        .map(|bit| SPEAKERS[bit].to_string())
        .collect();
    if channels == 1 && names == ["C"] {
        return positional();
    }
    names.truncate(channels as usize);
    // Channels beyond the mask's speakers are unassigned.
    for i in names.len()..channels as usize {
        names.push(format!("Ch {}", i + 1));
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_follow_mask_or_position() {
        assert_eq!(channel_labels(1, None), ["Mono"]);
        assert_eq!(channel_labels(2, Some(0x3)), ["L", "R"]);
        assert_eq!(
            channel_labels(6, Some(0x3F)),
            ["L", "R", "C", "LFE", "Ls", "Rs"]
        );
        assert_eq!(channel_labels(3, None), ["Ch 1", "Ch 2", "Ch 3"]);
        assert_eq!(channel_labels(4, Some(0x3)), ["L", "R", "Ch 3", "Ch 4"]);
    }
}
