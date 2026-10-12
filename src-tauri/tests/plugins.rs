//! Integration tests for VST3 hosting, using the plugins in `test-plugins/`.

mod common;

use common::test_plugins::{self, TestPlugin};
use hither_dither_lib::plugins::ClassId;
use hither_dither_lib::plugins::instance::{Config, Instance, InstanceError, State, Transport};
use hither_dither_lib::plugins::module::{self, Module};
use std::sync::Arc;
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
