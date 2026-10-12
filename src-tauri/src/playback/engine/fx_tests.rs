//! Playback through effects: the feeder and engine together, with HD Test Gain from
//! `test-plugins/` (+6.02 dB, 64 frames of latency) or synthetic stages.

use super::*;
use crate::fx::chain::{Chain, LIVE_BLOCK, Slot};
use crate::fx::live::Stage;
use crate::plugins::instance::{Config, Instance, Transport};
use crate::plugins::main_thread::Inline;
use crate::plugins::module;
use crate::plugins::scan::ScannedClass;
use crate::wav::synth::{Encoding, WavBuilder};

#[path = "../../../tests/common/test_plugins.rs"]
#[allow(dead_code)]
mod test_plugins;

const GAIN_PARAM: u32 = 0;

/// HD Test Gain's normalised value for `db` (it maps 0..1 onto ±24 dB).
fn gain_norm(db: f64) -> f64 {
    (db + 24.0) / 48.0
}

/// +6.02 dB, an exact doubling.
fn double() -> f64 {
    gain_norm(20.0 * 2f64.log10())
}

/// An HD Test Gain slot for mono audio at `rate`, with its gain at `norm`.
struct Gain {
    slot: Arc<Slot>,
    _dir: tempfile::TempDir,
}

fn gain(rate: u32, norm: f64) -> Gain {
    let dir = tempfile::tempdir().unwrap();
    let bundle = test_plugins::bundle(dir.path(), test_plugins::TestPlugin::Fx);
    let module = module::load(&bundle).unwrap();
    let info = module
        .classes()
        .into_iter()
        .find(|c| c.name == "HD Test Gain")
        .unwrap();
    let instance = Instance::create(module, info.cid).unwrap();
    let config = Config {
        channels: 1,
        channel_mask: None,
        sample_rate: rate as f64,
        max_block: LIVE_BLOCK,
        offline: false,
    };
    let latency = instance.configure(config).unwrap();
    assert_eq!(latency, 64);
    instance.set_param(GAIN_PARAM, norm);
    let class = ScannedClass {
        class_id: info.cid,
        name: info.name,
        vendor: info.vendor,
        version: info.version,
        sub_categories: info.sub_categories,
        has_editor: false,
        layouts: Vec::new(),
        error: None,
    };
    let slot = Slot::new(1, &class, instance, latency, Arc::new(Inline));
    Gain {
        slot: Arc::new(slot),
        _dir: dir,
    }
}

impl Gain {
    fn live(&self) -> Option<Arc<LiveChain>> {
        LiveChain::of(&Chain::new(vec![self.slot.clone()], false))
    }
}

fn volume() -> Arc<AtomicU32> {
    Arc::new(AtomicU32::new(1.0f32.to_bits()))
}

/// Mono float audio whose samples identify their frame.
fn ramp(frames: usize, rate: u32) -> Arc<Audio> {
    let x: Vec<f64> = (0..frames)
        .map(|i| (i as f64 + 1.0) / (frames as f64 + 1.0))
        .collect();
    Arc::new(Audio::from_bytes(WavBuilder::new(Encoding::Float, 32, 1, rate).build(&x)).unwrap())
}

fn sample(audio: &Audio, frame: u64) -> f32 {
    let mut one = Vec::new();
    audio.read(frame, 1, &mut one);
    one[0]
}

fn play(
    audio: &Arc<Audio>,
    (start, end, from): (u64, u64, u64),
    looping: bool,
    fx: Option<Arc<LiveChain>>,
) -> Engine {
    let req = PlayRequest { start, end, from };
    let rate = audio.info().sample_rate;
    let map = ChannelMap::new(1, 1, false);
    let (engine, _feeder) = super::start(audio.clone(), req, looping, map, rate, volume(), fx);
    engine.wait_prefilled(Duration::from_secs(5));
    engine
}

/// A buffer as played: the file frame at its start, whether the first switch had been
/// carried out by then, and the samples.
struct Buffer {
    at: f64,
    switched: bool,
    samples: Vec<f32>,
}

