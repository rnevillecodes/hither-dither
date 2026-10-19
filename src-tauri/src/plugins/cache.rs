//! The plugin list: where plugins are looked for, the remembered scan results, and the user's
//! plugin settings.
//!
//! Files in the app data directory:
//! - `plugins.json`: scan results keyed by bundle path, with the size and modification time
//!   of the bundle's binary, so unchanged bundles aren't scanned again;
//! - `plugin-settings.json`: the extra folders and the disabled class IDs;
//! - `plugin-loading.json`: the crash marker, present only while plugin code runs in this
//!   process for the first time (see [`PluginCache::loading`]).

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use super::ClassId;
use super::module;
use super::scan::{Layout, ScanResult, ScannedClass, Scanner};

const CACHE_FILE: &str = "plugins.json";
const SETTINGS_FILE: &str = "plugin-settings.json";
const MARKER_FILE: &str = "plugin-loading.json";
const CACHE_VERSION: u32 = 1;
/// How deep to look inside plugin folders, which also stops symlink loops.
const MAX_DEPTH: usize = 8;

/// The platform's standard VST3 folders.
pub fn standard_folders() -> Vec<PathBuf> {
    let env = |name| std::env::var_os(name).map(PathBuf::from);
    let mut folders = Vec::new();
    if cfg!(windows) {
        folders.extend(env("COMMONPROGRAMFILES").map(|p| p.join("VST3")));
        folders.extend(env("LOCALAPPDATA").map(|p| p.join("Programs/Common/VST3")));
    } else if cfg!(target_os = "macos") {
        folders.push("/Library/Audio/Plug-Ins/VST3".into());
        folders.extend(env("HOME").map(|p| p.join("Library/Audio/Plug-Ins/VST3")));
    } else {
        folders.extend(env("HOME").map(|p| p.join(".vst3")));
        folders.push("/usr/lib/vst3".into());
        folders.push("/usr/local/lib/vst3".into());
    }
    folders
}

/// Every `.vst3` bundle or file in `folders` and their subfolders, sorted. Missing folders are
/// skipped.
pub fn find_bundles(folders: &[PathBuf]) -> Vec<PathBuf> {
    fn walk(dir: &Path, depth: usize, found: &mut BTreeSet<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for path in entries.flatten().map(|e| e.path()) {
            let is_vst3 = path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("vst3"));
            if is_vst3 {
                found.insert(path);
            } else if depth < MAX_DEPTH && path.is_dir() {
                walk(&path, depth + 1, found);
            }
        }
    }
    let mut found = BTreeSet::new();
    for folder in folders {
        walk(folder, 0, &mut found);
    }
    found.into_iter().collect()
}

/// Identifies a version of a bundle: the size and modification time of its binary (or of the
/// bundle itself when it has no binary for this platform).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct Fingerprint {
    size: u64,
    /// Nanoseconds since the Unix epoch.
    modified: u64,
}

fn fingerprint(bundle: &Path) -> Option<Fingerprint> {
    let path = module::binary_path(bundle).unwrap_or_else(|| bundle.to_path_buf());
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some(Fingerprint {
        size: meta.len(),
        modified: u64::try_from(modified.as_nanos()).unwrap_or(u64::MAX),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BundleRecord {
    fingerprint: Option<Fingerprint>,
    result: ScanResult,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CacheFile {
    version: u32,
    bundles: BTreeMap<PathBuf, BundleRecord>,
}

/// The user's plugin settings.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub extra_folders: Vec<PathBuf>,
    pub disabled: BTreeSet<ClassId>,
}

/// Scan progress, sent with `plugins-changed`. A scan is running while `done < total`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct ScanProgress {
    pub done: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PluginStatus {
    Ready,
    Failed,
    Incompatible,
    Disabled,
    /// The bundle loaded but has no effect classes, such as an instrument.
    NoEffects,
}

/// One row of the plugin list: an effect class, or a bundle with no usable effect classes.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginEntry {
    pub path: PathBuf,
    /// `None` for a bundle that couldn't be scanned or has no effects.
    pub class_id: Option<ClassId>,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub sub_categories: String,
    pub has_editor: bool,
    pub layouts: Vec<Layout>,
    pub status: PluginStatus,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PluginList {
    pub plugins: Vec<PluginEntry>,
    pub progress: ScanProgress,
}

/// The standard folders, which can't be removed, and the user's extra folders.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Folders {
    pub standard: Vec<PathBuf>,
    pub extra: Vec<PathBuf>,
}

/// A step that runs plugin code in the app's own process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LoadStep {
    /// Creating and initialising an instance.
    Create,
    OpenEditor,
}

/// The contents of `plugin-loading.json`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Marker {
    class_id: ClassId,
    name: String,
    step: LoadStep,
}

