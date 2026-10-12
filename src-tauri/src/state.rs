//! Open-file state, independent of the Tauri runtime so it can be tested directly.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::edit::{Audio, ClipFade, Crossfade, Curve, EditList};
use crate::fx;
use crate::fx::chain::{Chain, ChainInfo, LIVE_BLOCK, MAX_EFFECTS, Slot, SlotId, layout_name};
use crate::fx::live::LiveChain;
use crate::fx::render::Render;
use crate::peaks::{PeakPyramid, PyramidBuilder};
use crate::plugins::cache::{LoadStep, PluginCache};
use crate::plugins::host::HandlerEvent;
use crate::plugins::instance::{Config, Instance};
use crate::plugins::main_thread::{self, Inline, UiThread};
use crate::plugins::{ClassId, module};
use crate::stats::{self, Analysis, ClipRun, MAX_STORED_RUNS, Stats, StatsRunner, map_runs};
use crate::wav::{
    NativeFrames, SampleFormat, SampleSource, WavError, WavInfo, check_size,
    write_wav_with_progress,
};

/// Everything known about one open file.
pub struct LoadedFile {
    pub metadata: Metadata,
    /// Size and modification time when loaded, to detect external changes.
    disk_stamp: (u64, Option<std::time::SystemTime>),
    /// The audio as currently edited.
    pub audio: Arc<Audio>,
    /// History revision that `audio` belongs to.
    pub revision: u64,
    /// Peaks of the unedited source (as opened or saved); edited peaks are drawn through
    /// it until the edited audio has been analysed.
    pub source_pyramid: Arc<PeakPyramid>,
    /// Whole-file analysis of the unedited source, including its clip runs.
    pub source_whole: Arc<Analysis>,
    /// Analysis of `audio`, once the background pass has finished.
    pub analysis: Option<EditAnalysis>,
    /// The effects the audio is heard and saved through. Changing them doesn't change
    /// `audio` or `revision`.
    pub chain: Arc<Chain>,
}

/// Peaks and whole-file analysis of one edit state.
#[derive(Clone)]
pub struct EditAnalysis {
    pub revision: u64,
    pub pyramid: Arc<PeakPyramid>,
    pub whole: Arc<Analysis>,
}

/// Wait after the last change before analysing, so a burst of edits is analysed once.
const ANALYSIS_DEBOUNCE: Duration = Duration::from_millis(300);

/// A pending or running background analysis.
struct Job {
    cancel: Arc<AtomicBool>,
    thread: std::thread::JoinHandle<()>,
}

/// Wakes `wait_analysis` callers when a file's analysis or revision changes.
#[derive(Default)]
struct Signal {
    lock: Mutex<()>,
    changed: Condvar,
    /// Background analyses that ran to completion (for tests).
    completed: AtomicU64,
}

impl Signal {
    fn notify(&self) {
        let _guard = self.lock.lock().unwrap();
        self.changed.notify_all();
    }
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
    /// The edited audio hasn't been analysed yet: these are the source's runs that
    /// survive the edits unchanged, and runs created by a hard join are missing.
    pub pending: bool,
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

/// Length of the crossfade Delete puts on a new join.
const DEFAULT_CROSSFADE_SECS: f64 = 0.010;

impl EditOp {
    /// `bend` shapes fades; `sample_rate` sizes Delete's default crossfade.
    fn apply(
        self,
        list: &EditList,
        start: u64,
        end: u64,
        bend: f64,
        sample_rate: u32,
    ) -> Result<EditList, String> {
        let curve = Curve::new(bend);
        let result = match self {
            EditOp::Crop => list.crop(start, end),
            EditOp::Delete => {
                let xfade = (sample_rate as f64 * DEFAULT_CROSSFADE_SECS).round() as u64;
                list.delete(start, end, xfade)
            }
            EditOp::Silence => list.silence(start, end),
            EditOp::FadeIn => list.fade_in_range(start, end, curve),
            EditOp::FadeOut => list.fade_out_range(start, end, curve),
        };
        result.map_err(|e| format!("Can't apply the edit: {e}"))
    }
}

/// A fade's length (frames) and bend, as sent to and from the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct FadeSetting {
    pub len: u64,
    pub bend: f64,
}

impl From<ClipFade> for FadeSetting {
    fn from(f: ClipFade) -> Self {
        Self {
            len: f.len,
            bend: f.curve.bend,
        }
    }
}

impl From<FadeSetting> for ClipFade {
    fn from(f: FadeSetting) -> Self {
        Self {
            len: f.len,
            curve: Curve::new(f.bend),
        }
    }
}

/// The trim window and end fades, as set by the handles. Trim positions are frames of
/// the untrimmed timeline (see `ClipInfo::untrimmed_frames`).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipSettings {
    pub trim_start: u64,
    pub trim_end: u64,
    pub fade_in: FadeSetting,
    pub fade_out: FadeSetting,
}

/// A join's crossfade as the handles need it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct JoinState {
    /// Identifies the join to `set_crossfade`.
    pub index: usize,
    /// Output frame of the join (may be outside the audio when trimmed away).
    pub position: i64,
    pub len: u64,
    pub bend: f64,
    pub max_len: u64,
}

