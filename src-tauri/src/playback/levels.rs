//! Level metering of the audio being played: peak and RMS per file channel, measured after
//! the effects in short blocks, and reported once each block is heard.

use std::collections::VecDeque;

/// File frames per measurement block.
pub const BLOCK_FRAMES: usize = 256;
/// RMS is averaged over this much of the most recently heard audio.
pub const RMS_WINDOW_SECS: f64 = 0.3;
/// Blocks not yet heard are kept for at most this long (the ring holds less).
const QUEUE_SECS: f64 = 1.0;

/// Levels of one block of audio, in the file's channels.
#[derive(Debug, Clone, PartialEq)]
pub struct LevelBlock {
    /// Output frame index just after this block is heard.
    pub out_end: u64,
    pub frames: u32,
    /// Largest magnitude per channel.
    pub peak: Vec<f32>,
    /// Sum of squared samples per channel.
    pub sum_sq: Vec<f64>,
}

/// Measure interleaved `frames` (all of one block) of `channels` channels.
pub fn measure(frames: &[f32], channels: usize, out_end: u64) -> LevelBlock {
    let mut peak = vec![0.0f32; channels];
    let mut sum_sq = vec![0.0f64; channels];
    for frame in frames.chunks_exact(channels) {
        for (c, v) in frame.iter().enumerate() {
            peak[c] = peak[c].max(v.abs());
            sum_sq[c] += (*v as f64) * (*v as f64);
        }
    }
    LevelBlock {
        out_end,
        frames: (frames.len() / channels) as u32,
        peak,
        sum_sq,
    }
}

/// Levels per channel as `[peak, rms]`, linear.
pub type Levels = Vec<[f32; 2]>;

/// Blocks measured ahead of the output, turned into levels as they are heard.
#[derive(Debug)]
pub struct Meter {
    channels: usize,
    /// Measured, not yet heard, in output order.
    queued: VecDeque<LevelBlock>,
    max_queued: usize,
    /// Heard, within the RMS window.
    heard: VecDeque<LevelBlock>,
    heard_frames: u64,
    window_frames: u64,
    /// The latest engine skip already dropped (see `Shared::levels`).
    pub skip_seq: u64,
}

impl Meter {
    pub fn new(channels: usize, file_rate: u32) -> Self {
        let blocks = (file_rate as f64 * QUEUE_SECS / BLOCK_FRAMES as f64).ceil() as usize;
        Self {
            channels,
            queued: VecDeque::new(),
            max_queued: blocks.max(4),
            heard: VecDeque::new(),
            heard_frames: 0,
            window_frames: ((file_rate as f64 * RMS_WINDOW_SECS) as u64).max(1),
            skip_seq: 0,
        }
    }

    pub fn push_block(&mut self, block: LevelBlock) {
        debug_assert_eq!(block.peak.len(), self.channels);
        self.queued.push_back(block);
        while self.queued.len() > self.max_queued {
            self.queued.pop_front();
        }
    }

    /// Forget blocks ending in output indices `(from, to]`: playback skipped them unheard.
    pub fn drop_range(&mut self, from: u64, to: u64) {
        self.queued.retain(|b| b.out_end <= from || b.out_end > to);
    }