impl Marker {
    fn notice(&self) -> StartupNotice {
        let name = if self.name.is_empty() {
            "A plugin"
        } else {
            &self.name
        };
        let doing = match self.step {
            LoadStep::Create => "loading it",
            LoadStep::OpenEditor => "opening its editor",
        };
        StartupNotice {
            class_id: Some(self.class_id),
            name: self.name.clone(),
            title: format!("{name} was disabled"),
            message: format!(
                "{name} was disabled because HitherDither closed unexpectedly while {doing}. \
                 You can enable it again in Manage Plugins."
            ),
        }
    }
}

/// Something to tell the user once at startup: a plugin disabled because the app closed
/// while it was loading, or saved app data that couldn't be read.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupNotice {
    /// The disabled plugin, if the notice is about one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_id: Option<ClassId>,
    pub name: String,
    pub title: String,
    pub message: String,
}

/// Keeps the crash marker in place while plugin code runs; see [`PluginCache::loading`].
#[must_use]
pub struct LoadingGuard<'a> {
    cache: &'a PluginCache,
    step: Option<(ClassId, LoadStep)>,
}

impl Drop for LoadingGuard<'_> {
    fn drop(&mut self) {
        if let Some(step) = self.step {
            let _ = std::fs::remove_file(self.cache.dir.join(MARKER_FILE));
            lock(&self.cache.inner).loaded.insert(step);
        }
    }
}

struct Inner {
    bundles: BTreeMap<PathBuf, BundleRecord>,
    settings: Settings,
    progress: ScanProgress,
    notices: Vec<StartupNotice>,
    /// Steps that have run once this session, which need no crash marker again.
    loaded: HashSet<(ClassId, LoadStep)>,
}

#[derive(Default)]
struct RescanQueue {
    running: bool,
    /// A requested rescan, and whether it is Rescan All.
    requested: Option<bool>,
}

/// The known plugins, kept in step with the plugin folders by rescans.
pub struct PluginCache {
    dir: PathBuf,
    standard: Vec<PathBuf>,
    scanner: Scanner,
    inner: Mutex<Inner>,
    queue: Mutex<RescanQueue>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// Writes `value` to `path` through a temporary file, so a crash never leaves half a file.
pub(crate) fn write_json(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    std::fs::rename(&tmp, path)
}

impl PluginCache {
    /// Reads the cache and settings from `dir` (the app data directory). `standard` is the
    /// list of standard folders, normally [`standard_folders`].
    pub fn open(dir: PathBuf, standard: Vec<PathBuf>, scanner: Scanner) -> PluginCache {
        let bundles = read_json::<CacheFile>(&dir.join(CACHE_FILE))
            .filter(|c| c.version == CACHE_VERSION)
            .map(|c| c.bundles)
            .unwrap_or_default();
        let mut settings: Settings = read_json(&dir.join(SETTINGS_FILE)).unwrap_or_default();
        let mut notices = Vec::new();
        let marker_path = dir.join(MARKER_FILE);
        if let Some(marker) = read_json::<Marker>(&marker_path) {
            // The app closed while this plugin was loading: disable it and say so.
            settings.disabled.insert(marker.class_id);
            if write_json(&dir.join(SETTINGS_FILE), &settings).is_ok() {
                notices.push(marker.notice());
                let _ = std::fs::remove_file(&marker_path);
            }
        } else {
            let _ = std::fs::remove_file(&marker_path);
        }
        PluginCache {
            dir,
            standard,
            scanner,
            inner: Mutex::new(Inner {
                bundles,
                settings,
                progress: ScanProgress::default(),
                notices,
                loaded: HashSet::new(),
            }),
            queue: Mutex::default(),
        }
    }