/// Everything the clip handles show: trim window, end fades and joins.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClipInfo {
    pub untrimmed_frames: u64,
    pub trim_start: u64,
    pub trim_end: u64,
    pub fade_in: FadeSetting,
    pub fade_out: FadeSetting,
    pub joins: Vec<JoinState>,
}

impl ClipInfo {
    fn of(list: &EditList) -> Self {
        let trim = list.trim();
        Self {
            untrimmed_frames: list.untrimmed_frames(),
            trim_start: trim.start,
            trim_end: trim.end,
            fade_in: list.fade_in().into(),
            fade_out: list.fade_out().into(),
            joins: list
                .joins()
                .into_iter()
                .map(|j| JoinState {
                    index: j.index,
                    position: j.position,
                    len: j.xfade.len,
                    bend: j.xfade.curve.bend,
                    max_len: j.max_len,
                })
                .collect(),
        }
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

/// Result of an edit, chain change, undo, redo or save: the file's new metadata, edit state
/// and chain.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EditOutcome {
    pub metadata: Metadata,
    pub edit_state: EditState,
    pub clip: ClipInfo,
    pub chain: ChainInfo,
    /// The edited audio changed (and with it `edit_state.revision`). False for chain changes,
    /// which leave the peaks and analysis as they are.
    #[serde(rename = "audioChanged")]
    pub audio_changed: bool,
    /// Processed samples a save had to clamp to fit the file's integer format. Zero for
    /// everything else.
    #[serde(rename = "clampedSamples")]
    pub clamped_samples: u64,
}

/// One step of a file's history: its edits and the effects in its chain, in order.
#[derive(Clone)]
struct Snapshot {
    edits: Arc<EditList>,
    chain: Vec<SlotId>,
}

/// Every edit and chain state of one file since it was opened or saved.
struct History {
    states: Vec<Snapshot>,
    pos: usize,
    revision: u64,
    /// Every slot the states refer to, so undo can bring an effect back as it was.
    slots: HashMap<SlotId, Arc<Slot>>,
    /// Bypass All, which isn't part of the history.
    bypass_all: bool,
}

impl History {
    fn new(edits: Arc<EditList>) -> Self {
        Self {
            states: vec![Snapshot {
                edits,
                chain: Vec::new(),
            }],
            pos: 0,
            revision: 0,
            slots: HashMap::new(),
            bypass_all: false,
        }
    }

    fn current(&self) -> &Snapshot {
        &self.states[self.pos]
    }

    /// The current state's chain.
    fn chain(&self) -> Chain {
        let slots = self.current().chain.iter();
        let slots = slots.filter_map(|id| self.slots.get(id).cloned());
        Chain::new(slots.collect(), self.bypass_all)
    }

    /// Unsaved: the audio is edited, or some effect changes it.
    fn state(&self) -> EditState {
        EditState {
            can_undo: self.pos > 0,
            can_redo: self.pos + 1 < self.states.len(),
            dirty: !self.current().edits.is_identity() || self.chain().is_active(),
            revision: self.revision,
        }
    }

    /// Adds `next` after the current state, discarding the redo history.
    fn push(&mut self, next: Snapshot) {
        self.states.truncate(self.pos + 1);
        self.states.push(next);
        self.pos += 1;
        self.release_unreferenced();
    }

    /// Forgets slots no state refers to any more. The last reference to a slot releases its
    /// plugin on the UI thread.
    fn release_unreferenced(&mut self) {
        let used: HashSet<SlotId> = self.states.iter().flat_map(|s| &s.chain).copied().collect();
        self.slots.retain(|id, _| used.contains(id));
    }
}

const NOT_IN_CHAIN: &str = "That effect isn't in the chain";

/// Called on the UI thread when a file's effects change how they process by themselves.
pub type FxListener = Arc<dyn Fn(FileId) + Send + Sync>;

pub struct AppState {
    files: Arc<RwLock<HashMap<FileId, Arc<LoadedFile>>>>,
    /// Cancel flags of loads and saves in progress.
    loading: Mutex<HashMap<FileId, Arc<AtomicBool>>>,
    history: Mutex<HashMap<FileId, History>>,
    /// Background analysis of each file's current edit state.
    analysers: Mutex<HashMap<FileId, Job>>,
    signal: Arc<Signal>,
    pub stats: StatsRunner,
    /// Where plugin calls that must run on the UI thread go.
    ui: Arc<dyn UiThread>,
    next_slot: AtomicU32,
    fx_listener: Arc<Mutex<Option<FxListener>>>,
}

impl Default for AppState {
    /// Runs plugin calls on the calling thread, for tests.
    fn default() -> Self {
        AppState::new(Arc::new(Inline))
    }
}

impl AppState {
    pub fn new(ui: Arc<dyn UiThread>) -> Self {
        AppState {
            files: Arc::default(),
            loading: Mutex::default(),
            history: Mutex::default(),
            analysers: Mutex::default(),
            signal: Arc::default(),
            stats: StatsRunner::default(),
            ui,
            next_slot: AtomicU32::new(1),
            fx_listener: Arc::default(),
        }
    }

