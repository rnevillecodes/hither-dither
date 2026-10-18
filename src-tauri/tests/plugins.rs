//! Integration tests for VST3 hosting, using the plugins in `test-plugins/`.

mod common;

use common::test_plugins::{self, TestPlugin};
use hither_dither_lib::plugins::ClassId;
use hither_dither_lib::plugins::cache::{
    LoadStep, PluginCache, PluginStatus, ScanProgress, find_bundles,
};
use hither_dither_lib::plugins::instance::{
    Config, Instance, InstanceError, State, Transport, speaker_arrangement,
};
use hither_dither_lib::plugins::module::{self, Module};
use hither_dither_lib::plugins::scan::{Layout, ScanResult, ScannedClass, Scanner};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use vst3::Steinberg::Vst::IEditControllerTrait;

#[test]
fn test_plugin_bundles_have_the_platform_layout() {
    let dir = tempfile::tempdir().unwrap();
    let bundles = test_plugins::bundle_all(dir.path());
    assert_eq!(bundles.len(), 3);
    for (bundle, plugin) in bundles.iter().zip(TestPlugin::ALL) {
        let name = plugin.package();
        assert_eq!(bundle, &dir.path().join(format!("{name}.vst3")));
        let contents = bundle.join("Contents");
        let binary = if cfg!(target_os = "macos") {
            assert!(contents.join("Info.plist").is_file());
            contents.join("MacOS").join(name)
        } else if cfg!(windows) {
            contents
                .join(test_plugins::binary_dir())
                .join(format!("{name}.vst3"))
        } else {
            contents
                .join(test_plugins::binary_dir())
                .join(format!("{name}.so"))
        };
        let size = std::fs::metadata(&binary).map(|m| m.len()).unwrap_or(0);
        assert!(size > 0, "{} is missing or empty", binary.display());
    }
    if !cfg!(target_os = "macos") {
        let arch_os = test_plugins::binary_dir();
        let (arch, os) = arch_os.split_once('-').unwrap();
        assert!(!arch.is_empty());
        assert_eq!(os, if cfg!(windows) { "win" } else { "linux" });
    }
}

const GAIN_PARAM: u32 = 0;
const LATENCY: usize = 64;

/// The gain parameter's normalised value for `db` (the plugin maps 0..1 onto ±24 dB).
fn gain_norm(db: f64) -> f64 {
    (db + 24.0) / 48.0
}

/// +6.02 dB, an exact doubling.
fn double() -> f64 {
    gain_norm(20.0 * 2f64.log10())
}

fn load_fx(dir: &tempfile::TempDir) -> Arc<Module> {
    let bundle = test_plugins::bundle(dir.path(), TestPlugin::Fx);
    module::load(&bundle).unwrap()
}

fn class(module: &Module, name: &str) -> ClassId {
    module
        .classes()
        .into_iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("{name} not in the factory"))
        .cid
}

fn stereo(max_block: usize) -> Config {
    Config {
        channels: 2,
        channel_mask: Some(0x3),
        sample_rate: 48_000.0,
        max_block,
        offline: false,
    }
}

fn ramp(channels: usize, frames: usize) -> Vec<Vec<f32>> {
    (0..channels)
        .map(|c| {
            let sign = if c % 2 == 0 { 1.0 } else { -1.0 };
            (0..frames)
                .map(|i| sign * (i + 1) as f32 / 4096.0)
                .collect()
        })
        .collect()
}

fn run(instance: &Instance, input: &[Vec<f32>]) -> Vec<Vec<f32>> {
    let frames = input[0].len();
    let mut output = vec![vec![f32::NAN; frames]; input.len()];
    let ins: Vec<&[f32]> = input.iter().map(|c| c.as_slice()).collect();
    let mut outs: Vec<&mut [f32]> = output.iter_mut().map(|c| c.as_mut_slice()).collect();
    let transport = Transport {
        playing: true,
        position: 0,
    };
    assert!(instance.process(&ins, &mut outs, frames, transport));
    output
}