    pub fn scanner(&self) -> &Scanner {
        &self.scanner
    }

    pub fn folders(&self) -> Folders {
        Folders {
            standard: self.standard.clone(),
            extra: lock(&self.inner).settings.extra_folders.clone(),
        }
    }

    pub fn settings(&self) -> Settings {
        lock(&self.inner).settings.clone()
    }

    /// Replaces the extra folders. The caller starts a rescan.
    pub fn set_extra_folders(&self, folders: Vec<PathBuf>) -> std::io::Result<()> {
        let mut inner = lock(&self.inner);
        let mut unique = Vec::new();
        for f in folders {
            if !unique.contains(&f) && !self.standard.contains(&f) {
                unique.push(f);
            }
        }
        inner.settings.extra_folders = unique;
        self.save_settings(&inner)
    }

    /// Enables or disables an effect class.
    pub fn set_enabled(&self, class_id: ClassId, enabled: bool) -> std::io::Result<()> {
        let mut inner = lock(&self.inner);
        let changed = if enabled {
            inner.settings.disabled.remove(&class_id)
        } else {
            inner.settings.disabled.insert(class_id)
        };
        if changed {
            self.save_settings(&inner)?;
        }
        Ok(())
    }

    /// Marks `step` as running for `class_id` until the guard drops. The first time a class
    /// runs a step this session, this writes the crash marker, so that if the app dies
    /// meanwhile the class is disabled at the next launch.
    pub fn loading(&self, class_id: ClassId, step: LoadStep) -> LoadingGuard<'_> {
        let first = !lock(&self.inner).loaded.contains(&(class_id, step));
        if first {
            let name = self
                .find_class(class_id)
                .map(|(_, c)| c.name)
                .unwrap_or_default();
            let marker = Marker {
                class_id,
                name,
                step,
            };
            // Without the marker a crash goes unexplained, but loading can still go ahead.
            let _ = write_json(&self.dir.join(MARKER_FILE), &marker);
        }
        LoadingGuard {
            cache: self,
            step: first.then_some((class_id, step)),
        }
    }

    /// Adds a notice from elsewhere in the app to those [`take_notices`](Self::take_notices)
    /// returns.
    pub fn add_notice(&self, notice: StartupNotice) {
        lock(&self.inner).notices.push(notice);
    }

    /// The notices for plugins disabled at startup, and any added, returned only once.
    pub fn take_notices(&self) -> Vec<StartupNotice> {
        std::mem::take(&mut lock(&self.inner).notices)
    }

    pub fn progress(&self) -> ScanProgress {
        lock(&self.inner).progress
    }

    /// Every plugin found, sorted by name, with the scan progress.
    pub fn list(&self) -> PluginList {
        let inner = lock(&self.inner);
        let mut plugins = Vec::new();
        for (path, record) in &inner.bundles {
            let bundle_name = || {
                path.file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default()
            };
            let bundle_entry = |vendor: &str, status, reason: &str| PluginEntry {
                path: path.clone(),
                class_id: None,
                name: bundle_name(),
                vendor: vendor.to_string(),
                version: String::new(),
                sub_categories: String::new(),
                has_editor: false,
                layouts: Vec::new(),
                status,
                reason: Some(reason.to_string()),
            };
            match &record.result {
                ScanResult::Ready { vendor, classes } if classes.is_empty() => {
                    plugins.push(bundle_entry(
                        vendor,
                        PluginStatus::NoEffects,
                        "No usable effects",
                    ));
                }
                ScanResult::Ready { classes, .. } => {
                    for class in classes {
                        plugins.push(class_entry(path, class, &inner.settings));
                    }
                }
                ScanResult::Failed { reason } => {
                    plugins.push(bundle_entry("", PluginStatus::Failed, reason));
                }
                ScanResult::Incompatible { reason } => {
                    plugins.push(bundle_entry("", PluginStatus::Incompatible, reason));
                }
            }
        }
        plugins.sort_by_cached_key(|p| (p.name.to_lowercase(), p.path.clone()));
        PluginList {
            plugins,
            progress: inner.progress,
        }
    }

