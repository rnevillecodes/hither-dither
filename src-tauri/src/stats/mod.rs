//! Streaming statistics: levels, clipping and loudness over a frame range.

mod clipping;
mod levels;
mod loudness;

pub use clipping::{ClipRun, MAX_STORED_RUNS, MIN_RUN};
pub use levels::Levels;
pub use loudness::Loudness;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::peaks::PyramidBuilder;
use crate::wav::SampleSource;
use clipping::ClipDetector;
use levels::LevelAccumulator;
use loudness::LoudnessMeter;

const BLOCK_FRAMES: usize = 65_536;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ChannelStats {
    #[serde(flatten)]
    pub levels: Levels,
    pub clip_runs: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Stats {
    pub start: u64,
    pub end: u64,
    pub channels: Vec<ChannelStats>,
    pub loudness: Loudness,
}

/// Statistics plus the clip-run positions found in the range, per channel.
#[derive(Debug, Clone)]
pub struct Analysis {
    pub stats: Stats,
    pub clip_runs: Vec<Vec<ClipRun>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cancelled;

/// Analyse frames `[start, end)` in one pass. If `pyramid` is given it is fed the same
/// frames. `progress` receives the number of frames processed so far.
pub fn analyse(
    src: &SampleSource,
    start: u64,
    end: u64,
    cancel: &AtomicBool,
    mut pyramid: Option<&mut PyramidBuilder>,
    mut progress: impl FnMut(u64),
) -> Result<Analysis, Cancelled> {
    let end = end.min(src.frames());
    let start = start.min(end);
    let ch = src.channels();
    let mut levels = vec![LevelAccumulator::default(); ch];
    let mut clips = vec![ClipDetector::default(); ch];
    let mut meter = LoudnessMeter::new(src.info(), end - start);
    let (mut buf, mut fs) = (Vec::new(), Vec::new());

    let mut pos = start;
    while pos < end {
        if cancel.load(Ordering::Relaxed) {
            return Err(Cancelled);
        }
        let count = (end - pos).min(BLOCK_FRAMES as u64) as usize;
        src.read_with_full_scale(pos, count, &mut buf, &mut fs);
        for (i, (frame, flags)) in buf.chunks_exact(ch).zip(fs.chunks_exact(ch)).enumerate() {
            let f = pos + i as u64;
            for c in 0..ch {
                levels[c].add(frame[c]);
                clips[c].add(f, flags[c]);
            }
        }
        meter.add(&buf);
        if let Some(p) = pyramid.as_deref_mut() {
            p.push(&buf);
        }
        pos += count as u64;
        progress(pos - start);
    }

    let channels = levels
        .iter()
        .zip(clips.iter_mut())
        .map(|(l, c)| {
            c.finish();
            ChannelStats {
                levels: l.finish(),
                clip_runs: c.count,
            }
        })
        .collect();
    Ok(Analysis {
        stats: Stats {
            start,
            end,
            channels,
            loudness: meter.finish(),
        },
        clip_runs: clips.into_iter().map(|c| c.runs).collect(),
    })
}

/// Runs range analyses on worker threads; starting a new one cancels the previous one.
#[derive(Default)]
pub struct StatsRunner {
    current: Mutex<Option<Arc<AtomicBool>>>,
}

impl StatsRunner {
    /// Cancel any running job.
    pub fn cancel(&self) {
        if let Some(old) = self.current.lock().unwrap().take() {
            old.store(true, Ordering::Relaxed);
        }
    }

    /// Start analysing `[start, end)`. `done` receives `None` if the job was superseded.
    pub fn spawn(
        &self,
        src: Arc<SampleSource>,
        start: u64,
        end: u64,
        done: impl FnOnce(Option<Analysis>) + Send + 'static,
    ) -> std::thread::JoinHandle<()> {
        let token = Arc::new(AtomicBool::new(false));
        if let Some(old) = self.current.lock().unwrap().replace(token.clone()) {
            old.store(true, Ordering::Relaxed);
        }
        std::thread::spawn(move || {
            done(analyse(&src, start, end, &token, None, |_| {}).ok());
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wav::synth::{Encoding, WavBuilder, db_to_amp, interleave, sine};

    fn src(enc: Encoding, bits: u16, ch: u16, rate: u32, samples: &[f64]) -> SampleSource {
        SampleSource::from_bytes(WavBuilder::new(enc, bits, ch, rate).build(samples)).unwrap()
    }

    fn whole(src: &SampleSource) -> Analysis {
        analyse(src, 0, u64::MAX, &AtomicBool::new(false), None, |_| {}).unwrap()
    }

    fn near(got: Option<f64>, want: f64, tol: f64) {
        let got = got.expect("finite value");
        assert!((got - want).abs() <= tol, "got {got}, want {want}±{tol}");
    }

    #[test]
    fn full_scale_sine_and_silent_channel() {
        let rate = 48_000;
        let tone = sine(1000.0, 1.0, rate, rate as usize);
        let silence = vec![0.0; tone.len()];
        for (enc, bits) in [(Encoding::Pcm, 16), (Encoding::Float, 32)] {
            let s = src(
                enc,
                bits,
                2,
                rate,
                &interleave(&[tone.clone(), silence.clone()]),
            );
            let st = whole(&s).stats;
            let l = &st.channels[0].levels;
            near(l.peak_dbfs, 0.0, 0.1);
            near(l.rms_dbfs, -3.0, 0.1);
            assert!(l.dc_offset_percent.abs() <= 0.1);
            near(l.crest_db, 3.0, 0.1);

            let z = &st.channels[1].levels;
            assert_eq!((z.peak_dbfs, z.rms_dbfs, z.crest_db), (None, None, None));
            assert_eq!(z.peak_linear, 0.0);
        }
    }

    #[test]
    fn dc_offset_is_percent_of_full_scale() {
        let s = src(Encoding::Float, 32, 1, 8_000, &vec![0.25; 8_000]);
        assert!((whole(&s).stats.channels[0].levels.dc_offset_percent - 25.0).abs() < 1e-6);
    }

    #[test]
    fn counts_clip_runs_not_isolated_samples() {
        for (enc, bits) in [(Encoding::Pcm, 16), (Encoding::Float, 32)] {
            let mut x = vec![0.3; 1000];
            x[100..105].fill(1.0);
            x[500..505].fill(1.0);
            x[800] = 1.0;
            x[900] = -1.0;
            x[901] = -1.0;
            let a = whole(&src(enc, bits, 1, 8_000, &x));
            assert_eq!(a.stats.channels[0].clip_runs, 2, "{enc:?}");
            assert_eq!(
                a.clip_runs[0],
                [
                    ClipRun { start: 100, len: 5 },
                    ClipRun { start: 500, len: 5 }
                ]
            );
        }
        // Negative full scale and a run ending at the end of the range both count.
        let mut x = vec![0.0; 10];
        x[7..].fill(-1.0);
        assert_eq!(
            whole(&src(Encoding::Pcm, 16, 1, 8_000, &x)).stats.channels[0].clip_runs,
            1
        );
    }

    #[test]
    fn clip_runs_span_block_boundaries() {
        let mut x = vec![0.0; BLOCK_FRAMES + 10];
        x[BLOCK_FRAMES - 2..BLOCK_FRAMES + 2].fill(1.0);
        let a = whole(&src(Encoding::Pcm, 16, 1, 8_000, &x));
        assert_eq!(
            a.clip_runs[0],
            [ClipRun {
                start: BLOCK_FRAMES as u64 - 2,
                len: 4
            }]
        );
    }

    #[test]
    fn reference_tone_loudness() {
        let rate = 48_000;
        let tone = sine(1000.0, db_to_amp(-18.0), rate, rate as usize * 5);
        let s = src(
            Encoding::Float,
            32,
            2,
            rate,
            &interleave(&[tone.clone(), tone]),
        );
        match whole(&s).stats.loudness {
            Loudness::Value { lufs } => assert!((lufs + 18.0).abs() <= 0.1, "{lufs}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn loudness_unavailable_for_short_range_and_odd_layouts() {
        let rate = 48_000;
        let tone = sine(1000.0, 0.5, rate, rate as usize);
        let s = src(Encoding::Float, 32, 1, rate, &tone);
        let a = analyse(&s, 0, 9_600, &AtomicBool::new(false), None, |_| {}).unwrap();
        assert!(matches!(a.stats.loudness, Loudness::Unavailable { .. }));
        assert!(matches!(whole(&s).stats.loudness, Loudness::Value { .. }));

        let quad = src(Encoding::Float, 32, 4, rate, &vec![0.1; rate as usize * 4]);
        assert!(matches!(
            whole(&quad).stats.loudness,
            Loudness::Unavailable { .. }
        ));

        let surround = src(Encoding::Float, 32, 6, rate, &vec![0.1; rate as usize * 6]);
        assert!(matches!(
            whole(&surround).stats.loudness,
            Loudness::Value { .. }
        ));

        let silent = src(Encoding::Float, 32, 2, rate, &vec![0.0; rate as usize * 2]);
        assert_eq!(whole(&silent).stats.loudness, Loudness::Silent);
    }

    #[test]
    fn range_stats_cover_only_the_range() {
        let rate = 8_000;
        let mut x = sine(440.0, 0.9, rate, 8_000);
        x[2_000..4_000].fill(0.0);
        let s = src(Encoding::Pcm, 16, 1, rate, &x);
        let a = analyse(&s, 2_000, 4_000, &AtomicBool::new(false), None, |_| {}).unwrap();
        assert_eq!((a.stats.start, a.stats.end), (2_000, 4_000));
        assert_eq!(a.stats.channels[0].levels.peak_dbfs, None);
    }

    fn ten_minutes_stereo() -> Arc<SampleSource> {
        let rate = 48_000;
        let frames = rate as usize * 600;
        let tone = sine(997.0, 0.5, rate, frames);
        let bytes =
            WavBuilder::new(Encoding::Pcm, 16, 2, rate).build(&interleave(&[tone.clone(), tone]));
        Arc::new(SampleSource::from_bytes(bytes).unwrap())
    }

    #[test]
    #[cfg_attr(debug_assertions, ignore = "timing only meaningful in release builds")]
    fn ten_minute_selection_under_one_second() {
        let s = ten_minutes_stereo();
        let t = std::time::Instant::now();
        let a = analyse(&s, 0, s.frames(), &AtomicBool::new(false), None, |_| {}).unwrap();
        let elapsed = t.elapsed();
        assert!(matches!(a.stats.loudness, Loudness::Value { .. }));
        assert!(elapsed.as_secs_f64() < 1.0, "took {elapsed:?}");
    }

    #[test]
    fn newer_job_cancels_older() {
        let s = ten_minutes_stereo();
        let runner = StatsRunner::default();
        let (tx, rx) = std::sync::mpsc::channel();
        let tx2 = tx.clone();
        let t = std::time::Instant::now();
        let first = runner.spawn(s.clone(), 0, s.frames(), move |r| {
            tx.send(("first", r.is_some(), t.elapsed())).unwrap()
        });
        std::thread::sleep(std::time::Duration::from_millis(5));
        let second = runner.spawn(s.clone(), 0, 48_000, move |r| {
            tx2.send(("second", r.is_some(), t.elapsed())).unwrap()
        });
        first.join().unwrap();
        second.join().unwrap();
        let results: Vec<_> = rx.iter().take(2).collect();
        let first = results.iter().find(|r| r.0 == "first").unwrap();
        let second = results.iter().find(|r| r.0 == "second").unwrap();
        assert!(!first.1, "first job should have been cancelled");
        assert!(second.1);
        // A cancelled job stops within one block rather than running to completion.
        assert!(first.2.as_secs_f64() < 1.0, "{:?}", first.2);
    }
}