/// Render at least `frames` frames, or until playback finishes, never faster than the
/// feeder (a device asks at the rate the audio plays, by which time the feeder is ahead).
fn play_for(engine: &mut Engine, frames: usize) -> Vec<Buffer> {
    let mut played = Vec::new();
    let mut done = 0;
    let deadline = Instant::now() + Duration::from_secs(20);
    while done < frames && !engine.shared().finished.load(Ordering::Acquire) {
        assert!(Instant::now() < deadline, "playback stalled");
        let wait = Instant::now() + Duration::from_millis(500);
        while engine.samples.slots() < 1_024
            && !engine.shared().feeder_done.load(Ordering::Acquire)
            && Instant::now() < wait
        {
            std::thread::sleep(Duration::from_micros(100));
        }
        let at = engine.shared().cursor().1;
        let switched = engine.shared().applied_gen.load(Ordering::Acquire) > 0;
        let mut samples = vec![0.0; 256];
        engine.render(&mut samples, Duration::ZERO);
        done += 256;
        played.push(Buffer {
            at,
            switched,
            samples,
        });
    }
    played
}

/// The frame heard `k` frames into a buffer starting at `at`, wrapping at the loop range.
fn frame_at(at: f64, k: usize, range: Option<(u64, u64)>) -> u64 {
    let f = at as u64 + k as u64;
    match range {
        Some((start, end)) if f >= end => start + (f - start) % (end - start),
        _ => f,
    }
}

/// Every sample of `buffers` is `audio` at its reported frame, times `gain`.
fn assert_heard(buffers: &[Buffer], audio: &Audio, gain: f32, range: Option<(u64, u64)>) {
    for b in buffers {
        for (k, v) in b.samples.iter().enumerate() {
            let f = frame_at(b.at, k, range);
            let want = sample(audio, f) * gain;
            assert_eq!(*v, want, "frame {f}: {v} instead of {want}");
        }
    }
}

#[test]
fn effects_are_heard_in_time_and_end_with_the_range() {
    let fx = gain(8_000, double());
    let audio = ramp(48_000, 8_000);
    let mut engine = play(&audio, (12_000, 24_000, 12_000), false, fx.live());
    let played = play_for(&mut engine, 100_000);
    assert!(engine.shared().finished.load(Ordering::Acquire));

    // Exactly the range is heard, doubled, from its first frame: the 64 frames of latency
    // are removed at the start and drained at the end.
    let heard: Vec<f32> = played
        .iter()
        .flat_map(|b| b.samples.iter().copied())
        .filter(|v| *v != 0.0)
        .collect();
    assert_eq!(heard.len(), 12_000);
    assert_eq!(heard[0], sample(&audio, 12_000) * 2.0);
    assert_eq!(heard[11_999], sample(&audio, 23_999) * 2.0);
    // The reported position of each buffer is the frame heard at its start.
    let before_end: Vec<_> = played.into_iter().take(12_000 / 256).collect();
    assert_heard(&before_end, &audio, 2.0, None);
    assert_eq!(engine.shared().position(), 24_000.0);
}

#[test]
fn looping_through_effects_stays_aligned_across_the_wrap() {
    let fx = gain(8_000, double());
    let audio = ramp(10_000, 8_000);
    let range = (2_000, 3_000);
    let mut engine = play(&audio, (range.0, range.1, 2_500), true, fx.live());
    let played = play_for(&mut engine, 5_000);
    assert_eq!(played[0].at, 2_500.0);
    assert!(played.windows(2).any(|w| w[1].at < w[0].at), "it wraps");
    assert_heard(&played, &audio, 2.0, Some(range));
    engine.shared().stop.store(true, Ordering::Relaxed);
}