    /// Sets what to call when a file's effects change how they process by themselves (a
    /// plugin reports a new latency), so playback can follow.
    pub fn on_fx_changed(&self, listener: impl Fn(FileId) + Send + Sync + 'static) {
        *self.fx_listener.lock().unwrap() = Some(Arc::new(listener));
    }

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
        self.stop_analysis(id);
        self.history
            .lock()
            .unwrap()
            .insert(id, History::new(loaded.audio.edits().clone()));
        self.files.write().unwrap().insert(id, loaded);
        self.signal.notify();
        Ok(metadata)
    }

    /// Cancel a load still in progress for `id`; it then fails with `Cancelled`.
    pub fn cancel_open(&self, id: FileId) {
        if let Some(c) = self.loading.lock().unwrap().remove(&id) {
            c.store(true, Ordering::Relaxed);
        }
    }

    /// Forget `id`, cancelling its load, save or analysis if one is running. Returns the
    /// file that was open.
    pub fn close(&self, id: FileId) -> Option<Arc<LoadedFile>> {
        self.cancel_open(id);
        self.stop_analysis(id);
        self.history.lock().unwrap().remove(&id);
        let file = self.files.write().unwrap().remove(&id);
        self.signal.notify();
        file
    }

    pub fn edit_state(&self, id: FileId) -> Option<EditState> {
        self.history.lock().unwrap().get(&id).map(History::state)
    }

    /// The clip handles' state for file `id`.
    pub fn clip(&self, id: FileId) -> Option<ClipInfo> {
        self.history
            .lock()
            .unwrap()
            .get(&id)
            .map(|h| ClipInfo::of(&h.current().edits))
    }

    /// Apply `op` to frames `[start, end)` of file `id`. `bend` shapes fades. Discards
    /// the redo history. The result is analysed in the background.
    pub fn apply_edit(
        &self,
        id: FileId,
        op: EditOp,
        start: u64,
        end: u64,
        bend: f64,
    ) -> Result<EditOutcome, String> {
        let rate = self.require(id)?.metadata.sample_rate;
        self.change(id, |list| op.apply(list, start, end, bend, rate))
    }

    /// Replace file `id`'s trim window and end fades (clamped to what the audio allows).
    pub fn set_clip(&self, id: FileId, clip: ClipSettings) -> Result<EditOutcome, String> {
        self.change(id, |list| Ok(clip.apply(list)))
    }

    /// Replace the crossfade at join `join` of file `id` (length clamped).
    pub fn set_crossfade(
        &self,
        id: FileId,
        join: usize,
        xfade: FadeSetting,
    ) -> Result<EditOutcome, String> {
        self.change(id, |list| apply_crossfade(list, join, xfade))
    }

    /// File `id`'s current edits with `preview` applied, as the committing command would
    /// apply it, without changing the file or its history.
    pub fn preview(&self, id: FileId, preview: &Preview) -> Result<Arc<Audio>, String> {
        let file = self.require(id)?;
        let list = file.audio.edits();
        let edited = match *preview {
            Preview::Clip { clip } => clip.apply(list),
            Preview::Crossfade { join, xfade } => apply_crossfade(list, join, xfade)?,
            Preview::Fade {
                op,
                start,
                end,
                bend,
            } => match op {
                EditOp::FadeIn | EditOp::FadeOut => {
                    op.apply(list, start, end, bend, file.metadata.sample_rate)?
                }
                _ => return Err("Only fades can be previewed".to_string()),
            },
        };
        Ok(Arc::new(Audio::with_edits(
            file.audio.source().clone(),
            Arc::new(edited),
        )))
    }

    /// Derive a new edit state from the current one and add it to the history
    /// (discarding redo). A change that leaves the audio as it is adds nothing.
    fn change(
        &self,
        id: FileId,
        edit: impl FnOnce(&EditList) -> Result<EditList, String>,
    ) -> Result<EditOutcome, String> {
        self.check_not_saving(id)?;
        let mut history = self.history.lock().unwrap();
        let h = history.get_mut(&id).ok_or(NO_FILE)?;
        let file = self.require(id)?;
        let next = edit(&h.current().edits)?;
        if next == *h.current().edits {
            return Ok(Self::outcome(&file.metadata, h, false));
        }
        let chain = h.current().chain.clone();
        h.push(Snapshot {
            edits: Arc::new(next),
            chain,
        });
        Ok(self.install(id, &file, h))
    }

    /// Revert the most recent edit of file `id`.
    pub fn undo(&self, id: FileId) -> Result<EditOutcome, String> {
        self.step(id, true)
    }

    /// Reapply the most recently undone edit of file `id`.
    pub fn redo(&self, id: FileId) -> Result<EditOutcome, String> {
        self.step(id, false)
    }

    fn step(&self, id: FileId, back: bool) -> Result<EditOutcome, String> {
        self.check_not_saving(id)?;
        let mut history = self.history.lock().unwrap();
        let h = history.get_mut(&id).ok_or(NO_FILE)?;
        let file = self.require(id)?;
        h.pos = match back {
            true if h.pos > 0 => h.pos - 1,
            false if h.pos + 1 < h.states.len() => h.pos + 1,
            true => return Err("Nothing to undo".to_string()),
            false => return Err("Nothing to redo".to_string()),
        };
        let outcome = self.install(id, &file, h);
        drop(history);
        self.sync_bypass(id);
        Ok(outcome)
    }

