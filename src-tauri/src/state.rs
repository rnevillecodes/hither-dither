//! Loaded-file state, independent of the Tauri runtime so it can be tested directly.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use serde::Serialize;

use crate::peaks::{PeakPyramid, PyramidBuilder};
use crate::stats::{self, Analysis, ClipRun, MAX_STORED_RUNS, Stats, StatsRunner};
use crate::wav::{SampleFormat, SampleSource, WavError, WavInfo};

/// Everything known about the currently open file.
pub struct LoadedFile {
    pub metadata: Metadata,
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

#[derive(Default)]
pub struct AppState {
    file: RwLock<Option<Arc<LoadedFile>>>,
    loading: Mutex<Option<Arc<AtomicBool>>>,
    pub stats: StatsRunner,
}

impl AppState {
    pub fn current(&self) -> Option<Arc<LoadedFile>> {
        self.file.read().unwrap().clone()
    }

    /// Parse, map and analyse `path`, then make it the current file. On failure the
    /// previous file stays loaded. A newer `open` cancels one still in progress.
    /// `progress(done, total)` is called with frames analysed so far.
    pub fn open(
        &self,
        path: &Path,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<Metadata, OpenError> {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let wav_err = |source| OpenError::Wav {
            name: name.clone(),
            source,
        };

        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(old) = self.loading.lock().unwrap().replace(cancel.clone()) {
            old.store(true, Ordering::Relaxed);
        }

        let size_bytes = std::fs::metadata(path)
            .map_err(|e| wav_err(e.into()))?
            .len();
        let source = Arc::new(SampleSource::open(path).map_err(wav_err)?);
        let total = source.frames();
        let mut builder = PyramidBuilder::new(source.channels(), total);
        let whole = stats::analyse(&source, 0, total, &cancel, Some(&mut builder), |done| {
            progress(done, total)
        })
        .map_err(|_| OpenError::Cancelled(name.clone()))?;

        let metadata = Metadata::new(path, name.clone(), size_bytes, source.info());
        let loaded = Arc::new(LoadedFile {
            metadata: metadata.clone(),
            source,
            pyramid: builder.finish(),
            whole,
        });

        // Only install the result if no newer open superseded this one meanwhile.
        let mut loading = self.loading.lock().unwrap();
        if cancel.load(Ordering::Relaxed) {
            return Err(OpenError::Cancelled(name));
        }
        *loading = None;
        self.stats.cancel();
        *self.file.write().unwrap() = Some(loaded);
        Ok(metadata)
    }

    /// Stats for `[start, end)`, or the cached whole-file stats when `range` is `None`.
    /// Returns `Ok(None)` when superseded by a newer request.
    pub fn compute_stats(&self, range: Option<(u64, u64)>) -> Result<Option<Stats>, String> {
        let file = self.current().ok_or("No file is loaded")?;
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

    pub fn clip_runs(&self) -> Result<ClipRuns, String> {
        let file = self.current().ok_or("No file is loaded")?;
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