/// Asserts `output` is `input` delayed by the plugin's latency and multiplied by `gain`.
fn assert_delayed(input: &[Vec<f32>], output: &[Vec<f32>], gain: f32) {
    for (c, (i, o)) in input.iter().zip(output).enumerate() {
        assert!(o[..LATENCY].iter().all(|&s| s == 0.0), "channel {c} leads");
        for n in LATENCY..o.len() {
            assert_eq!(o[n], i[n - LATENCY] * gain, "channel {c}, frame {n}");
        }
    }
}

#[test]
fn loads_the_test_bundle_and_lists_its_classes() {
    let dir = tempfile::tempdir().unwrap();
    let module = load_fx(&dir);
    let audio: Vec<_> = module
        .classes()
        .into_iter()
        .filter(|c| c.category == "Audio Module Class")
        .map(|c| (c.name, c.vendor, c.sub_categories, c.version))
        .collect();
    let expect = |name: &str, sub: &str| {
        (
            name.to_string(),
            "HitherDither Tests".to_string(),
            sub.to_string(),
            "1.0.0".to_string(),
        )
    };
    assert_eq!(
        audio,
        [
            expect("HD Test Gain", "Fx|Tools"),
            expect("HD Test Stereo", "Fx|Spatial"),
            expect("HD Test Synth", "Instrument|Synth"),
        ]
    );
    // Modules are cached for the life of the process.
    let again = module::load(module.path()).unwrap();
    assert!(Arc::ptr_eq(&module, &again));
}

#[test]
fn gain_doubles_stereo_samples_after_its_latency() {
    let dir = tempfile::tempdir().unwrap();
    let module = load_fx(&dir);
    let gain = Instance::create(module.clone(), class(&module, "HD Test Gain")).unwrap();
    assert_eq!(gain.configure(stereo(256)), Ok(LATENCY as u32));
    gain.set_param(GAIN_PARAM, double());
    // More than one block, so processing is split.
    let input = ramp(2, 1000);
    let output = run(&gain, &input);
    assert_delayed(&input, &output, 2.0);
}

#[test]
fn gain_accepts_every_layout_up_to_eight_channels() {
    let dir = tempfile::tempdir().unwrap();
    let module = load_fx(&dir);
    let gain = Instance::create(module.clone(), class(&module, "HD Test Gain")).unwrap();
    for (channels, mask) in [(1, None), (6, Some(0x3F)), (8, None)] {
        let config = Config {
            channels,
            channel_mask: mask,
            ..stereo(128)
        };
        assert_eq!(gain.configure(config), Ok(LATENCY as u32), "{channels}");
        let input = ramp(channels as usize, 300);
        assert_delayed(&input, &run(&gain, &input), 1.0);
    }
}

#[test]
fn stereo_only_plugin_refuses_5_1() {
    let dir = tempfile::tempdir().unwrap();
    let module = load_fx(&dir);
    let stereo_only = Instance::create(module.clone(), class(&module, "HD Test Stereo")).unwrap();
    let surround = Config {
        channels: 6,
        channel_mask: Some(0x3F),
        ..stereo(256)
    };
    assert_eq!(stereo_only.configure(surround), Err(InstanceError::Layout));
    assert!(stereo_only.configure(stereo(256)).is_ok());
}

#[test]
fn instrument_is_not_an_effect() {
    let dir = tempfile::tempdir().unwrap();
    let module = load_fx(&dir);
    let synth = Instance::create(module.clone(), class(&module, "HD Test Synth")).unwrap();
    assert_eq!(
        synth.configure(stereo(256)),
        Err(InstanceError::NotAnEffect)
    );
}

#[test]
fn state_round_trip_restores_the_gain() {
    let dir = tempfile::tempdir().unwrap();
    let module = load_fx(&dir);
    let cid = class(&module, "HD Test Gain");
    let first = Instance::create(module.clone(), cid).unwrap();
    first.configure(stereo(256)).unwrap();
    first.set_param(GAIN_PARAM, double());
    run(&first, &ramp(2, 16));
    let state = first.get_state().unwrap();
    assert_eq!(state.component, double().to_le_bytes());

    let second = Instance::create(module.clone(), cid).unwrap();
    assert!(second.set_state(&state));
    let controller = second.controller().unwrap();
    let shown = unsafe { controller.getParamNormalized(GAIN_PARAM) };
    assert_eq!(shown, double());
    second.configure(stereo(256)).unwrap();
    let input = ramp(2, 500);
    assert_delayed(&input, &run(&second, &input), 2.0);
    assert!(!second.set_state(&State::default()));
}