    /// Serve file `id` as the current state of `h`. If the edits changed, that is the next
    /// revision and its analysis (re)starts; a change to the chain alone keeps the revision
    /// and analysis. Called with the history lock held, so changes to one file are serialized.
    fn install(&self, id: FileId, file: &LoadedFile, h: &mut History) -> EditOutcome {
        let edits = h.current().edits.clone();
        let chain = Arc::new(h.chain());
        if Arc::ptr_eq(&edits, file.audio.edits()) {
            let mut files = self.files.write().unwrap();
            // Re-read under the lock: the analysis may have finished meanwhile.
            if let Some(next) = files.get(&id).map(|f| Arc::new(f.with_chain(chain))) {
                files.insert(id, next);
            }
            drop(files);
            self.signal.notify();
            return Self::outcome(&file.metadata, h, false);
        }
        h.revision += 1;
        let audio = Arc::new(Audio::with_edits(file.audio.source().clone(), edits));
        let mut metadata = file.metadata.clone();
        metadata.frames = audio.frames();
        metadata.duration_secs = audio.info().duration_secs();
        let next = Arc::new(
            LoadedFile {
                metadata: metadata.clone(),
                disk_stamp: file.disk_stamp,
                audio,
                revision: h.revision,
                source_pyramid: file.source_pyramid.clone(),
                source_whole: file.source_whole.clone(),
                analysis: None,
                chain,
            }
            .seeded(),
        );
        self.files.write().unwrap().insert(id, next.clone());
        self.restart_analysis(id, &next);
        self.signal.notify();
        Self::outcome(&metadata, h, true)
    }

    fn outcome(metadata: &Metadata, h: &History, audio_changed: bool) -> EditOutcome {
        EditOutcome {
            metadata: metadata.clone(),
            edit_state: h.state(),
            clip: ClipInfo::of(&h.current().edits),
            chain: h.chain().info(),
            audio_changed,
            clamped_samples: 0,
        }
    }

    /// Sends file `id`'s bypass states to plugins that bypass themselves, on the UI thread.
    /// Call without the history lock, which the UI thread may be waiting for.
    fn sync_bypass(&self, id: FileId) {
        let Some(chain) = self.get(id).map(|f| f.chain.clone()) else {
            return;
        };
        if !chain.is_empty() {
            let _ = main_thread::run(&*self.ui, move || chain.apply_bypass());
        }
    }

    /// File `id`'s effect chain.
    pub fn get_chain(&self, id: FileId) -> Option<ChainInfo> {
        let history = self.history.lock().unwrap();
        history.get(&id).map(|h| h.chain().info())
    }

    /// An effect of file `id`, in its chain or kept for undo.
    pub fn slot(&self, id: FileId, slot: SlotId) -> Option<Arc<Slot>> {
        let history = self.history.lock().unwrap();
        history.get(&id)?.slots.get(&slot).cloned()
    }

    /// What playback of file `id` runs through: the slots of its chain that process audio.
    pub fn live_chain(&self, id: FileId) -> Option<Arc<LiveChain>> {
        LiveChain::of(&self.get(id)?.chain)
    }

    /// [`AppState::live_chain`], with the slots' tails and delay lines cleared on the UI
    /// thread, so audio from an earlier play doesn't leak into a new one.
    pub fn start_live_chain(&self, id: FileId) -> Option<Arc<LiveChain>> {
        let chain = self.get(id)?.chain.clone();
        let live = LiveChain::of(&chain)?;
        let _ = main_thread::run(&*self.ui, move || {
            for s in chain.processed() {
                s.instance().reset();
            }
        });
        Some(live)
    }

    /// When the plugin in `slot` (of file `id`) reports a new latency, reactivate it so the
    /// new latency applies, record it, and tell the listener.
    fn watch_latency(&self, id: FileId, slot: &Arc<Slot>) {
        use vst3::Steinberg::Vst::RestartFlags_::kLatencyChanged;
        let weak = Arc::downgrade(slot);
        let (ui, listener) = (self.ui.clone(), self.fx_listener.clone());
        let on_event = move |event| {
            if !matches!(event, HandlerEvent::Restart(flags) if flags & kLatencyChanged != 0) {
                return;
            }
            let (weak, listener) = (weak.clone(), listener.clone());
            let _ = ui.post(Box::new(move || {
                let Some(slot) = weak.upgrade() else { return };
                slot.instance().reset();
                slot.set_latency(slot.instance().latency());
                let listener = listener.lock().unwrap_or_else(|e| e.into_inner()).clone();
                if let Some(f) = listener {
                    f(id);
                }
            }));
        };
        let handler = slot.instance().handler();
        handler.set_listener(Some(Box::new(on_event)));
    }