#[test]
fn bypass_all_is_heard_dry_after_one_flush() {
    let fx = gain(8_000, double());
    let audio = ramp(480_000, 8_000);
    let range = (0, 480_000);
    let mut engine = play(&audio, (range.0, range.1, 0), false, fx.live());
    let before = play_for(&mut engine, 4_000);
    assert_heard(&before, &audio, 2.0, None);

    // Bypass All, as `Player::follow` hears it: nothing processes, and playback flushes.
    let bypassed = LiveChain::of(&Chain::new(vec![fx.slot.clone()], true));
    assert!(bypassed.is_none());
    engine.shared().set_fx(bypassed);
    engine.shared().retarget(Retarget {
        audio: audio.clone(),
        start: range.0,
        end: range.1,
    });
    let after = play_for(&mut engine, 8_000);
    let switched: Vec<_> = after.into_iter().filter(|b| b.switched).collect();
    // Past the buffer that crossfades from the processed audio, it is dry.
    assert!(switched.len() > 20);
    assert_heard(&switched[1..], &audio, 1.0, None);
    engine.shared().stop.store(true, Ordering::Relaxed);
}

#[test]
fn parameter_changes_are_heard_within_a_quarter_second() {
    const RATE: u32 = 48_000;
    let fx = gain(RATE, gain_norm(0.0));
    let audio = ramp(480_000, RATE);
    let mut engine = play(&audio, (0, 480_000, 0), true, fx.live());
    assert_heard(&play_for(&mut engine, 20_000), &audio, 1.0, None);

    fx.slot.instance().set_param(GAIN_PARAM, double());
    let mut heard_after = 0;
    loop {
        let played = play_for(&mut engine, 256);
        let b = &played[0];
        let f = frame_at(b.at, 0, None);
        if b.samples[0] == sample(&audio, f) * 2.0 {
            assert_heard(&played, &audio, 2.0, None);
            break;
        }
        heard_after += 256;
        assert!(
            heard_after <= RATE as usize / 4,
            "still not heard after {heard_after} frames"
        );
    }
    engine.shared().stop.store(true, Ordering::Relaxed);
}

/// Passes audio through, taking `delay` per call.
struct Slow {
    delay: Duration,
}

impl Stage for Slow {
    fn latency(&self) -> u32 {
        0
    }

    fn process(
        &self,
        input: &[&[f32]],
        output: &mut [&mut [f32]],
        frames: usize,
        _: Transport,
    ) -> bool {
        std::thread::sleep(self.delay);
        for (i, o) in input.iter().zip(output.iter_mut()) {
            o[..frames].copy_from_slice(&i[..frames]);
        }
        true
    }
}

#[test]
fn a_chain_too_slow_for_real_time_reports_one_overload() {
    // Each 1024-frame chunk (128 ms at 8 kHz) takes 300 ms to process.
    let slow = LiveChain::new(vec![Arc::new(Slow {
        delay: Duration::from_millis(300),
    }) as Arc<dyn Stage>]);
    let audio = ramp(480_000, 8_000);
    let mut engine = play(&audio, (0, 480_000, 0), false, Some(Arc::new(slow)));
    // Render in real time, 32 ms per buffer, for 2.5 s.
    let mut overloads = 0;
    let mut buf = vec![0.0; 256];
    for _ in 0..80 {
        std::thread::sleep(Duration::from_millis(32));
        engine.render(&mut buf, Duration::ZERO);
        overloads += engine.shared().take_overload() as usize;
    }
    assert_eq!(overloads, 1);
    engine.shared().stop.store(true, Ordering::Relaxed);
}

#[test]
fn a_dry_playback_never_reports_an_overload() {
    let audio = ramp(480_000, 8_000);
    let mut engine = play(&audio, (0, 480_000, 0), false, None);
    // Rendering far faster than the feeder underruns, but there are no effects to blame.
    let mut buf = vec![0.0; 8_192];
    for _ in 0..20 {
        engine.render(&mut buf, Duration::ZERO);
    }
    assert!(!engine.shared().take_overload());
    engine.shared().stop.store(true, Ordering::Relaxed);
}