// --- Scanning ------------------------------------------------------------------------------

fn scanner() -> Scanner {
    Scanner::new(env!("CARGO_BIN_EXE_hither-dither").into())
}

fn ready_classes(result: &ScanResult) -> &[ScannedClass] {
    match result {
        ScanResult::Ready { classes, .. } => classes,
        other => panic!("expected a ready bundle, got {other:?}"),
    }
}

#[test]
fn scan_offers_effects_only_with_their_layouts() {
    let dir = tempfile::tempdir().unwrap();
    let bundle = test_plugins::bundle(dir.path(), TestPlugin::Fx);
    let result = scanner().scan(&bundle);
    let classes = ready_classes(&result);
    let names: Vec<_> = classes.iter().map(|c| c.name.as_str()).collect();
    // HD Test Synth is an instrument and isn't offered.
    assert_eq!(names, ["HD Test Gain", "HD Test Stereo"]);
    assert!(classes.iter().all(|c| !c.has_editor && c.error.is_none()));
    assert_eq!(classes[0].vendor, "HitherDither Tests");

    let gain = &classes[0];
    let counts: Vec<_> = gain.layouts.iter().map(|l| l.channels).collect();
    assert_eq!(counts, [1, 2, 3, 4, 4, 5, 5, 6, 7, 8, 8]);
    assert!(gain.supports(6, Some(0x3F)) && gain.supports(8, Some(0x63F)));

    let stereo = &classes[1];
    assert_eq!(
        stereo.layouts,
        [Layout {
            channels: 2,
            arrangement: speaker_arrangement(2, None),
        }]
    );
    assert!(stereo.supports(2, Some(0x3)));
    assert!(!stereo.supports(6, Some(0x3F)) && !stereo.supports(1, None));
}

#[test]
fn crashing_plugin_is_reported_and_isolated() {
    let dir = tempfile::tempdir().unwrap();
    let bundle = test_plugins::bundle(dir.path(), TestPlugin::Crash);
    assert_eq!(
        scanner().scan(&bundle),
        ScanResult::Failed {
            reason: "Crashed while being scanned".into()
        }
    );
}

#[test]
fn hanging_plugin_times_out_while_others_scan() {
    let dir = tempfile::tempdir().unwrap();
    let bundles = test_plugins::bundle_all(dir.path());
    let hang = test_plugins::bundle(dir.path(), TestPlugin::Hang);
    let mut scanner = scanner();
    scanner.timeout = Duration::from_secs(5);
    let start = Instant::now();
    let mut order = Vec::new();
    scanner.scan_all(&bundles, |bundle, result| {
        order.push((bundle.to_path_buf(), result));
    });
    // The others are scanned alongside it; the hanging plugin finishes last, at the timeout.
    assert!(start.elapsed() >= scanner.timeout);
    assert_eq!(order.len(), 3);
    let timed_out = ScanResult::Failed {
        reason: "Timed out while being scanned".into(),
    };
    assert!(order[..2].iter().all(|(_, r)| r != &timed_out));
    let (last, result) = order.last().unwrap();
    assert_eq!(last, &hang);
    assert_eq!(result, &timed_out);
    assert_eq!(scanner.launches(), 3);
}

#[test]
fn bundle_without_a_binary_for_this_platform_is_incompatible() {
    let dir = tempfile::tempdir().unwrap();
    let bundle = dir.path().join("Elsewhere.vst3");
    let other = if cfg!(windows) {
        "x86_64-linux"
    } else {
        "x86_64-win"
    };
    std::fs::create_dir_all(bundle.join("Contents").join(other)).unwrap();
    let scanner = scanner();
    assert!(matches!(
        scanner.scan(&bundle),
        ScanResult::Incompatible { reason } if reason.starts_with("No version for this")
    ));
    assert_eq!(scanner.launches(), 0);
}

// --- Plugin cache --------------------------------------------------------------------------