    /// The bundle and scan details of an effect class.
    pub fn find_class(&self, class_id: ClassId) -> Option<(PathBuf, ScannedClass)> {
        let inner = lock(&self.inner);
        inner
            .bundles
            .iter()
            .find_map(|(path, record)| match &record.result {
                ScanResult::Ready { classes, .. } => classes
                    .iter()
                    .find(|c| c.class_id == class_id)
                    .map(|c| (path.clone(), c.clone())),
                _ => None,
            })
    }

    /// Brings the cache in line with the plugin folders: forgets bundles that are gone and
    /// scans those that are new or changed (or, with `all`, every bundle). Calls `progress`
    /// as it goes. Blocks until the scan finishes.
    pub fn rescan(&self, all: bool, mut progress: impl FnMut(ScanProgress)) {
        let mut folders = self.standard.clone();
        folders.extend(self.folders().extra);
        let found = find_bundles(&folders);
        let queue: Vec<(PathBuf, Option<Fingerprint>)> = {
            let mut inner = lock(&self.inner);
            inner
                .bundles
                .retain(|path, _| found.binary_search(path).is_ok());
            let queue: Vec<_> = found
                .into_iter()
                .map(|path| {
                    let fp = fingerprint(&path);
                    (path, fp)
                })
                .filter(|(path, fp)| {
                    all || fp.is_none()
                        || inner.bundles.get(path).is_none_or(|r| r.fingerprint != *fp)
                })
                .collect();
            inner.progress = ScanProgress {
                done: 0,
                total: queue.len(),
            };
            self.save_cache(&inner);
            queue
        };
        progress(self.progress());

        let paths: Vec<PathBuf> = queue.iter().map(|(p, _)| p.clone()).collect();
        let fingerprints: BTreeMap<_, _> = queue.into_iter().collect();
        self.scanner.scan_all(&paths, |path, result| {
            let current = {
                let mut inner = lock(&self.inner);
                let fingerprint = fingerprints.get(path).copied().flatten();
                inner.bundles.insert(
                    path.to_path_buf(),
                    BundleRecord {
                        fingerprint,
                        result,
                    },
                );
                inner.progress.done += 1;
                self.save_cache(&inner);
                inner.progress
            };
            progress(current);
        });
    }

    /// Runs [`PluginCache::rescan`] on a background thread. A request made while a rescan is
    /// running is queued, and requests made meanwhile are merged into it.
    pub fn start_rescan(
        self: &Arc<Self>,
        all: bool,
        notify: impl Fn(ScanProgress) + Send + 'static,
    ) {
        {
            let mut queue = lock(&self.queue);
            queue.requested = Some(queue.requested.unwrap_or(false) || all);
            if queue.running {
                return;
            }
            queue.running = true;
        }
        let this = self.clone();
        std::thread::spawn(move || {
            loop {
                let all = {
                    let mut queue = lock(&this.queue);
                    match queue.requested.take() {
                        Some(all) => all,
                        None => {
                            queue.running = false;
                            return;
                        }
                    }
                };
                this.rescan(all, &notify);
            }
        });
    }

    fn save_cache(&self, inner: &Inner) {
        let file = CacheFile {
            version: CACHE_VERSION,
            bundles: inner.bundles.clone(),
        };
        // The cache only saves time; a failed write means scanning again next launch.
        let _ = write_json(&self.dir.join(CACHE_FILE), &file);
    }

    fn save_settings(&self, inner: &Inner) -> std::io::Result<()> {
        write_json(&self.dir.join(SETTINGS_FILE), &inner.settings)
    }
}

fn class_entry(path: &Path, class: &ScannedClass, settings: &Settings) -> PluginEntry {
    let (status, reason) = if let Some(error) = &class.error {
        (PluginStatus::Failed, Some(error.clone()))
    } else if settings.disabled.contains(&class.class_id) {
        (PluginStatus::Disabled, None)
    } else {
        (PluginStatus::Ready, None)
    };
    PluginEntry {
        path: path.to_path_buf(),
        class_id: Some(class.class_id),
        name: class.name.clone(),
        vendor: class.vendor.clone(),
        version: class.version.clone(),
        sub_categories: class.sub_categories.clone(),
        has_editor: class.has_editor,
        layouts: class.layouts.clone(),
        status,
        reason,
    }
}
