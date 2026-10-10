//! Per-channel min/max peak pyramid.
//!
//! Level 0 stores the min/max of each 256-frame block; each higher level merges pairs of
//! buckets from the level below. A query decomposes each pixel column's frame range into
//! whole buckets (taken from the coarsest level that fits) plus partial blocks at the edges,
//! which are read from the sample source, so results are exact at every zoom level.

use crate::wav::SampleSource;

pub const BASE_BLOCK: u64 = 256;

/// Inclusive sample range `[min, max]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinMax {
    pub min: f32,
    pub max: f32,
}

impl MinMax {
    pub const EMPTY: MinMax = MinMax {
        min: f32::INFINITY,
        max: f32::NEG_INFINITY,
    };

    #[inline(always)]
    pub fn add(&mut self, v: f32) {
        self.min = self.min.min(v);
        self.max = self.max.max(v);
    }

    #[inline(always)]
    pub fn merge(self, o: MinMax) -> MinMax {
        MinMax {
            min: self.min.min(o.min),
            max: self.max.max(o.max),
        }
    }

    pub fn is_empty(self) -> bool {
        self.min > self.max
    }
}

pub struct PeakPyramid {
    channels: usize,
    frames: u64,
    /// `levels[l][bucket * channels + ch]`.
    levels: Vec<Vec<MinMax>>,
}

/// Builds a pyramid from frames pushed in order, in a single streaming pass.
pub struct PyramidBuilder {
    channels: usize,
    level0: Vec<MinMax>,
    current: Vec<MinMax>,
    in_block: u64,
    frames: u64,
}

impl PyramidBuilder {
    pub fn new(channels: usize, expected_frames: u64) -> Self {
        Self {
            channels,
            level0: Vec::with_capacity(
                (expected_frames.div_ceil(BASE_BLOCK) as usize).saturating_mul(channels),
            ),
            current: vec![MinMax::EMPTY; channels],
            in_block: 0,
            frames: 0,
        }
    }

    /// Append interleaved frames.
    pub fn push(&mut self, interleaved: &[f32]) {
        let ch = self.channels;
        let mut frames = interleaved.chunks_exact(ch);
        loop {
            // Fill the current block as far as possible.
            let room = (BASE_BLOCK - self.in_block) as usize;
            let mut taken = 0;
            for frame in frames.by_ref().take(room) {
                for (acc, &v) in self.current.iter_mut().zip(frame) {
                    acc.add(v);
                }
                taken += 1;
            }
            self.in_block += taken as u64;
            self.frames += taken as u64;
            if self.in_block == BASE_BLOCK {
                self.flush_block();
            }
            if taken < room {
                break;
            }
        }
    }

    fn flush_block(&mut self) {
        self.level0.extend_from_slice(&self.current);
        self.current.fill(MinMax::EMPTY);
        self.in_block = 0;
    }

    pub fn finish(mut self) -> PeakPyramid {
        if self.in_block > 0 {
            self.flush_block();
        }
        let ch = self.channels;
        let mut levels = vec![self.level0];
        while levels.last().unwrap().len() > ch {
            let below = levels.last().unwrap();
            let buckets = below.len() / ch;
            let mut next = Vec::with_capacity(buckets.div_ceil(2) * ch);
            for pair in 0..buckets.div_ceil(2) {
                for c in 0..ch {
                    let a = below[2 * pair * ch + c];
                    let b = below.get((2 * pair + 1) * ch + c).copied();
                    next.push(b.map_or(a, |b| a.merge(b)));
                }
            }
            levels.push(next);
        }
        PeakPyramid {
            channels: ch,
            frames: self.frames,
            levels,
        }
    }
}

impl PeakPyramid {
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Approximate heap size in bytes.
    pub fn memory_bytes(&self) -> usize {
        self.levels.iter().map(Vec::len).sum::<usize>() * std::mem::size_of::<MinMax>()
    }

    /// Min/max of frames `[start, end)` for every channel, split evenly into `columns`
    /// columns. Returns `columns * channels` values, channel-major (`[ch][column]`).
    /// Columns that cover no frames are `(0, 0)`.
    pub fn query(&self, src: &SampleSource, start: u64, end: u64, columns: usize) -> Vec<MinMax> {
        let ch = self.channels;
        let end = end.min(self.frames);
        let start = start.min(end);
        let span = end - start;
        let mut out = vec![MinMax { min: 0.0, max: 0.0 }; columns * ch];
        let mut acc = vec![MinMax::EMPTY; ch];
        let mut scratch = Vec::new();
        for col in 0..columns {
            let a = start + span * col as u64 / columns as u64;
            let b = start + span * (col as u64 + 1) / columns as u64;
            if a >= b {
                continue;
            }
            acc.fill(MinMax::EMPTY);
            self.range(src, a, b, &mut acc, &mut scratch);
            for (c, m) in acc.iter().enumerate() {
                out[c * columns + col] = *m;
            }
        }
        out
    }