struct CacheFixture {
    data: tempfile::TempDir,
    folder: tempfile::TempDir,
    missing: PathBuf,
}

impl CacheFixture {
    /// A plugin folder holding HD Test Fx and HD Test Crash, and a missing standard folder.
    fn new() -> CacheFixture {
        let data = tempfile::tempdir().unwrap();
        let folder = tempfile::tempdir().unwrap();
        test_plugins::bundle(folder.path(), TestPlugin::Fx);
        test_plugins::bundle(folder.path(), TestPlugin::Crash);
        let missing = folder.path().join("not-installed");
        CacheFixture {
            data,
            folder,
            missing,
        }
    }

    /// Opens the cache as the app does at startup, with a fresh scanner.
    fn open(&self) -> PluginCache {
        let standard = vec![self.folder.path().to_path_buf(), self.missing.clone()];
        PluginCache::open(self.data.path().to_path_buf(), standard, scanner())
    }

    fn bundle(&self, plugin: TestPlugin) -> PathBuf {
        self.folder
            .path()
            .join(format!("{}.vst3", plugin.package()))
    }
}

/// Rescans and returns the progress reports.
fn rescan(cache: &PluginCache, all: bool) -> Vec<ScanProgress> {
    let mut reports = Vec::new();
    cache.rescan(all, |p| reports.push(p));
    reports
}

fn summary(cache: &PluginCache) -> Vec<(String, PluginStatus, Option<String>)> {
    cache
        .list()
        .plugins
        .into_iter()
        .map(|p| (p.name, p.status, p.reason))
        .collect()
}

#[test]
fn first_scan_lists_effects_and_failures() {
    let fx = CacheFixture::new();
    let cache = fx.open();
    let reports = rescan(&cache, false);
    assert_eq!(reports.first(), Some(&ScanProgress { done: 0, total: 2 }));
    assert_eq!(reports.last(), Some(&ScanProgress { done: 2, total: 2 }));
    assert_eq!(
        summary(&cache),
        [
            ("HD Test Gain".into(), PluginStatus::Ready, None),
            ("HD Test Stereo".into(), PluginStatus::Ready, None),
            (
                "hd-test-crash".into(),
                PluginStatus::Failed,
                Some("Crashed while being scanned".into())
            ),
        ]
    );
    let gain = &cache.list().plugins[0];
    assert_eq!(gain.path, fx.bundle(TestPlugin::Fx));
    assert_eq!(gain.version, "1.0.0");
}

#[test]
fn second_scan_of_unchanged_bundles_launches_nothing() {
    let fx = CacheFixture::new();
    let first = fx.open();
    rescan(&first, false);
    assert_eq!(first.scanner().launches(), 2);

    // As at the next launch: everything is listed straight away, without scanning.
    let second = fx.open();
    assert_eq!(summary(&second), summary(&first));
    assert_eq!(rescan(&second, false), [ScanProgress { done: 0, total: 0 }]);
    assert_eq!(second.scanner().launches(), 0);
    assert_eq!(summary(&second), summary(&first));

    // Rescan All scans everything again.
    rescan(&second, true);
    assert_eq!(second.scanner().launches(), 2);
}

#[test]
fn touching_one_bundle_rescans_only_it() {
    let fx = CacheFixture::new();
    let cache = fx.open();
    rescan(&cache, false);
    let binary = module::binary_path(&fx.bundle(TestPlugin::Fx)).unwrap();
    let later = std::time::SystemTime::now() + Duration::from_secs(60);
    std::fs::File::options()
        .write(true)
        .open(&binary)
        .unwrap()
        .set_modified(later)
        .unwrap();
    let reports = rescan(&cache, false);
    assert_eq!(reports.last(), Some(&ScanProgress { done: 1, total: 1 }));
    assert_eq!(cache.scanner().launches(), 3);
    assert_eq!(summary(&cache).len(), 3);
}