    /// Load effect `class_id` from `plugins` and add it to the end of file `id`'s chain, as
    /// one undo step. Fails, leaving the chain as it was, when the chain is full, the effect
    /// can't process the file's channels, or the plugin fails to load.
    pub fn add_effect(
        &self,
        id: FileId,
        plugins: &Arc<PluginCache>,
        class_id: ClassId,
    ) -> Result<EditOutcome, String> {
        const FULL: &str = "The effect chain is full";
        self.check_not_saving(id)?;
        let file = self.require(id)?;
        let full = |h: &History| h.current().chain.len() >= MAX_EFFECTS;
        if self.with_history(id, full)? {
            return Err(FULL.to_string());
        }
        let (bundle, class) = plugins
            .find_class(class_id)
            .filter(|(_, c)| c.error.is_none())
            .ok_or("That effect isn't available. Rescan the plugins and try again.")?;
        if plugins.settings().disabled.contains(&class_id) {
            return Err(format!(
                "{} is disabled. You can enable it in Manage Plugins.",
                class.name
            ));
        }
        let info = file.audio.info();
        let (channels, mask) = (info.channels, info.channel_mask);
        if !class.supports(channels, mask) {
            return Err(format!(
                "{} doesn't support {} audio",
                class.name,
                layout_name(channels, mask)
            ));
        }
        let config = Config {
            channels,
            channel_mask: mask,
            sample_rate: info.sample_rate as f64,
            max_block: LIVE_BLOCK,
            offline: false,
        };
        let source = file.audio.source().clone();
        drop(file);

        let slot_id = self.next_slot.fetch_add(1, Ordering::Relaxed);
        let (cache, ui, name) = (plugins.clone(), self.ui.clone(), class.name.clone());
        let slot = main_thread::run(&*self.ui, move || {
            let _loading = cache.loading(class_id, LoadStep::Create);
            let module = module::load(&bundle).map_err(|e| e.to_string())?;
            let instance = Instance::create(module, class_id).map_err(|e| e.to_string())?;
            let latency = instance.configure(config).map_err(|e| e.to_string())?;
            Ok::<_, String>(Slot::new(slot_id, &class, instance, latency, ui))
        })
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("“{name}” could not be loaded: {e}"))?;
        let slot = Arc::new(slot);
        self.watch_latency(id, &slot);

