//! Device-independent playback core: a feeder thread fills a lock-free ring buffer with
//! mapped, resampled frames, and `Engine::render` (called from the audio callback, or from
//! tests in place of a device) drains it and publishes the audible position.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering, fence};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rtrb::{Consumer, Producer, RingBuffer};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

use super::mapping::ChannelMap;
use crate::edit::{Audio, EditList, Located};
use crate::fx::live::{Live, LiveChain};

/// Frames read from the file per feeder step.
const CHUNK_FRAMES: usize = 1024;
/// Ring buffer length in seconds of device audio.
const RING_SECS: f64 = 0.2;
/// How much of the ring the feeder fills while effects process, in seconds, so parameter
/// changes are heard within 0.25 s (this, plus a chunk, plus the device buffer).
const FX_RING_SECS: f64 = 0.15;
/// Underruns within `OVERLOAD_WINDOW` that mean the effects can't keep up.
const OVERLOAD_UNDERRUNS: usize = 3;
const OVERLOAD_WINDOW: Duration = Duration::from_secs(2);

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
    /// Play range of the audio being heard.
    range_start: AtomicU64,
    range_end: AtomicU64,
    /// A newer version of the audio to switch to, taken by the feeder.
    retarget: Mutex<Option<Retarget>>,
    /// Published by the feeder when it switches: queued output before `flush_to` is stale.
    /// The new audio starts at the frame that the engine played at output `flush_anchor`.
    flush_gen: AtomicU32,
    flush_to: AtomicU64,
    flush_anchor: AtomicU64,
    flush_start: AtomicU64,
    flush_end: AtomicU64,
    /// The last switch the engine has carried out.
    applied_gen: AtomicU32,
    /// The engine's next output index and the file frame it plays (a seqlock: `cursor_seq`
    /// is odd while they are written).
    cursor_seq: AtomicU64,
    cursor_out: AtomicU64,
    cursor_frame: AtomicU64,
    /// The effects to run the audio through, taken by the feeder at the start and on each
    /// switch.
    fx: Mutex<Option<Arc<LiveChain>>>,
    /// Interleaved samples the feeder may have queued: less than the ring holds while
    /// effects process.
    fill_limit: AtomicUsize,
    /// Effects are processing the audio being queued.
    fx_active: AtomicBool,
    /// The effects couldn't keep up with real time (reported once per playback).
    overload: AtomicBool,
}

/// Switch playback to `audio` (a new edit state of the same source), playing `[start, end)`.
pub struct Retarget {
    pub audio: Arc<Audio>,
    pub start: u64,
    pub end: u64,
}

impl Shared {
    fn new(file_rate: u32, req: PlayRequest, looping: bool, volume: Arc<AtomicU32>) -> Self {
        Self {
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
            range_start: AtomicU64::new(req.start),
            range_end: AtomicU64::new(req.end),
            retarget: Mutex::new(None),
            flush_gen: AtomicU32::new(0),
            flush_to: AtomicU64::new(0),
            flush_anchor: AtomicU64::new(0),
            flush_start: AtomicU64::new(req.start),
            flush_end: AtomicU64::new(req.end),
            applied_gen: AtomicU32::new(0),
            cursor_seq: AtomicU64::new(0),
            cursor_out: AtomicU64::new(0),
            cursor_frame: AtomicU64::new((req.from as f64).to_bits()),
            fx: Mutex::new(None),
            fill_limit: AtomicUsize::new(usize::MAX),
            fx_active: AtomicBool::new(false),
            overload: AtomicBool::new(false),
        }
    }

    /// Best estimate of the file frame audible right now.
    pub fn position(&self) -> f64 {
        let audible = f64::from_bits(self.audible.load(Ordering::Acquire));
        let (start, end) = self.range();
        let mut pos = audible;
        if self.running.load(Ordering::Acquire) && !self.finished.load(Ordering::Acquire) {
            let stamp = Duration::from_nanos(self.stamp_nanos.load(Ordering::Acquire));
            let elapsed = self.base.elapsed().saturating_sub(stamp);
            // Interpolate between callbacks, but never by more than one typical buffer.
            pos += elapsed.as_secs_f64().min(0.1) * self.file_rate as f64;
        }
        if self.finished.load(Ordering::Acquire) {
            return end as f64;
        }
        pos.clamp(start as f64, end as f64)
    }

    /// The play range of the audio being heard.
    pub fn range(&self) -> (u64, u64) {
        (
            self.range_start.load(Ordering::Acquire),
            self.range_end.load(Ordering::Acquire),
        )
    }

    /// Switch to new audio as soon as possible (replacing a switch not yet made).
    pub fn retarget(&self, to: Retarget) {
        *self.retarget.lock().unwrap() = Some(to);
    }

    /// The effects the feeder uses from its next switch on.
    pub fn fx(&self) -> Option<Arc<LiveChain>> {
        self.fx.lock().unwrap().clone()
    }

