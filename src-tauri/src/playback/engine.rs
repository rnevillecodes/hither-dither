//! Device-independent playback core: a feeder thread fills a lock-free ring buffer with
//! mapped, resampled frames, and `Engine::render` (called from the audio callback, or from
//! tests in place of a device) drains it and publishes the audible position.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rtrb::{Consumer, Producer, RingBuffer};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

use super::mapping::ChannelMap;
use crate::edit::Audio;

/// Frames read from the file per feeder step.
const CHUNK_FRAMES: usize = 1024;
/// Ring buffer length in seconds of device audio.
const RING_SECS: f64 = 0.2;

/// What to play: from `from` up to `end`, wrapping to `start` while looping is on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayRequest {
    pub start: u64,
    pub end: u64,
    /// First frame played, within `[start, end]` (e.g. resuming mid-selection).
    pub from: u64,
}

/// State shared between the feeder, the audio callback and position reporting.
pub struct Shared {
    pub request: PlayRequest,
    pub file_rate: u32,
    pub looping: AtomicBool,
    /// Linear gain as `f32` bits.
    pub volume: Arc<AtomicU32>,
    /// Asks the feeder to exit.
    pub stop: AtomicBool,
    feeder_done: AtomicBool,
    /// All audio up to the range end has been handed to the device.
    pub finished: AtomicBool,
    /// File frame audible at `stamp_nanos`, as `f64` bits.
    audible: AtomicU64,
    stamp_nanos: AtomicU64,
    /// The callback is advancing (false before the first callback and after an underrun
    /// at the end).
    running: AtomicBool,
    base: Instant,
}

impl Shared {
    /// Best estimate of the file frame audible right now.
    pub fn position(&self) -> f64 {
        let audible = f64::from_bits(self.audible.load(Ordering::Acquire));
        let r = self.request;
        let mut pos = audible;
        if self.running.load(Ordering::Acquire) && !self.finished.load(Ordering::Acquire) {
            let stamp = Duration::from_nanos(self.stamp_nanos.load(Ordering::Acquire));
            let elapsed = self.base.elapsed().saturating_sub(stamp);
            // Interpolate between callbacks, but never by more than one typical buffer.
            pos += elapsed.as_secs_f64().min(0.1) * self.file_rate as f64;
        }
        if self.finished.load(Ordering::Acquire) {
            return r.end as f64;
        }
        pos.clamp(r.start as f64, r.end as f64)
    }
}

#[derive(Debug, Clone, Copy)]
struct Marker {
    /// Output frame index at which this segment starts.
    out_index: u64,
    /// File frame played at that index.
    file_frame: f64,
}

/// Consumer side, owned by the audio callback.
pub struct Engine {
    samples: Consumer<f32>,
    markers: Consumer<Marker>,
    shared: Arc<Shared>,
    out_ch: usize,
    out_index: u64,
    segment: Marker,
    /// File frames per output frame.
    step: f64,
}

impl Engine {
    /// Fill an interleaved device buffer. `latency` is the time until this buffer is heard.
    pub fn render(&mut self, out: &mut [f32], latency: Duration) {
        while let Ok(m) = self.markers.peek().copied() {
            if m.out_index > self.out_index {
                break;
            }
            self.segment = m;
            let _ = self.markers.pop();
        }
        let pos =
            self.segment.file_frame + (self.out_index - self.segment.out_index) as f64 * self.step;
        let audible = pos - latency.as_secs_f64() * self.shared.file_rate as f64;
        let sh = &self.shared;
        sh.audible.store(audible.to_bits(), Ordering::Release);
        sh.stamp_nanos
            .store(sh.base.elapsed().as_nanos() as u64, Ordering::Release);

        let gain = f32::from_bits(sh.volume.load(Ordering::Relaxed));
        let wanted = out.len() / self.out_ch * self.out_ch;
        let available = self.samples.slots() / self.out_ch * self.out_ch;
        let n = wanted.min(available);
        if n > 0 {
            let chunk = self.samples.read_chunk(n).expect("slots checked");
            let (a, b) = chunk.as_slices();
            for (dst, src) in out.iter_mut().zip(a.iter().chain(b)) {
                *dst = src * gain;
            }
            chunk.commit_all();
            self.out_index += (n / self.out_ch) as u64;
            sh.running.store(true, Ordering::Release);
        }
        out[n..].fill(0.0);
        if n < wanted && sh.feeder_done.load(Ordering::Acquire) && self.samples.is_empty() {
            sh.finished.store(true, Ordering::Release);
            sh.running.store(false, Ordering::Release);
        }
    }

