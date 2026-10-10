//! Open-file state, independent of the Tauri runtime so it can be tested directly.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use serde::Serialize;

use crate::peaks::{PeakPyramid, PyramidBuilder};
use crate::stats::{self, Analysis, ClipRun, MAX_STORED_RUNS, Stats, StatsRunner};
use crate::wav::{SampleFormat, SampleSource, WavError, WavInfo};

/// Everything known about one open file.
pub struct LoadedFile {
    pub metadata: Metadata,
    /// Size and modification time when loaded, to detect external changes.
    disk_stamp: (u64, Option<std::time::SystemTime>),
    pub source: Arc<SampleSource>,
    pub pyramid: PeakPyramid,
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

/// Identifies an open file. Assigned by the frontend so a load can be cancelled
/// before it finishes.
pub type FileId = u32;

#[derive(Default)]
pub struct AppState {
    files: RwLock<HashMap<FileId, Arc<LoadedFile>>>,
    loading: Mutex<HashMap<FileId, Arc<AtomicBool>>>,
    pub stats: StatsRunner,
}

impl AppState {
    pub fn get(&self, id: FileId) -> Option<Arc<LoadedFile>> {
        self.files.read().unwrap().get(&id).cloned()
    }

    fn require(&self, id: FileId) -> Result<Arc<LoadedFile>, String> {
        self.get(id).ok_or_else(|| "No file is loaded".to_string())
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
        self.files.write().unwrap().remove(&id)
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
            .spawn(file.source.clone(), start, end, move |r| {
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
    let source = Arc::new(SampleSource::open(path).map_err(wav_err)?);
    let total = source.frames();
    let mut builder = PyramidBuilder::new(source.channels(), total);
    let whole = stats::analyse(&source, 0, total, cancel, Some(&mut builder), |done| {
        progress(done, total)
    })
    .map_err(|_| OpenError::Cancelled(name.to_string()))?;

    Ok(Arc::new(LoadedFile {
        metadata: Metadata::new(path, name.to_string(), size_bytes, source.info()),
        disk_stamp: (size_bytes, fs_meta.modified().ok()),
        source,
        pyramid: builder.finish(),
        whole,
    }))
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