    /// Run the audio through `fx` from the next switch on. Follow with a [`Retarget`] to
    /// hear the change at once.
    pub fn set_fx(&self, fx: Option<Arc<LiveChain>>) {
        *self.fx.lock().unwrap() = fx;
    }

    /// Whether the effects turned out too slow for real time since this was last asked.
    /// True at most once per playback.
    pub fn take_overload(&self) -> bool {
        self.overload.swap(false, Ordering::AcqRel)
    }

    /// Interleaved samples the ring may hold, given its capacity.
    fn fill_limit(&self, capacity: usize) -> usize {
        self.fill_limit.load(Ordering::Relaxed).min(capacity)
    }

    fn publish_cursor(&self, out_index: u64, frame: f64) {
        let seq = self.cursor_seq.load(Ordering::Relaxed);
        self.cursor_seq.store(seq + 1, Ordering::Relaxed);
        fence(Ordering::Release);
        self.cursor_out.store(out_index, Ordering::Relaxed);
        self.cursor_frame.store(frame.to_bits(), Ordering::Relaxed);
        self.cursor_seq.store(seq + 2, Ordering::Release);
    }

    /// The engine's next output index and the file frame it plays there.
    fn cursor(&self) -> (u64, f64) {
        loop {
            let before = self.cursor_seq.load(Ordering::Acquire);
            let out = self.cursor_out.load(Ordering::Relaxed);
            let frame = f64::from_bits(self.cursor_frame.load(Ordering::Relaxed));
            fence(Ordering::Acquire);
            if before.is_multiple_of(2) && self.cursor_seq.load(Ordering::Relaxed) == before {
                return (out, frame);
            }
            std::hint::spin_loop();
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Marker {
    /// Switch this marker belongs to; older ones are dropped with the stale audio.
    generation: u32,
    /// Output frame index at which this segment starts.
    out_index: u64,
    /// File frame played at that index.
    file_frame: f64,
}

/// Length of the crossfade from stale to new audio when playback switches.
const DECLICK_SECS: f64 = 0.005;

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
    generation: u32,
    /// A switch the feeder published, waiting for enough of the new audio to arrive.
    pending: Option<Switch>,
    /// Audio queued before a switch, moved out of the ring so the feeder has room for the
    /// new audio. It plays on until the new audio is ready, then fades out under it.
    stale: Vec<f32>,
    stale_len: usize,
    stale_pos: usize,
    /// Stale frame at which the switch crossfade starts.
    fade_from: usize,
    /// Frames of the switch crossfade, and how far it has got.
    declick: usize,
    declick_pos: usize,
    /// When the latest underruns mid-range happened, oldest first.
    underruns: [Option<Duration>; OVERLOAD_UNDERRUNS],
    /// An overload has been reported for this playback.
    overloaded: bool,
}

#[derive(Debug, Clone, Copy)]
struct Switch {
    generation: u32,
    to: u64,
    anchor: u64,
    start: u64,
    end: u64,
    /// New audio already played as if it were old (the switch was published during a
    /// callback that then read past `to`).
    new_played: u64,
}

impl Engine {
    fn new(
        samples: Consumer<f32>,
        markers: Consumer<Marker>,
        shared: Arc<Shared>,
        out_ch: usize,
        out_rate: u32,
        from: u64,
    ) -> Self {
        let declick = ((out_rate as f64 * DECLICK_SECS) as usize).max(1);
        let capacity = samples.buffer().capacity();
        Self {
            samples,
            markers,
            step: shared.file_rate as f64 / out_rate as f64,
            shared,
            out_ch,
            out_index: 0,
            segment: Marker {
                generation: 0,
                out_index: 0,
                file_frame: from as f64,
            },
            generation: 0,
            pending: None,
            stale: vec![0.0; capacity],
            stale_len: 0,
            stale_pos: 0,
            fade_from: 0,
            declick,
            // No crossfade at the very start.
            declick_pos: declick,
            underruns: [None; OVERLOAD_UNDERRUNS],
            overloaded: false,
        }
    }

    /// Fill an interleaved device buffer. `latency` is the time until this buffer is heard.
    pub fn render(&mut self, out: &mut [f32], latency: Duration) {
        let ch = self.out_ch;
        let wanted = out.len() / ch;
        let was_running = self.shared.running.load(Ordering::Acquire);
        self.take_switch();
        let switched = self.complete_switch(wanted);
        let pos = self.frame_at_cursor();
        let sh = &self.shared;
        let audible = pos - latency.as_secs_f64() * sh.file_rate as f64;
        sh.audible.store(audible.to_bits(), Ordering::Release);
        sh.stamp_nanos
            .store(sh.base.elapsed().as_nanos() as u64, Ordering::Release);
        let gain = f32::from_bits(sh.volume.load(Ordering::Relaxed));

        let n;
        if self.pending.is_some() {
            // Waiting for the new audio: play on from the stale audio set aside. Time moves
            // on even if that runs out, so the new audio is skipped to match.
            n = wanted.min(self.stale_len - self.stale_pos);
            let from = &self.stale[self.stale_pos * ch..(self.stale_pos + n) * ch];
            for (dst, src) in out.iter_mut().zip(from) {
                *dst = src * gain;
            }
            self.stale_pos += n;
            self.out_index += wanted as u64;
            sh.running.store(true, Ordering::Release);
        } else {
            n = wanted.min(self.samples.slots() / ch);
            if n > 0 {
                let chunk = self.samples.read_chunk(n * ch).expect("slots checked");
                let (a, b) = chunk.as_slices();
                for (i, (dst, src)) in out.iter_mut().zip(a.iter().chain(b)).enumerate() {
                    let f = self.declick_pos + i / ch;
                    *dst = if f < self.declick {
                        // Crossfade from the stale audio (if any is left) to the new.
                        let g = f as f32 / self.declick as f32;
                        let k = self.fade_from + f;
                        let old = if k < self.stale_len {
                            self.stale[k * ch + i % ch]
                        } else {
                            0.0
                        };
                        (src * g + old * (1.0 - g)) * gain
                    } else {
                        src * gain
                    };
                }
                chunk.commit_all();
                self.out_index += n as u64;
                self.declick_pos = (self.declick_pos + n).min(self.declick);
                sh.running.store(true, Ordering::Release);
            }
        }
        out[n * ch..].fill(0.0);
        let sh = &self.shared;
        let underrun = n < wanted
            && was_running
            && self.pending.is_none()
            && !sh.feeder_done.load(Ordering::Acquire)
            && sh.fx_active.load(Ordering::Acquire);
        if underrun {
            self.note_underrun();
        }

        // Where the next buffer starts, for the feeder to continue from after a switch.
        let next = self.frame_at_cursor();
        let sh = &self.shared;
        sh.publish_cursor(self.out_index, next);
        if switched {
            // Only now is the cursor in the new timeline, ready for the next switch.
            sh.applied_gen.store(self.generation, Ordering::Release);
        }
        if n < wanted
            && self.pending.is_none()
            && sh.feeder_done.load(Ordering::Acquire)
            && self.samples.is_empty()
        {
            sh.finished.store(true, Ordering::Release);
            sh.running.store(false, Ordering::Release);
        }
    }

    /// Count an underrun while effects are processing; report an overload (once) when there
    /// have been several in a short time.
    fn note_underrun(&mut self) {
        let now = self.shared.base.elapsed();
        self.underruns.rotate_left(1);
        self.underruns[OVERLOAD_UNDERRUNS - 1] = Some(now);
        let burst = self.underruns[0].is_some_and(|t| now - t <= OVERLOAD_WINDOW);
        if burst && !self.overloaded {
            self.overloaded = true;
            self.shared.overload.store(true, Ordering::Release);
        }
    }

    /// File frame at the current output index, after taking any markers that start by it.
    fn frame_at_cursor(&mut self) -> f64 {
        while let Ok(m) = self.markers.peek().copied() {
            if m.generation < self.generation {
                let _ = self.markers.pop();
                continue;
            }
            if m.generation > self.generation || m.out_index > self.out_index {
                break;
            }
            self.segment = m;
            let _ = self.markers.pop();
        }
        self.segment.file_frame + (self.out_index - self.segment.out_index) as f64 * self.step
    }

    /// If the feeder published a switch, move the stale audio queued before it out of the
    /// ring, making room for the new audio.
    fn take_switch(&mut self) {
        let sh = &self.shared;
        let generation = sh.flush_gen.load(Ordering::Acquire);
        if self.pending.is_some() || generation == self.generation {
            return;
        }
        let ch = self.out_ch;
        let to = sh.flush_to.load(Ordering::Relaxed);
        // Everything before `to` has been queued, so it is all in the ring.
        let stale = (to.saturating_sub(self.out_index) as usize).min(self.samples.slots() / ch);
        let chunk = self.samples.read_chunk(stale * ch).expect("slots checked");
        let (a, b) = chunk.as_slices();
        self.stale[..a.len()].copy_from_slice(a);
        self.stale[a.len()..a.len() + b.len()].copy_from_slice(b);
        chunk.commit_all();
        self.stale_len = stale;
        self.stale_pos = 0;
        self.pending = Some(Switch {
            generation,
            to,
            anchor: sh.flush_anchor.load(Ordering::Relaxed),
            start: sh.flush_start.load(Ordering::Relaxed),
            end: sh.flush_end.load(Ordering::Relaxed),
            new_played: self.out_index.saturating_sub(to),
        });
    }

    /// Switch to the new audio once enough of it is queued for this buffer (or all of it
    /// is). It starts at the frame the engine played at output `anchor`, and time has
    /// moved on since then, so that much of it is skipped.
    fn complete_switch(&mut self, wanted: usize) -> bool {
        let Some(sw) = self.pending else {
            return false;
        };
        let ch = self.out_ch;
        let played = self.out_index.saturating_sub(sw.anchor);
        let skip = played.saturating_sub(sw.new_played) as usize;
        let have = self.samples.slots() / ch;
        let all_queued = self.shared.feeder_done.load(Ordering::Acquire);
        // (A full ring can't take more: then go with what there is.)
        let full = have >= self.shared.fill_limit(self.samples.buffer().capacity()) / ch;
        if have < skip + wanted && !all_queued && !full {
            return false;
        }
        let dropped = skip.min(have);
        self.samples
            .read_chunk(dropped * ch)
            .expect("slots checked")
            .commit_all();
        self.out_index = sw.to + sw.new_played + dropped as u64;
        self.generation = sw.generation;
        self.pending = None;
        self.fade_from = self.stale_pos;
        self.declick_pos = 0;
        let sh = &self.shared;
        sh.range_start.store(sw.start, Ordering::Release);
        sh.range_end.store(sw.end, Ordering::Release);
        true
    }

    pub fn shared(&self) -> &Arc<Shared> {
        &self.shared
    }

    /// Wait (up to `timeout`) until the feeder has buffered half the ring or finished, so
    /// output does not start with an underrun.
    pub fn wait_prefilled(&self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        let target = self.shared.fill_limit(self.samples.buffer().capacity()) / 2;
        while self.samples.slots() < target
            && !self.shared.feeder_done.load(Ordering::Acquire)
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

/// Start a feeder for `req`, processing the audio through `fx`, and return the engine that
/// consumes its output.
pub fn start(
    src: Arc<Audio>,
    req: PlayRequest,
    looping: bool,
    map: ChannelMap,
    out_rate: u32,
    volume: Arc<AtomicU32>,
    fx: Option<Arc<LiveChain>>,
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
    let fx_limit = ((out_rate as f64 * FX_RING_SECS) as usize * out_ch).min(ring);
    let shared = Arc::new(Shared::new(file_rate, req, looping, volume));
    let limit = if fx.is_some() { fx_limit } else { ring };
    shared.fill_limit.store(limit, Ordering::Relaxed);
    shared.set_fx(fx);
    let engine = Engine::new(
        consumer,
        marker_rx,
        shared.clone(),
        out_ch,
        out_rate,
        req.from,
    );
    let feeder = Feeder {
        src,
        req,
        shared,
        map,
        producer,
        markers: marker_tx,
        resampler: (file_rate != out_rate).then(|| Resample::new(file_rate, out_rate, out_ch)),
        written: 0,
        live: None,
        capacity: ring,
        fx_limit,
    };
    let handle = std::thread::Builder::new()
        .name("playback-feeder".into())
        .spawn(move || feeder.run())
        .expect("spawn feeder thread");
    (engine, handle)
}

/// Where file frame `frame` of the `old` edit state is heard in `new`: the same source
/// audio if it is still there, else the audio that followed it (`None` if none is heard).
pub fn map_frame(old: &EditList, new: &EditList, frame: f64) -> Option<f64> {
    if std::ptr::eq(old, new) || old == new {
        return Some(frame);
    }
    let whole = frame.max(0.0).floor();
    let src = old.source_at(whole as u64)?;
    match new.output_of(src) {
        Located::At(out) => Some(out as f64 + (frame - whole).max(0.0)),
        Located::Removed { next } => next.map(|n| n as f64),
    }
}

struct Feeder {
    src: Arc<Audio>,
    req: PlayRequest,
    shared: Arc<Shared>,
    map: ChannelMap,
    producer: Producer<f32>,
    markers: Producer<Marker>,
    resampler: Option<Resample>,
    /// Output frames queued so far.
    written: u64,
    /// The effects the audio is processed through.
    live: Option<Live>,
    /// Ring capacity, and the part of it filled while effects process, in samples.
    capacity: usize,
    fx_limit: usize,
}

impl Feeder {
    fn run(mut self) {
        let out_ch = self.map.out_channels();
        let ratio = self.resampler.as_ref().map_or(1.0, |r| r.ratio);
        let (mut raw, mut mapped, mut out) = (Vec::new(), Vec::new(), Vec::new());
        let mut processed = Vec::new();
        self.take_fx();
        let mut pos = self.req.from;
        // Input frames fed since `base_out`; marks where loop segments start in the output.
        let mut fed: u64 = 0;
        let mut base_out: u64 = 0;
        let mut generation = 0u32;
        // Everything up to the range end is queued; wait for a switch or the end.
        let mut drained = false;

        while !self.shared.stop.load(Ordering::Relaxed) {
            if let Some(to) = self.take_retarget(generation) {
                // Continue from the frame the engine is playing now, in the new audio.
                let (anchor, frame) = self.shared.cursor();
                let from = map_frame(self.src.edits(), to.audio.edits(), frame)
                    .map_or(to.end, |f| f as u64)
                    .clamp(to.start, to.end.max(to.start));
                self.src = to.audio;
                self.req = PlayRequest {
                    start: to.start,
                    end: to.end,
                    from,
                };
                pos = from;
                fed = 0;
                base_out = self.written;
                if let Some(r) = &mut self.resampler {
                    r.reset();
                }
                self.take_fx();
                generation += 1;
                let marker = Marker {
                    generation,
                    out_index: self.written,
                    file_frame: from as f64,
                };
                if !self.push_marker(marker) {
                    return;
                }
                let sh = &self.shared;
                sh.feeder_done.store(false, Ordering::Release);
                drained = false;
                sh.flush_to.store(self.written, Ordering::Relaxed);
                sh.flush_anchor.store(anchor, Ordering::Relaxed);
                sh.flush_start.store(to.start, Ordering::Relaxed);
                sh.flush_end.store(to.end, Ordering::Relaxed);
                sh.flush_gen.store(generation, Ordering::Release);
            }
            if drained {
                if self.shared.finished.load(Ordering::Acquire) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(2));
                continue;
            }
            let req = self.req;
            if pos >= req.end {
                let looping = self.shared.looping.load(Ordering::Relaxed);
                if !looping || req.end <= req.start {
                    // Drain the effects, so the end of the range is heard.
                    if let Some(live) = &mut self.live {
                        processed.clear();
                        live.drain(pos, &mut processed);
                        if !self.output(&processed, &mut mapped, &mut out, out_ch) {
                            return;
                        }
                    }
                    if let Some(r) = &mut self.resampler {
                        out.clear();
                        r.finish(fed, &mut out);
                        if !self.write(&out, out_ch) {
                            return;
                        }
                    }
                    self.shared.feeder_done.store(true, Ordering::Release);
                    drained = true;
                    continue;
                }
                pos = req.start;
                let marker = Marker {
                    generation,
                    out_index: base_out + (fed as f64 * ratio).round() as u64,
                    file_frame: req.start as f64,
                };
                if !self.push_marker(marker) {
                    return;
                }
            }
            let n = (req.end - pos).min(CHUNK_FRAMES as u64) as usize;
            self.src.read(pos, n, &mut raw);
            // Effects run at the file's rate, on its channels, before mapping and resampling.
            let input = match &mut self.live {
                Some(live) => {
                    processed.clear();
                    live.process(&raw, pos, &mut processed);
                    &processed
                }
                None => &raw,
            };
            pos += n as u64;
            fed += n as u64;
            if !self.output(input, &mut mapped, &mut out, out_ch) {
                return;
            }
        }
    }

    /// Map, resample and queue `frames` (interleaved, in the file's channels). Returns false
    /// if asked to stop.
    fn output(
        &mut self,
        frames: &[f32],
        mapped: &mut Vec<f32>,
        out: &mut Vec<f32>,
        out_ch: usize,
    ) -> bool {
        mapped.clear();
        self.map.apply(frames, mapped);
        out.clear();
        match &mut self.resampler {
            Some(r) => r.push(mapped, out),
            None => out.extend_from_slice(mapped),
        }
        self.write(out, out_ch)
    }

    /// Take the effects to use from now on. The input is starting or jumping, so the chain's
    /// first `latency` frames of output belong to the audio before and are dropped.
    fn take_fx(&mut self) {
        let fx = self.shared.fx();
        let same = LiveChain::same(self.live.as_ref().map(Live::chain), fx.as_ref());
        match &mut self.live {
            Some(live) if same => live.restart(),
            _ => self.live = fx.map(|c| Live::new(c, self.src.channels())),
        }
        let active = self.live.is_some();
        let limit = if active { self.fx_limit } else { self.capacity };
        self.shared.fill_limit.store(limit, Ordering::Relaxed);
        self.shared.fx_active.store(active, Ordering::Release);
    }

    /// The pending switch, once the engine has carried out the previous one (so its
    /// cursor is in the timeline of the audio being fed).
    fn take_retarget(&self, generation: u32) -> Option<Retarget> {
        if self.shared.applied_gen.load(Ordering::Acquire) != generation {
            return None;
        }
        self.shared.retarget.lock().unwrap().take()
    }

    /// Queue a marker, waiting for space. Returns false if asked to stop.
    fn push_marker(&mut self, marker: Marker) -> bool {
        while self.markers.push(marker).is_err() {
            if self.shared.stop.load(Ordering::Relaxed) {
                return false;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        true
    }

    /// Push all of `data`, waiting for space. Returns false if asked to stop.
    fn write(&mut self, mut data: &[f32], out_ch: usize) -> bool {
        self.written += (data.len() / out_ch) as u64;
        while !data.is_empty() {
            if self.shared.stop.load(Ordering::Relaxed) {
                return false;
            }
            let queued = self.capacity - self.producer.slots();
            let limit = self.shared.fill_limit(self.capacity);
            let free = limit.saturating_sub(queued) / out_ch * out_ch;
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
    rates: (u32, u32),
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
            rates: (in_rate, out_rate),
            channels,
            pending: vec![Vec::new(); channels],
            emitted: 0,
        }
    }

    /// Start afresh, dropping any input not yet converted.
    pub fn reset(&mut self) {
        *self = Self::new(self.rates.0, self.rates.1, self.channels);
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
mod fx_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::Curve;
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
            None,
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
            None,
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
            None,
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
        let (mut engine, _f) = start(
            src,
            req,
            false,
            ChannelMap::new(1, 1, false),
            8_000,
            vol,
            None,
        );
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
            None,
        );
        let (out, positions) = render_until_finished(&mut engine, 1, 200_000);
        feeder.join().unwrap();
        let played = out.iter().filter(|v| **v != 0.0).count();
        assert!((played as i64 - 48_000).abs() <= 2, "{played}");
        // Position is in file frames (96 kHz) even though the device runs at 48 kHz.
        let mid = positions[positions.len() / 2];
        assert!(mid > 30_000.0 && mid < 66_000.0, "{mid}");
    }

    /// An engine at 8 kHz mono with no feeder: the test plays the feeder's part.
    fn bare_engine() -> (Engine, Producer<f32>, Producer<Marker>) {
        let (producer, consumer) = RingBuffer::<f32>::new(8_000);
        let (marker_tx, marker_rx) = RingBuffer::<Marker>::new(16);
        let req = PlayRequest {
            start: 0,
            end: 100_000,
            from: 0,
        };
        let shared = Arc::new(Shared::new(8_000, req, false, volume()));
        let engine = Engine::new(consumer, marker_rx, shared, 1, 8_000, 0);
        (engine, producer, marker_tx)
    }

    fn push(producer: &mut Producer<f32>, values: impl IntoIterator<Item = f32>) {
        for v in values {
            producer.push(v).unwrap();
        }
    }

    /// Publish a switch as the feeder does: new audio starts at output `to` with file frame
    /// `frame`, which is where the engine was at output `anchor`.
    fn switch(engine: &Engine, markers: &mut Producer<Marker>, to: u64, anchor: u64, frame: f64) {
        let sh = engine.shared();
        let generation = sh.flush_gen.load(Ordering::Relaxed) + 1;
        markers
            .push(Marker {
                generation,
                out_index: to,
                file_frame: frame,
            })
            .unwrap();
        sh.feeder_done.store(false, Ordering::Release);
        sh.flush_to.store(to, Ordering::Relaxed);
        sh.flush_anchor.store(anchor, Ordering::Relaxed);
        sh.flush_start.store(0, Ordering::Relaxed);
        sh.flush_end.store(50_000, Ordering::Relaxed);
        sh.flush_gen.store(generation, Ordering::Release);
    }

    fn render(engine: &mut Engine, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0; frames];
        engine.render(&mut out, Duration::ZERO);
        out
    }

    #[test]
    fn switch_drops_stale_audio_and_continues_in_the_new_timeline() {
        let (mut engine, mut producer, mut markers) = bare_engine();
        // 1000 frames of the old audio are queued.
        push(&mut producer, std::iter::repeat_n(0.5, 1_000));
        render(&mut engine, 200);
        // The feeder reads the cursor (output 200) and maps it to file frame 5000 of the
        // new audio, while the engine plays on for another 100 frames.
        let (anchor, frame) = engine.shared().cursor();
        assert_eq!((anchor, frame), (200, 200.0));
        render(&mut engine, 100);
        switch(&engine, &mut markers, 1_000, anchor, 5_000.0);
        let new = |i: usize| 1.0 + i as f32 * 0.001;
        push(&mut producer, (0..500).map(new));

        let out = render(&mut engine, 100);
        // Those 100 frames are skipped in the new audio too, so nothing repeats; the old
        // audio fades out under the new over 5 ms (40 frames).
        for (k, v) in out.iter().enumerate() {
            let g = (k as f32 / 40.0).min(1.0);
            let want = new(100 + k) * g + 0.5 * (1.0 - g);
            assert!((v - want).abs() < 1e-6, "frame {k}: {v} vs {want}");
        }
        assert_eq!(engine.shared().cursor(), (1_200, 5_200.0));
        assert_eq!(engine.shared().range(), (0, 50_000));
        assert_eq!(engine.shared().applied_gen.load(Ordering::Acquire), 1);
        // No stale samples are left: the next frames continue the new audio.
        let out = render(&mut engine, 100);
        assert!(out.iter().enumerate().all(|(k, v)| *v == new(200 + k)));
        assert!((engine.shared().position() - 5_200.0).abs() < 1.0);
    }

    #[test]
    fn switch_ramps_up_from_silence() {
        let (mut engine, mut producer, mut markers) = bare_engine();
        switch(&engine, &mut markers, 0, 0, 0.0);
        push(&mut producer, std::iter::repeat_n(0.8, 100));
        let out = render(&mut engine, 100);
        assert_eq!(out[0], 0.0);
        for (k, v) in out.iter().enumerate() {
            let want = 0.8 * (k as f32 / 40.0).min(1.0);
            assert!((v - want).abs() < 1e-6, "frame {k}: {v} vs {want}");
        }
    }

    #[test]
    fn empty_buffer_after_a_switch_is_not_the_end() {
        let (mut engine, mut producer, mut markers) = bare_engine();
        // The feeder had queued everything up to the end of the range.
        push(&mut producer, std::iter::repeat_n(0.5, 50));
        engine.shared().feeder_done.store(true, Ordering::Release);
        switch(&engine, &mut markers, 50, 0, 0.0);
        // The new audio hasn't arrived yet: the old plays on, then there is a gap, but
        // playback hasn't ended.
        let out = render(&mut engine, 100);
        assert!(out[..50].iter().all(|v| *v == 0.5) && out[50..].iter().all(|v| *v == 0.0));
        assert!(!engine.shared().finished.load(Ordering::Acquire));
        // The new audio starts at output 0, so the 100 frames of time passed are skipped.
        push(&mut producer, (0..300).map(|i| i as f32));
        let out = render(&mut engine, 100);
        assert_eq!(out[99], 199.0);
        // Once the new audio is all played, playback ends as usual.
        engine.shared().feeder_done.store(true, Ordering::Release);
        render(&mut engine, 100);
        assert!(!engine.shared().finished.load(Ordering::Acquire));
        render(&mut engine, 100);
        assert!(engine.shared().finished.load(Ordering::Acquire));
    }

    // --- Playback following edits (the feeder and engine together) --------------------

    const RATE: u32 = 8_000;
    /// 60 s whose samples identify their source frame (see `source_frame`).
    const FRAMES: usize = 480_000;

    fn edited(src: &Arc<Audio>, edit: impl FnOnce(EditList) -> EditList) -> Arc<Audio> {
        let list = edit(EditList::identity(src.frames()));
        Arc::new(Audio::with_edits(src.source().clone(), Arc::new(list)))
    }

    /// The source frame an unfaded sample of `ramp_source(FRAMES, RATE)` came from.
    fn source_frame(v: f32) -> i64 {
        (v as f64 * (FRAMES as f64 + 1.0) - 1.0).round() as i64
    }

    /// Buffers as played: the file frame at the start of each, whether the switch had
    /// been carried out by then, and the samples.
    type Played = Vec<(f64, bool, Vec<f32>)>;

    fn samples(played: &Played) -> impl Iterator<Item = f32> + '_ {
        played.iter().flat_map(|b| b.2.iter().copied())
    }

    /// Render at least `frames` frames (mono out) like a device would.
    fn play_for(engine: &mut Engine, frames: usize) -> Played {
        let mut played = Vec::new();
        let mut done = 0;
        while done < frames {
            // Render far faster than real time, but never faster than the feeder (a device
            // asks at the rate the audio plays, by which time the feeder is well ahead).
            let deadline = Instant::now() + Duration::from_millis(200);
            while engine.samples.slots() < 1_024
                && !engine.shared().feeder_done.load(Ordering::Acquire)
                && Instant::now() < deadline
            {
                std::thread::sleep(Duration::from_micros(100));
            }
            let start = engine.shared().cursor().1;
            let switched = engine.shared().applied_gen.load(Ordering::Acquire) > 0;
            let mut buf = vec![0.0; 256];
            engine.render(&mut buf, Duration::ZERO);
            done += 256;
            played.push((start, switched, buf));
        }
        played
    }

    fn start_playing(audio: &Arc<Audio>, start: u64, end: u64, from: u64, looping: bool) -> Engine {
        let req = PlayRequest { start, end, from };
        let (engine, _feeder) = super::start(
            audio.clone(),
            req,
            looping,
            ChannelMap::new(1, 1, false),
            RATE,
            volume(),
            None,
        );
        engine.wait_prefilled(Duration::from_secs(2));
        engine
    }

    /// Switch from `old` to `new` the way `Player::follow` does.
    fn follow(engine: &Engine, old: &Audio, new: &Arc<Audio>) {
        let (start, end) = crate::playback::follow_range(old, new, engine.shared().range());
        engine.shared().retarget(Retarget {
            audio: new.clone(),
            start,
            end,
        });
    }

    /// Every sample of every buffer played wholly after the switch (past the crossfade)
    /// is `audio` at the reported frames, wrapping at the loop range.
    fn assert_plays(played: &Played, audio: &Audio, range: (u64, u64)) {
        let mut checked = 0;
        let mut one = Vec::new();
        for (start, _, buf) in played.iter().filter(|b| b.1).skip(1) {
            for (k, v) in buf.iter().enumerate() {
                let mut f = *start as u64 + k as u64;
                if f >= range.1 {
                    f -= range.1 - range.0;
                }
                audio.read(f, 1, &mut one);
                assert_eq!(*v, one[0], "frame {f}");
                checked += 1;
            }
        }
        assert!(checked > 1_000, "only {checked} samples after the switch");
    }

    #[test]
    fn delete_before_the_playhead_keeps_playing_the_same_audio() {
        let old = ramp_source(FRAMES, RATE);
        let mut engine = start_playing(&old, 0, FRAMES as u64, 236_000, false);
        let mut played = play_for(&mut engine, 4_000);
        // At about 30 s, delete 10–20 s.
        let new = edited(&old, |l| l.delete(80_000, 160_000, 80).unwrap());
        follow(&engine, &old, &new);
        played.extend(play_for(&mut engine, 6_000));
        // The sound runs on through the switch without a jump or repeat.
        let frames: Vec<i64> = samples(&played).map(source_frame).collect();
        if let Some(i) = frames.windows(2).position(|w| w[1] != w[0] + 1) {
            panic!("the sound jumps: {:?}", &frames[i.saturating_sub(2)..i + 3]);
        }
        // The playhead moved back by the deleted length.
        let (at, ..) = played.last().unwrap();
        assert_eq!(*at as i64, frames.last().unwrap() - 255 - 80_000);
        assert_eq!(engine.shared().range(), (0, 400_000));
        assert_plays(&played, &new, (0, 400_000));
        engine.shared().stop.store(true, Ordering::Relaxed);
    }

    #[test]
    fn deleting_the_audio_being_played_continues_from_the_join() {
        let old = ramp_source(FRAMES, RATE);
        let mut engine = start_playing(&old, 0, FRAMES as u64, 116_000, false);
        play_for(&mut engine, 4_000);
        // At about 15 s, delete 10–20 s.
        let new = edited(&old, |l| l.delete(80_000, 160_000, 80).unwrap());
        follow(&engine, &old, &new);
        let played = play_for(&mut engine, 6_000);
        let after: Vec<_> = played.iter().filter(|b| b.1).collect();
        // Playback continues from the join (plus what played meanwhile, at most a ring's
        // worth at this test's speed), with the audio after the deleted part, and the
        // playhead agrees with what is heard.
        let at = after[0].0;
        assert!((80_000.0..84_096.0).contains(&at), "{at}");
        let (next_at, _, next) = after[1];
        assert_eq!(source_frame(next[0]), *next_at as i64 + 80_000);
        assert_plays(&played, &new, (0, 400_000));
        engine.shared().stop.store(true, Ordering::Relaxed);
    }

    #[test]
    fn fade_while_listening() {
        let old = ramp_source(FRAMES, RATE);
        let mut engine = start_playing(&old, 0, FRAMES as u64, 20_000, false);
        let mut played = play_for(&mut engine, 4_000);
        // At about 3.0 s, fade out 5.0–8.0 s.
        let new = edited(&old, |l| {
            l.fade_out_range(40_000, 64_000, Curve::LINEAR).unwrap()
        });
        follow(&engine, &old, &new);
        played.extend(play_for(&mut engine, 46_000));
        // No interruption before the fade, which is heard when playback reaches it.
        let frames: Vec<i64> = samples(&played)
            .map(source_frame)
            .take_while(|&f| f < 40_000)
            .collect();
        assert!(frames.windows(2).all(|w| w[1] == w[0] + 1));
        assert_eq!(*frames.last().unwrap(), 39_999);
        assert_plays(&played, &new, (0, FRAMES as u64));
        engine.shared().stop.store(true, Ordering::Relaxed);
    }

    #[test]
    fn looping_selection_keeps_its_range_through_silence() {
        let old = ramp_source(FRAMES, RATE);
        let mut engine = start_playing(&old, 16_000, 24_000, 16_000, true);
        play_for(&mut engine, 4_000);
        let new = edited(&old, |l| l.silence(18_000, 20_000).unwrap());
        follow(&engine, &old, &new);
        let played = play_for(&mut engine, 20_000);
        assert_eq!(engine.shared().range(), (16_000, 24_000));
        assert_plays(&played, &new, (16_000, 24_000));
        // The silenced part is heard in each loop.
        let silent = samples(&played).filter(|v| *v == 0.0).count();
        assert!(silent >= 4_000, "{silent}");
        engine.shared().stop.store(true, Ordering::Relaxed);
    }

    #[test]
    fn removed_position_with_nothing_after_it_ends() {
        let old = ramp_source(FRAMES, RATE);
        let mut engine = start_playing(&old, 0, FRAMES as u64, 470_000, false);
        play_for(&mut engine, 1_000);
        // Trim the end off before the playhead.
        let new = edited(&old, |l| l.set_trim(0, 400_000));
        follow(&engine, &old, &new);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !engine.shared().finished.load(Ordering::Acquire) {
            assert!(Instant::now() < deadline, "playback never ended");
            play_for(&mut engine, 256);
        }
        assert_eq!(engine.shared().position(), 400_000.0);
    }
}