        self.change_chain(id, |h, file| {
            // The file was reopened while the plugin loaded.
            if !Arc::ptr_eq(file.audio.source(), &source) {
                return Err(CANCELLED.to_string());
            }
            if full(h) {
                return Err(FULL.to_string());
            }
            h.slots.insert(slot_id, slot);
            let mut chain = h.current().chain.clone();
            chain.push(slot_id);
            Ok(chain)
        })
    }

    /// Remove an effect from file `id`'s chain, as one undo step.
    pub fn remove_effect(&self, id: FileId, slot: SlotId) -> Result<EditOutcome, String> {
        self.change_chain(id, |h, _| {
            let mut chain = h.current().chain.clone();
            let index = chain.iter().position(|&s| s == slot).ok_or(NOT_IN_CHAIN)?;
            chain.remove(index);
            Ok(chain)
        })
    }

    /// Move an effect of file `id` to position `index` of its chain (clamped), as one undo
    /// step. Moving it to where it already is adds nothing.
    pub fn move_effect(
        &self,
        id: FileId,
        slot: SlotId,
        index: usize,
    ) -> Result<EditOutcome, String> {
        self.change_chain(id, |h, _| {
            let mut chain = h.current().chain.clone();
            let from = chain.iter().position(|&s| s == slot).ok_or(NOT_IN_CHAIN)?;
            chain.remove(from);
            chain.insert(index.min(chain.len()), slot);
            Ok(chain)
        })
    }

    /// Bypass one effect of file `id`, or stop bypassing it. Not an undo step.
    pub fn set_effect_bypass(
        &self,
        id: FileId,
        slot: SlotId,
        on: bool,
    ) -> Result<EditOutcome, String> {
        self.change_bypass(id, |h| {
            let in_chain = h.current().chain.contains(&slot);
            let s = h
                .slots
                .get(&slot)
                .filter(|_| in_chain)
                .ok_or(NOT_IN_CHAIN)?;
            s.set_bypassed(on);
            Ok(())
        })
    }

    /// Bypass file `id`'s whole chain, or stop bypassing it. Not an undo step.
    pub fn set_bypass_all(&self, id: FileId, on: bool) -> Result<EditOutcome, String> {
        self.change_bypass(id, |h| {
            h.bypass_all = on;
            Ok(())
        })
    }

    fn with_history<T>(&self, id: FileId, f: impl FnOnce(&History) -> T) -> Result<T, String> {
        let history = self.history.lock().unwrap();
        Ok(f(history.get(&id).ok_or(NO_FILE)?))
    }

    /// Derive file `id`'s next chain from its current state and add it to the history
    /// (discarding redo). A chain that stays the same adds nothing.
    fn change_chain(
        &self,
        id: FileId,
        edit: impl FnOnce(&mut History, &LoadedFile) -> Result<Vec<SlotId>, String>,
    ) -> Result<EditOutcome, String> {
        self.check_not_saving(id)?;
        let mut history = self.history.lock().unwrap();
        let h = history.get_mut(&id).ok_or(NO_FILE)?;
        let file = self.require(id)?;
        let next = edit(h, &file)?;
        let outcome = if next == h.current().chain {
            Self::outcome(&file.metadata, h, false)
        } else {
            let edits = h.current().edits.clone();
            h.push(Snapshot { edits, chain: next });
            self.install(id, &file, h)
        };
        drop(history);
        self.sync_bypass(id);
        Ok(outcome)
    }

    /// Change file `id`'s bypass settings, which are outside the history.
    fn change_bypass(
        &self,
        id: FileId,
        edit: impl FnOnce(&mut History) -> Result<(), String>,
    ) -> Result<EditOutcome, String> {
        self.check_not_saving(id)?;
        let mut history = self.history.lock().unwrap();
        let h = history.get_mut(&id).ok_or(NO_FILE)?;
        let file = self.require(id)?;
        edit(h)?;
        let outcome = self.install(id, &file, h);
        drop(history);
        self.sync_bypass(id);
        Ok(outcome)
    }

    fn check_not_saving(&self, id: FileId) -> Result<(), String> {
        if self.loading.lock().unwrap().contains_key(&id) {
            return Err("The file is being saved".to_string());
        }
        Ok(())
    }

    /// Cancel any analysis of `id` and schedule one of `file` unless it is already
    /// analysed. The pass starts after a short wait, so rapid changes are analysed once.
    fn restart_analysis(&self, id: FileId, file: &Arc<LoadedFile>) {
        let mut analysers = self.analysers.lock().unwrap();
        if let Some(old) = analysers.remove(&id) {
            old.cancel.store(true, Ordering::Relaxed);
        }
        if file.analysis.is_some() {
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let (files, signal) = (self.files.clone(), self.signal.clone());
        let (audio, revision) = (file.audio.clone(), file.revision);
        let flag = cancel.clone();
        let thread = std::thread::spawn(move || {
            let start = Instant::now() + ANALYSIS_DEBOUNCE;
            while Instant::now() < start {
                if flag.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let Some(analysis) = analyse_audio(&audio, revision, &flag, |_| {}) else {
                return;
            };
            drop(audio);
            let mut files = files.write().unwrap();
            if let Some(f) = files
                .get(&id)
                .filter(|f| f.revision == revision && f.analysis.is_none())
            {
                let f = Arc::new(f.with_analysis(analysis));
                files.insert(id, f);
                signal.completed.fetch_add(1, Ordering::Relaxed);
            }
            drop(files);
            signal.notify();
        });
        analysers.insert(id, Job { cancel, thread });
    }

    /// Cancel any background analysis of `id` and wait for it to let go of the audio.
    fn stop_analysis(&self, id: FileId) {
        let job = self.analysers.lock().unwrap().remove(&id);
        if let Some(job) = job {
            job.cancel.store(true, Ordering::Relaxed);
            let _ = job.thread.join();
        }
    }

    /// Wait until revision `revision` of file `id` has been analysed: `true` once it has,
    /// `false` if a newer revision replaced it or the file was closed first.
    pub fn wait_analysis(&self, id: FileId, revision: u64) -> bool {
        let mut guard = self.signal.lock.lock().unwrap();
        loop {
            match self.get(id) {
                Some(f) if f.revision == revision && f.analysis.is_some() => return true,
                Some(f) if f.revision == revision => {}
                _ => return false,
            }
            guard = self
                .signal
                .changed
                .wait_timeout(guard, Duration::from_millis(250))
                .unwrap()
                .0;
        }
    }

    /// Background analyses that have run to completion since the app started.
    #[doc(hidden)]
    pub fn completed_analyses(&self) -> u64 {
        self.signal.completed.load(Ordering::Relaxed)
    }

    /// Write file `id`'s audio to `target`, or over its own file when `None`. If the
    /// current edit state hasn't been analysed yet, it is analysed first, so the saved
    /// file starts with exact peaks and statistics. `progress(done, total)` covers both.
    /// Afterwards the file is served from the saved copy, with an empty history. On
    /// failure any existing file at the target is untouched and the edits are kept.
    pub fn save(
        &self,
        id: FileId,
        target: Option<&Path>,
        progress: impl FnMut(u64, u64),
    ) -> Result<EditOutcome, String> {
        let token = {
            let mut loading = self.loading.lock().unwrap();
            if loading.contains_key(&id) {
                return Err("The file is still being saved".to_string());
            }
            let token = Arc::new(AtomicBool::new(false));
            loading.insert(id, token.clone());
            token
        };
        let result = self.save_inner(id, target, &token, progress);
        let mut loading = self.loading.lock().unwrap();
        if loading.get(&id).is_some_and(|t| Arc::ptr_eq(t, &token)) {
            loading.remove(&id);
        }
        result
    }

    fn save_inner(
        &self,
        id: FileId,
        target: Option<&Path>,
        cancel: &AtomicBool,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<EditOutcome, String> {
        // Range-stats workers and the background analysis hold the audio (and so the
        // mapping) while they run.
        self.stats.cancel();
        self.stop_analysis(id);
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

        let frames = file.audio.frames();
        let chain = file.chain.clone();
        // With effects, the saved audio is analysed as it is written: the edited audio's
        // analysis would be thrown away.
        let effects = chain.is_active();
        let temp = temp_path(&target);
        let written = if effects {
            self.render_file(&file, &chain, &temp, cancel, |done| progress(done, frames))
        } else {
            // Analyse first if the background pass hasn't finished, then write.
            let (analysis, written_from, total) = match &file.analysis {
                Some(a) => (a.clone(), 0, frames),
                None => {
                    let a = analyse_audio(&file.audio, file.revision, cancel, |done| {
                        progress(done, 2 * frames)
                    })
                    .ok_or(CANCELLED)?;
                    (a, frames, 2 * frames)
                }
            };
            let written = write_file(&*file.audio, &temp, |done| {
                progress(written_from + done, total)
            });
            written.map(|()| (analysis, 0)).map_err(|e| e.to_string())
        };
        let (analysis, clamped) = match written {
            Ok(done) if !cancel.load(Ordering::Relaxed) => done,
            failed => {
                let _ = std::fs::remove_file(&temp);
                if effects {
                    // Keep the chain playable as it was.
                    let _ = fx::render::realtime(&*self.ui, &chain);
                }
                return Err(match failed {
                    Err(e) if !cancel.load(Ordering::Relaxed) => fail(&e),
                    _ => CANCELLED.to_string(),
                });
            }
        };

        let mut history = self.history.lock().unwrap();
        let mut files = self.files.write().unwrap();
        if overwrite {
            // Windows can't replace a file that is still memory-mapped: release ours first.
            files.remove(&id);
        }
        let old_meta = file.metadata.clone();
        let old_stamp = file.disk_stamp;
        let old_revision = file.revision;
        let (old_pyramid, old_whole) = (file.source_pyramid.clone(), file.source_whole.clone());
        let edits = file.audio.edits().clone();
        // What was written was processed, so it doesn't describe the edited audio.
        let edited_analysis = if effects {
            file.analysis.clone()
        } else {
            Some(analysis.clone())
        };
        drop(file);

        if let Err(e) = rename_with_retry(&temp, &target) {
            let _ = std::fs::remove_file(&temp);
            let mut reopened = None;
            if overwrite {
                // The original is unchanged: map it again with the same edits.
                match SampleSource::open(&old_meta.path) {
                    Ok(src) => {
                        let audio = Arc::new(Audio::with_edits(Arc::new(src), edits));
                        let file = Arc::new(LoadedFile {
                            metadata: old_meta,
                            disk_stamp: old_stamp,
                            audio,
                            revision: old_revision,
                            source_pyramid: old_pyramid,
                            source_whole: old_whole,
                            analysis: edited_analysis,
                            chain: chain.clone(),
                        });
                        files.insert(id, file.clone());
                        reopened = Some(file);
                    }
                    Err(_) => {
                        history.remove(&id);
                    }
                }
            }
            drop((files, history));
            if let Some(file) = reopened {
                // Only starts one if the edited audio wasn't analysed yet.
                self.restart_analysis(id, &file);
            }
            if effects {
                let _ = fx::render::realtime(&*self.ui, &chain);
            }
            self.signal.notify();
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
                drop((files, history));
                self.signal.notify();
                return Err(format!(
                    "Saved “{target_name}”, but could not reopen it: {e}"
                ));
            }
        };
        let audio = Arc::new(Audio::new(Arc::new(src)));
        let size = fs_meta.len();
        let metadata = Metadata::new(&target, target_name, size, audio.info());
        // The history starts again from the saved file. The effects are in it now, so the
        // chain is emptied, and the slots are released on the UI thread as the last
        // references to them go.
        let mut fresh = History::new(audio.edits().clone());
        fresh.revision = history.get(&id).map_or(old_revision, |h| h.revision) + 1;
        let revision = fresh.revision;
        let mut outcome = Self::outcome(&metadata, &fresh, true);
        outcome.clamped_samples = clamped;
        history.insert(id, fresh);
        // The analysis of what was written becomes the source's.
        files.insert(
            id,
            Arc::new(
                LoadedFile {
                    metadata: metadata.clone(),
                    disk_stamp: (size, fs_meta.modified().ok()),
                    audio,
                    revision,
                    source_pyramid: analysis.pyramid,
                    source_whole: analysis.whole,
                    analysis: None,
                    chain: Arc::default(),
                }
                .seeded(),
            ),
        );
        drop((files, history));
        self.signal.notify();
        Ok(outcome)
    }

    /// Write `file`'s audio through its active `chain` to a new file at `path`, analysing what
    /// is written in the same pass. Returns that analysis and the number of samples clamped.
    /// Leaves the chain's slots set up for offline rendering.
    fn render_file(
        &self,
        file: &LoadedFile,
        chain: &Chain,
        path: &Path,
        cancel: &AtomicBool,
        progress: impl FnMut(u64),
    ) -> Result<(EditAnalysis, u64), String> {
        fx::render::offline(&*self.ui, chain)?;
        // Built after going offline, which may change the slots' latency.
        let live = LiveChain::of(chain).ok_or("No effect processes the audio")?;
        let render = Render::new(&file.audio, live, cancel);
        write_file(&render, path, progress).map_err(|e| e.to_string())?;
        let rendered = render.finish().ok_or(CANCELLED)?;
        let analysis = EditAnalysis {
            revision: file.revision,
            pyramid: Arc::new(rendered.pyramid),
            whole: Arc::new(rendered.whole),
        };
        Ok((analysis, rendered.clamped))
    }

    /// Stats for `[start, end)` of file `id`, or its whole-file stats when `range` is
    /// `None`. Returns `Ok(None)` when superseded by a newer request, or for the whole
    /// file while its current edit state hasn't been analysed yet.
    pub fn compute_stats(
        &self,
        id: FileId,
        range: Option<(u64, u64)>,
    ) -> Result<Option<Stats>, String> {
        let file = self.require(id)?;
        let Some((start, end)) = range else {
            self.stats.cancel();
            return Ok(file.analysis.as_ref().map(|a| a.whole.stats.clone()));
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

    /// Clip runs of file `id`: exact once analysed, else the source's runs that survive
    /// the edits unchanged.
    pub fn clip_runs(&self, id: FileId) -> Result<ClipRuns, String> {
        let file = self.require(id)?;
        let capped = |whole: &Analysis| {
            whole
                .stats
                .channels
                .iter()
                .any(|c| c.clip_runs > MAX_STORED_RUNS as u64)
        };
        Ok(match &file.analysis {
            Some(a) => ClipRuns {
                channels: a.whole.clip_runs.clone(),
                capped: capped(&a.whole),
                pending: false,
            },
            None => ClipRuns {
                channels: file
                    .source_whole
                    .clip_runs
                    .iter()
                    .map(|runs| map_runs(runs, file.audio.spans()))
                    .collect(),
                capped: capped(&file.source_whole),
                pending: true,
            },
        })
    }
}

/// A setting tried out while playing, before it is committed.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Preview {
    Clip {
        clip: ClipSettings,
    },
    Crossfade {
        join: usize,
        xfade: FadeSetting,
    },
    Fade {
        op: EditOp,
        start: u64,
        end: u64,
        bend: f64,
    },
}

impl ClipSettings {
    fn apply(&self, list: &EditList) -> EditList {
        list.set_trim(self.trim_start, self.trim_end)
            .set_clip_fades(self.fade_in.into(), self.fade_out.into())
    }
}

fn apply_crossfade(list: &EditList, join: usize, xfade: FadeSetting) -> Result<EditList, String> {
    let xfade = Crossfade {
        len: xfade.len,
        curve: Curve::new(xfade.bend),
    };
    list.set_crossfade(join, xfade)
        .map_err(|e| format!("Can't change the crossfade: {e}"))
}

/// One pass over `audio` for its statistics and peaks (`None` if cancelled).
fn analyse_audio(
    audio: &Audio,
    revision: u64,
    cancel: &AtomicBool,
    progress: impl FnMut(u64),
) -> Option<EditAnalysis> {
    let total = audio.frames();
    let mut builder = PyramidBuilder::new(audio.channels(), total);
    let whole = stats::analyse(audio, 0, total, cancel, Some(&mut builder), progress).ok()?;
    Some(EditAnalysis {
        revision,
        pyramid: Arc::new(builder.finish()),
        whole: Arc::new(whole),
    })
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
    let analysis = analyse_audio(&audio, 0, cancel, |done| progress(done, total))
        .ok_or_else(|| OpenError::Cancelled(name.to_string()))?;

    Ok(Arc::new(
        LoadedFile {
            metadata: Metadata::new(path, name.to_string(), size_bytes, audio.info()),
            disk_stamp: (size_bytes, fs_meta.modified().ok()),
            audio,
            revision: 0,
            source_pyramid: analysis.pyramid,
            source_whole: analysis.whole,
            analysis: None,
            chain: Arc::default(),
        }
        .seeded(),
    ))
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
fn write_file(
    audio: &impl NativeFrames,
    path: &Path,
    progress: impl FnMut(u64),
) -> Result<(), crate::wav::WriteError> {
    let file = std::fs::File::create_new(path)?;
    write_wav_with_progress(audio, &file, progress)?;
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
    /// With the source's analysis as the analysis of `audio` when it is the source
    /// unedited (as opened, saved, or undone back to that).
    fn seeded(self) -> Self {
        if self.analysis.is_some() || !self.audio.edits().is_identity() {
            return self;
        }
        let analysis = EditAnalysis {
            revision: self.revision,
            pyramid: self.source_pyramid.clone(),
            whole: self.source_whole.clone(),
        };
        self.with_analysis(analysis)
    }

    fn with_analysis(&self, analysis: EditAnalysis) -> Self {
        Self {
            analysis: Some(analysis),
            ..self.with_chain(self.chain.clone())
        }
    }

    fn with_chain(&self, chain: Arc<Chain>) -> Self {
        Self {
            metadata: self.metadata.clone(),
            disk_stamp: self.disk_stamp,
            audio: self.audio.clone(),
            revision: self.revision,
            source_pyramid: self.source_pyramid.clone(),
            source_whole: self.source_whole.clone(),
            analysis: self.analysis.clone(),
            chain,
        }
    }

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