    /// Merge the min/max of frames `[a, b)` into `acc` (one entry per channel).
    fn range(
        &self,
        src: &SampleSource,
        a: u64,
        b: u64,
        acc: &mut [MinMax],
        scratch: &mut Vec<f32>,
    ) {
        let first_block = a.div_ceil(BASE_BLOCK);
        let last_block = b / BASE_BLOCK;
        if first_block >= last_block {
            self.raw(src, a, b, acc, scratch);
            return;
        }
        self.raw(src, a, first_block * BASE_BLOCK, acc, scratch);
        self.raw(src, last_block * BASE_BLOCK, b, acc, scratch);

        // Bottom-up segment-tree walk over whole buckets [i, j).
        let ch = self.channels;
        let (mut i, mut j) = (first_block as usize, last_block as usize);
        let mut level = 0;
        while i < j {
            let buckets = &self.levels[level];
            if i & 1 == 1 {
                merge_bucket(acc, &buckets[i * ch..(i + 1) * ch]);
                i += 1;
            }
            if j & 1 == 1 {
                j -= 1;
                merge_bucket(acc, &buckets[j * ch..(j + 1) * ch]);
            }
            i /= 2;
            j /= 2;
            level += 1;
        }
    }

    fn raw(&self, src: &SampleSource, a: u64, b: u64, acc: &mut [MinMax], scratch: &mut Vec<f32>) {
        if a >= b {
            return;
        }
        src.read(a, (b - a) as usize, scratch);
        for frame in scratch.chunks_exact(self.channels) {
            for (m, &v) in acc.iter_mut().zip(frame) {
                m.add(v);
            }
        }
    }
}

fn merge_bucket(acc: &mut [MinMax], bucket: &[MinMax]) {
    for (a, b) in acc.iter_mut().zip(bucket) {
        *a = a.merge(*b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wav::synth::{Encoding, WavBuilder, interleave};

    fn build(src: &SampleSource) -> PeakPyramid {
        let mut b = PyramidBuilder::new(src.channels(), src.frames());
        let mut buf = Vec::new();
        let mut pos = 0;
        // Odd-sized chunks exercise block boundaries inside push().
        while pos < src.frames() {
            src.read(pos, 1000, &mut buf);
            b.push(&buf);
            pos += 1000;
        }
        b.finish()
    }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn unit(&mut self) -> f64 {
            (self.next() % 2_000_001) as f64 / 1_000_000.0 - 1.0
        }
    }

    #[test]
    fn spike_in_silence_visible_at_every_zoom() {
        let frames = 1_000_003usize;
        let spike_at = 654_321usize;
        let mut samples = vec![0.0; frames];
        samples[spike_at] = 1.0;
        let src =
            SampleSource::from_bytes(WavBuilder::new(Encoding::Float, 32, 1, 48_000).build(&samples))
                .unwrap();
        let pyr = build(&src);
        assert_eq!(pyr.frames(), frames as u64);

        let mut columns = 1;
        while columns <= frames {
            let out = pyr.query(&src, 0, frames as u64, columns);
            let expected_col = (0..columns)
                .find(|&c| {
                    let a = frames * c / columns;
                    let b = frames * (c + 1) / columns;
                    (a..b).contains(&spike_at)
                })
                .unwrap();
            for (c, m) in out.iter().enumerate() {
                if c == expected_col {
                    assert_eq!(m.max, 1.0, "columns={columns}");
                } else {
                    assert_eq!(m.max, 0.0, "columns={columns} col={c}");
                }
            }
            columns = columns * 3 + 1;
        }
    }

    #[test]
    fn matches_brute_force_on_random_data() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let frames = 300_017;
        let left: Vec<f64> = (0..frames).map(|_| rng.unit()).collect();
        let right: Vec<f64> = (0..frames).map(|_| rng.unit() * 0.5).collect();
        let src = SampleSource::from_bytes(
            WavBuilder::new(Encoding::Pcm, 16, 2, 44_100).build(&interleave(&[left, right])),
        )
        .unwrap();
        let pyr = build(&src);
        let mut all = Vec::new();
        src.read(0, frames, &mut all);

        for _ in 0..200 {
            let a = rng.next() % frames as u64;
            let b = a + 1 + rng.next() % (frames as u64 - a);
            let columns = 1 + (rng.next() % 1500) as usize;
            let out = pyr.query(&src, a, b, columns);
            let span = b - a;
            for col in 0..columns {
                let ca = a + span * col as u64 / columns as u64;
                let cb = a + span * (col as u64 + 1) / columns as u64;
                for ch in 0..2 {
                    let got = out[ch * columns + col];
                    if ca == cb {
                        assert_eq!(got, MinMax { min: 0.0, max: 0.0 });
                        continue;
                    }
                    let mut want = MinMax::EMPTY;
                    for f in ca..cb {
                        want.add(all[f as usize * 2 + ch]);
                    }
                    assert_eq!(got, want, "range {a}..{b} cols {columns} col {col} ch {ch}");
                }
            }
        }
    }
}