    pub fn shared(&self) -> &Arc<Shared> {
        &self.shared
    }

    /// Wait (up to `timeout`) until the feeder has buffered half the ring or finished, so
    /// output does not start with an underrun.
    pub fn wait_prefilled(&self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        let target = self.samples.buffer().capacity() / 2;
        while self.samples.slots() < target
            && !self.shared.feeder_done.load(Ordering::Acquire)
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

/// Start a feeder for `req` and return the engine that consumes its output.
pub fn start(
    src: Arc<Audio>,
    req: PlayRequest,
    looping: bool,
    map: ChannelMap,
    out_rate: u32,
    volume: Arc<AtomicU32>,
) -> (Engine, JoinHandle<()>) {
    let end = req.end.min(src.frames());
    let start = req.start.min(end);
    let req = PlayRequest {
        start,
        end,
        from: req.from.clamp(start, end),
    };
    let file_rate = src.info().sample_rate;
    let out_ch = map.out_channels();
    let ring = ((out_rate as f64 * RING_SECS) as usize).max(CHUNK_FRAMES * 4) * out_ch;
    let (producer, consumer) = RingBuffer::<f32>::new(ring);
    let (marker_tx, marker_rx) = RingBuffer::<Marker>::new(64);
    let shared = Arc::new(Shared {
        request: req,
        file_rate,
        looping: AtomicBool::new(looping),
        volume,
        stop: AtomicBool::new(false),
        feeder_done: AtomicBool::new(false),
        finished: AtomicBool::new(false),
        audible: AtomicU64::new((req.from as f64).to_bits()),
        stamp_nanos: AtomicU64::new(0),
        running: AtomicBool::new(false),
        base: Instant::now(),
    });
    let engine = Engine {
        samples: consumer,
        markers: marker_rx,
        shared: shared.clone(),
        out_ch,
        out_index: 0,
        segment: Marker {
            out_index: 0,
            file_frame: req.from as f64,
        },
        step: file_rate as f64 / out_rate as f64,
    };
    let feeder = Feeder {
        src,
        shared,
        map,
        producer,
        markers: marker_tx,
        resampler: (file_rate != out_rate).then(|| Resample::new(file_rate, out_rate, out_ch)),
    };
    let handle = std::thread::Builder::new()
        .name("playback-feeder".into())
        .spawn(move || feeder.run())
        .expect("spawn feeder thread");
    (engine, handle)
}

struct Feeder {
    src: Arc<Audio>,
    shared: Arc<Shared>,
    map: ChannelMap,
    producer: Producer<f32>,
    markers: Producer<Marker>,
    resampler: Option<Resample>,
}

impl Feeder {
    fn run(mut self) {
        let req = self.shared.request;
        let out_ch = self.map.out_channels();
        let ratio = self.resampler.as_ref().map_or(1.0, |r| r.ratio);
        let (mut raw, mut mapped, mut out) = (Vec::new(), Vec::new(), Vec::new());
        let mut pos = req.from;
        // Input frames fed so far; marks where loop segments start in the output.
        let mut fed: u64 = 0;

        while !self.shared.stop.load(Ordering::Relaxed) {
            if pos >= req.end {
                let looping = self.shared.looping.load(Ordering::Relaxed);
                if !looping || req.end <= req.start {
                    break;
                }
                pos = req.start;
                let marker = Marker {
                    out_index: (fed as f64 * ratio).round() as u64,
                    file_frame: req.start as f64,
                };
                while self.markers.push(marker).is_err() {
                    if self.shared.stop.load(Ordering::Relaxed) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
            }
            let n = (req.end - pos).min(CHUNK_FRAMES as u64) as usize;
            self.src.read(pos, n, &mut raw);
            mapped.clear();
            self.map.apply(&raw, &mut mapped);
            pos += n as u64;
            fed += n as u64;

            out.clear();
            match &mut self.resampler {
                Some(r) => r.push(&mapped, &mut out),
                None => out.extend_from_slice(&mapped),
            }
            if !self.write(&out, out_ch) {
                return;
            }
        }

        if let Some(r) = &mut self.resampler
            && !self.shared.stop.load(Ordering::Relaxed)
        {
            out.clear();
            r.finish(fed, &mut out);
            if !self.write(&out, out_ch) {
                return;
            }
        }
        self.shared.feeder_done.store(true, Ordering::Release);
    }

    /// Push all of `data`, waiting for space. Returns false if asked to stop.
    fn write(&mut self, mut data: &[f32], out_ch: usize) -> bool {
        while !data.is_empty() {
            if self.shared.stop.load(Ordering::Relaxed) {
                return false;
            }
            let free = self.producer.slots() / out_ch * out_ch;
            if free == 0 {
                std::thread::sleep(Duration::from_millis(2));
                continue;
            }
            let n = free.min(data.len());
            let chunk = self.producer.write_chunk_uninit(n).expect("slots checked");
            let written = chunk.fill_from_iter(data[..n].iter().copied());
            data = &data[written..];
        }
        true
    }
}

/// Streaming sample-rate converter. `SincFixedIn` output is already time-aligned with its
/// input (an impulse at input frame n appears at output frame n·ratio), so nothing is trimmed.
pub struct Resample {
    inner: SincFixedIn<f32>,
    /// Output frames per input frame.
    pub ratio: f64,
    channels: usize,
    /// Planar input waiting for a full chunk.
    pending: Vec<Vec<f32>>,
    emitted: u64,
}

impl Resample {
    pub fn new(in_rate: u32, out_rate: u32, channels: usize) -> Self {
        let ratio = out_rate as f64 / in_rate as f64;
        let params = SincInterpolationParameters {
            sinc_len: 128,
            f_cutoff: 0.95,
            oversampling_factor: 128,
            interpolation: SincInterpolationType::Linear,
            window: WindowFunction::BlackmanHarris2,
        };
        let inner = SincFixedIn::new(ratio, 1.0, params, CHUNK_FRAMES, channels)
            .expect("valid resampler parameters");
        Self {
            inner,
            ratio,
            channels,
            pending: vec![Vec::new(); channels],
            emitted: 0,
        }
    }

    /// Feed interleaved frames; append any finished interleaved output to `out`.
    pub fn push(&mut self, interleaved: &[f32], out: &mut Vec<f32>) {
        for frame in interleaved.chunks_exact(self.channels) {
            for (c, v) in frame.iter().enumerate() {
                self.pending[c].push(*v);
            }
        }
        while self.pending[0].len() >= self.inner.input_frames_next() {
            let need = self.inner.input_frames_next();
            let input: Vec<&[f32]> = self.pending.iter().map(|c| &c[..need]).collect();
            let res = self.inner.process(&input, None).expect("resample");
            for c in &mut self.pending {
                c.drain(..need);
            }
            self.emit(&res, out, usize::MAX);
        }
    }

    /// Flush remaining input so total output is `fed_frames * ratio` frames.
    pub fn finish(&mut self, fed_frames: u64, out: &mut Vec<f32>) {
        let total = (fed_frames as f64 * self.ratio).round() as u64;
        let mut first = true;
        while self.emitted < total {
            let res = if first && !self.pending[0].is_empty() {
                let input: Vec<&[f32]> = self.pending.iter().map(Vec::as_slice).collect();
                self.inner.process_partial(Some(&input), None)
            } else {
                self.inner.process_partial::<&[f32]>(None, None)
            }
            .expect("resample");
            first = false;
            let limit = (total - self.emitted) as usize;
            self.emit(&res, out, limit);
        }
        for c in &mut self.pending {
            c.clear();
        }
    }

    fn emit(&mut self, planar: &[Vec<f32>], out: &mut Vec<f32>, limit: usize) {
        let end = planar[0].len().min(limit);
        for i in 0..end {
            out.extend(planar.iter().map(|c| c[i]));
        }
        self.emitted += end as u64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wav::synth::{Encoding, WavBuilder, sine};

    fn volume() -> Arc<AtomicU32> {
        Arc::new(AtomicU32::new(1.0f32.to_bits()))
    }

    /// Resample a mono signal in one go and return the output.
    fn resample(input: &[f32], in_rate: u32, out_rate: u32) -> Vec<f32> {
        let mut r = Resample::new(in_rate, out_rate, 1);
        let mut out = Vec::new();
        for chunk in input.chunks(777) {
            r.push(chunk, &mut out);
        }
        r.finish(input.len() as u64, &mut out);
        out
    }

    /// Frequency estimated from rising zero crossings.
    fn frequency(x: &[f32], rate: u32) -> f64 {
        let crossings: Vec<usize> = (1..x.len())
            .filter(|&i| x[i - 1] < 0.0 && x[i] >= 0.0)
            .collect();
        let span = (crossings.last().unwrap() - crossings[0]) as f64;
        (crossings.len() - 1) as f64 * rate as f64 / span
    }

    #[test]
    fn resampler_keeps_length_and_pitch() {
        let input: Vec<f32> = sine(1000.0, 0.5, 96_000, 96_000)
            .iter()
            .map(|&v| v as f32)
            .collect();
        let out = resample(&input, 96_000, 48_000);
        assert_eq!(out.len(), 48_000);
        assert!((frequency(&out[1000..47_000], 48_000) - 1000.0).abs() < 0.5);
        // Delay is trimmed: the output starts in phase with the input (sin(0) = 0, rising).
        assert!(out[0].abs() < 0.05 && out[3] > 0.0, "{:?}", &out[..4]);

        let up = resample(&input[..44_100], 44_100, 48_000);
        assert_eq!(up.len(), 48_000);
    }

    fn render_until_finished(
        engine: &mut Engine,
        out_ch: usize,
        max_frames: usize,
    ) -> (Vec<f32>, Vec<f64>) {
        let mut all = Vec::new();
        let mut positions = Vec::new();
        let mut buf = vec![0.0; 256 * out_ch];
        let deadline = Instant::now() + Duration::from_secs(10);
        engine.wait_prefilled(Duration::from_secs(2));
        while all.len() < max_frames * out_ch && !engine.shared().finished.load(Ordering::Acquire) {
            // Give the feeder time to fill, like a real device would.
            std::thread::sleep(Duration::from_micros(200));
            engine.render(&mut buf, Duration::ZERO);
            positions.push(engine.shared().position());
            all.extend_from_slice(&buf);
            assert!(Instant::now() < deadline, "playback never finished");
        }
        (all, positions)
    }

    fn ramp_source(frames: usize, rate: u32) -> Arc<Audio> {
        // Distinct value per frame so output can be matched to file positions.
        let x: Vec<f64> = (0..frames)
            .map(|i| (i as f64 + 1.0) / (frames as f64 + 1.0))
            .collect();
        Arc::new(
            Audio::from_bytes(WavBuilder::new(Encoding::Float, 32, 1, rate).build(&x)).unwrap(),
        )
    }

    #[test]
    fn selection_playback_stops_at_range_end() {
        let src = ramp_source(48_000, 48_000);
        let req = PlayRequest {
            start: 12_000,
            end: 24_000,
            from: 12_000,
        };
        let (mut engine, feeder) = start(
            src,
            req,
            false,
            ChannelMap::new(1, 2, false),
            48_000,
            volume(),
        );
        let (out, positions) = render_until_finished(&mut engine, 2, 200_000);
        feeder.join().unwrap();
        assert!(engine.shared().finished.load(Ordering::Acquire));
        let left: Vec<f32> = out
            .iter()
            .step_by(2)
            .copied()
            .filter(|v| *v != 0.0)
            .collect();
        assert_eq!(left.len(), 12_000, "exactly the selected frames are played");
        assert!((left[0] - 12_001.0 / 48_001.0).abs() < 1e-6);
        assert!((left[11_999] - 24_000.0 / 48_001.0).abs() < 1e-6);
        assert!(positions.windows(2).all(|w| w[1] >= w[0]));
        assert_eq!(engine.shared().position(), 24_000.0);
    }

    #[test]
    fn loop_wraps_until_stopped() {
        let src = ramp_source(10_000, 8_000);
        let req = PlayRequest {
            start: 2_000,
            end: 3_000,
            from: 2_000,
        };
        let (mut engine, feeder) = start(
            src,
            req,
            true,
            ChannelMap::new(1, 1, false),
            8_000,
            volume(),
        );
        let (out, positions) = render_until_finished(&mut engine, 1, 3_500);
        assert!(
            !engine.shared().finished.load(Ordering::Acquire),
            "looping never finishes"
        );
        // Output repeats the range: frame 1000 equals frame 0.
        assert_eq!(out[1_000], out[0]);
        assert_eq!(out[2_000], out[0]);
        assert!((out[0] - 2_001.0 / 10_001.0).abs() < 1e-6);
        assert!(
            positions.windows(2).any(|w| w[1] < w[0]),
            "position wraps back"
        );
        assert!(positions.iter().all(|p| (2_000.0..=3_000.0).contains(p)));
        engine.shared().stop.store(true, Ordering::Relaxed);
        feeder.join().unwrap();
    }

    #[test]
    fn resumes_mid_range_and_loops_to_range_start() {
        let src = ramp_source(10_000, 8_000);
        let req = PlayRequest {
            start: 2_000,
            end: 3_000,
            from: 2_600,
        };
        let (mut engine, feeder) = start(
            src,
            req,
            true,
            ChannelMap::new(1, 1, false),
            8_000,
            volume(),
        );
        let (out, _) = render_until_finished(&mut engine, 1, 1_500);
        assert!((out[0] - 2_601.0 / 10_001.0).abs() < 1e-6);
        // After the 400 frames to the end, it wraps to the range start, not to `from`.
        assert!((out[400] - 2_001.0 / 10_001.0).abs() < 1e-6);
        engine.shared().stop.store(true, Ordering::Relaxed);
        feeder.join().unwrap();
    }

    #[test]
    fn volume_scales_output_only() {
        let src = ramp_source(4_000, 8_000);
        let vol = Arc::new(AtomicU32::new(0.5f32.to_bits()));
        let req = PlayRequest {
            start: 0,
            end: 4_000,
            from: 0,
        };
        let (mut engine, _f) = start(src, req, false, ChannelMap::new(1, 1, false), 8_000, vol);
        let (out, _) = render_until_finished(&mut engine, 1, 10_000);
        assert!((out[0] - 0.5 / 4_001.0).abs() < 1e-7);
    }

    #[test]
    fn resampled_playback_covers_range_at_device_rate() {
        let src = ramp_source(96_000, 96_000);
        let req = PlayRequest {
            start: 0,
            end: 96_000,
            from: 0,
        };
        let (mut engine, feeder) = start(
            src,
            req,
            false,
            ChannelMap::new(1, 1, false),
            48_000,
            volume(),
        );
        let (out, positions) = render_until_finished(&mut engine, 1, 200_000);
        feeder.join().unwrap();
        let played = out.iter().filter(|v| **v != 0.0).count();
        assert!((played as i64 - 48_000).abs() <= 2, "{played}");
        // Position is in file frames (96 kHz) even though the device runs at 48 kHz.
        let mid = positions[positions.len() / 2];
        assert!(mid > 30_000.0 && mid < 66_000.0, "{mid}");
    }
}