#[test]
fn removed_bundles_disappear() {
    let fx = CacheFixture::new();
    let cache = fx.open();
    rescan(&cache, false);
    std::fs::remove_dir_all(fx.bundle(TestPlugin::Crash)).unwrap();
    rescan(&cache, false);
    let names: Vec<_> = summary(&cache).into_iter().map(|(n, ..)| n).collect();
    assert_eq!(names, ["HD Test Gain", "HD Test Stereo"]);
    assert_eq!(cache.scanner().launches(), 2);
    // And stay gone after a restart.
    assert_eq!(summary(&fx.open()).len(), 2);
}

#[test]
fn missing_standard_folder_is_skipped_silently() {
    let fx = CacheFixture::new();
    assert!(!fx.missing.exists());
    assert!(find_bundles(std::slice::from_ref(&fx.missing)).is_empty());
    let cache = fx.open();
    assert_eq!(cache.folders().standard[1], fx.missing);
    rescan(&cache, false);
    assert_eq!(summary(&cache).len(), 3);
    assert!(
        cache
            .list()
            .plugins
            .iter()
            .all(|p| !p.path.starts_with(&fx.missing))
    );
}

#[test]
fn extra_folders_and_disabled_plugins_are_remembered() {
    let fx = CacheFixture::new();
    let extra = tempfile::tempdir().unwrap();
    test_plugins::bundle(extra.path(), TestPlugin::Fx);
    let cache = fx.open();
    cache
        .set_extra_folders(vec![extra.path().to_path_buf()])
        .unwrap();
    rescan(&cache, false);
    assert_eq!(summary(&cache).len(), 5);

    let gain = cache.list().plugins[0].class_id.unwrap();
    cache.set_enabled(gain, false).unwrap();
    let reopened = fx.open();
    assert_eq!(reopened.folders().extra, [extra.path().to_path_buf()]);
    let disabled: Vec<_> = reopened
        .list()
        .plugins
        .into_iter()
        .filter(|p| p.status == PluginStatus::Disabled)
        .map(|p| p.name)
        .collect();
    // The same class in both folders.
    assert_eq!(disabled, ["HD Test Gain", "HD Test Gain"]);

    // Removing the folder drops its plugins.
    reopened.set_extra_folders(Vec::new()).unwrap();
    rescan(&reopened, false);
    assert_eq!(summary(&reopened).len(), 3);
}

// --- Crash marker --------------------------------------------------------------------------

fn status_of(cache: &PluginCache, name: &str) -> PluginStatus {
    let list = cache.list().plugins;
    list.iter().find(|p| p.name == name).unwrap().status
}

#[test]
fn leftover_marker_disables_the_class_with_one_notice() {
    let fx = CacheFixture::new();
    let cache = fx.open();
    rescan(&cache, false);
    let gain = cache.list().plugins[0].class_id.unwrap();
    let marker = fx.data.path().join("plugin-loading.json");

    // A step that finishes leaves no marker, and the next time needs none.
    drop(cache.loading(gain, LoadStep::Create));
    assert!(!marker.exists());
    let again = cache.loading(gain, LoadStep::Create);
    assert!(!marker.exists());
    drop(again);

    // The app dies while opening the editor for the first time.
    let guard = cache.loading(gain, LoadStep::OpenEditor);
    assert!(marker.exists());
    std::mem::forget(guard);
    drop(cache);

    let restarted = fx.open();
    assert!(!marker.exists());
    assert_eq!(
        status_of(&restarted, "HD Test Gain"),
        PluginStatus::Disabled
    );
    assert_eq!(status_of(&restarted, "HD Test Stereo"), PluginStatus::Ready);
    let notices = restarted.take_notices();
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].class_id, Some(gain));
    assert_eq!(
        notices[0].message,
        "HD Test Gain was disabled because HitherDither closed unexpectedly while opening \
         its editor. You can enable it again in Manage Plugins."
    );
    assert!(restarted.take_notices().is_empty());
    // Only once: the marker is gone after it has been handled.
    assert!(fx.open().take_notices().is_empty());

    // Enabling it again clears the disabled entry, which stays cleared.
    restarted.set_enabled(gain, true).unwrap();
    assert_eq!(status_of(&restarted, "HD Test Gain"), PluginStatus::Ready);
    let reopened = fx.open();
    assert!(reopened.settings().disabled.is_empty());
    assert_eq!(status_of(&reopened, "HD Test Gain"), PluginStatus::Ready);
}