    /// Take the blocks heard by output index `audible`. Returns the largest peak among
    /// them and the RMS over the window, or `None` if no block was newly heard.
    pub fn advance_to(&mut self, audible: f64) -> Option<Levels> {
        let mut peak: Option<Vec<f32>> = None;
        while self
            .queued
            .front()
            .is_some_and(|b| b.out_end as f64 <= audible)
        {
            let block = self.queued.pop_front().expect("checked");
            let p = peak.get_or_insert_with(|| vec![0.0; self.channels]);
            for (m, v) in p.iter_mut().zip(&block.peak) {
                *m = m.max(*v);
            }
            self.heard_frames += block.frames as u64;
            self.heard.push_back(block);
        }
        let peak = peak?;
        while self.heard.len() > 1 {
            let first = self.heard[0].frames as u64;
            if self.heard_frames - first < self.window_frames {
                break;
            }
            self.heard_frames -= first;
            self.heard.pop_front();
        }
        let n = self.heard_frames.max(1) as f64;
        Some(
            (0..self.channels)
                .map(|c| {
                    let sum: f64 = self.heard.iter().map(|b| b.sum_sq[c]).sum();
                    [peak[c], (sum / n).sqrt() as f32]
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn db(x: f32) -> f64 {
        20.0 * (x as f64).log10()
    }

    /// Mono blocks of `samples`, tagged with output indices from 0.
    fn fill(meter: &mut Meter, samples: &[f32], out: &mut u64) {
        for block in samples.chunks(BLOCK_FRAMES) {
            *out += block.len() as u64;
            meter.push_block(measure(block, 1, *out));
        }
    }

    #[test]
    fn measures_peak_and_sum_of_squares_per_channel() {
        let b = measure(&[0.5, -0.25, -0.75, 0.25], 2, 2);
        assert_eq!(b.frames, 2);
        assert_eq!(b.peak, vec![0.75, 0.25]);
        assert_eq!(b.sum_sq, vec![0.8125, 0.125]);
    }

    #[test]
    fn full_scale_sine_reads_0_db_peak_and_minus_3_db_rms() {
        let sine: Vec<f32> = (0..RATE)
            .map(|i| (2.0 * std::f64::consts::PI * 1_000.0 * i as f64 / RATE as f64).sin() as f32)
            .collect();
        let mut meter = Meter::new(1, RATE);
        let mut out = 0;
        fill(&mut meter, &sine[..RATE as usize / 2], &mut out);
        let levels = meter.advance_to(out as f64).unwrap();
        assert!(db(levels[0][0]).abs() < 0.01, "{}", db(levels[0][0]));
        assert!(
            (db(levels[0][1]) + 3.01).abs() < 0.05,
            "{}",
            db(levels[0][1])
        );
    }

    #[test]
    fn nothing_is_reported_before_a_block_is_heard() {
        let mut meter = Meter::new(1, RATE);
        assert_eq!(meter.advance_to(1e9), None);
        meter.push_block(measure(&[0.5; BLOCK_FRAMES], 1, 256));
        assert_eq!(meter.advance_to(255.0), None);
        assert!(meter.advance_to(256.0).is_some());
        assert_eq!(meter.advance_to(1_000.0), None, "no new block");
    }

    #[test]
    fn a_transient_between_reports_is_kept() {
        let mut meter = Meter::new(1, RATE);
        let mut quiet = vec![0.01f32; BLOCK_FRAMES * 4];
        quiet[300] = 0.9;
        let mut out = 0;
        fill(&mut meter, &quiet, &mut out);
        // One report covers all four blocks; the click was in the second.
        let levels = meter.advance_to(out as f64).unwrap();
        assert_eq!(levels[0][0], 0.9);
        // The next report has only quiet audio.
        fill(&mut meter, &[0.01; BLOCK_FRAMES], &mut out);
        assert_eq!(meter.advance_to(out as f64).unwrap()[0][0], 0.01);
    }

    #[test]
    fn rms_covers_only_the_recent_window() {
        let mut meter = Meter::new(1, RATE);
        let mut out = 0;
        fill(&mut meter, &vec![1.0; RATE as usize], &mut out);
        meter.advance_to(out as f64);
        // 300 ms of silence pushes the loud audio out of the window.
        fill(
            &mut meter,
            &vec![0.0; (RATE as f64 * 0.31) as usize],
            &mut out,
        );
        let levels = meter.advance_to(out as f64).unwrap();
        assert_eq!(levels[0][1], 0.0);
    }

    #[test]
    fn skipped_blocks_are_never_reported() {
        let mut meter = Meter::new(1, RATE);
        meter.push_block(measure(&[0.1; BLOCK_FRAMES], 1, 256));
        meter.push_block(measure(&[0.9; BLOCK_FRAMES], 1, 512));
        meter.push_block(measure(&[0.8; BLOCK_FRAMES], 1, 768));
        meter.push_block(measure(&[0.2; BLOCK_FRAMES], 1, 1_024));
        meter.drop_range(256, 768);
        assert_eq!(meter.advance_to(1_024.0).unwrap()[0][0], 0.2);
    }

    #[test]
    fn the_queue_is_capped() {
        let mut meter = Meter::new(1, 8_000);
        let mut out = 0;
        fill(&mut meter, &vec![0.5; 8_000 * 5], &mut out);
        assert!(meter.queued.len() <= 32, "{}", meter.queued.len());
    }
}
