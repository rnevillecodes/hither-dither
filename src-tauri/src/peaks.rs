//! Per-channel min/max peak pyramid.
//!
//! Level 0 stores the min/max of each 256-frame block; each higher level merges pairs of
//! buckets from the level below. A query decomposes each pixel column's frame range into
//! whole buckets (taken from the coarsest level that fits) plus partial blocks at the edges,
//! which are read from the sample source, so results are exact at every zoom level.

use crate::edit::{Audio, Dir};
use crate::wav::SampleSource;

pub const BASE_BLOCK: u64 = 256;

/// Changed audio up to this many frames within one column is read exactly; longer runs
/// are bounded from the source pyramid and the gains instead.
pub const EXACT_READ_FRAMES: u64 = 4096;

/// Anything frames can be read from as interleaved `f32`.
pub trait ReadFrames {
    fn read_frames(&self, start: u64, count: usize, out: &mut Vec<f32>);
}

impl ReadFrames for Audio {
    fn read_frames(&self, start: u64, count: usize, out: &mut Vec<f32>) {
        self.read(start, count, out);
    }
}

impl ReadFrames for SampleSource {
    fn read_frames(&self, start: u64, count: usize, out: &mut Vec<f32>) {
        self.read(start, count, out);
    }
}

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
    pub fn query(&self, src: &Audio, start: u64, end: u64, columns: usize) -> Vec<MinMax> {
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
    fn range<R: ReadFrames + ?Sized>(
        &self,
        src: &R,
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

    fn raw<R: ReadFrames + ?Sized>(
        &self,
        src: &R,
        a: u64,
        b: u64,
        acc: &mut [MinMax],
        scratch: &mut Vec<f32>,
    ) {
        if a >= b {
            return;
        }
        src.read_frames(a, (b - a) as usize, scratch);
        for frame in scratch.chunks_exact(self.channels) {
            for (m, &v) in acc.iter_mut().zip(frame) {
                m.add(v);
            }
        }
    }
}

/// Like [`PeakPyramid::query`] for `audio`, using only the pyramid of its unedited
/// source: verbatim copies come from `source`, short changed runs are read exactly, and
/// longer ones are bounded by the source's min/max times the range of gains applied to
/// it. A bound is never smaller than the true peak, but may be larger.
pub fn query_edited(
    source: &PeakPyramid,
    audio: &Audio,
    start: u64,
    end: u64,
    columns: usize,
) -> Vec<MinMax> {
    let ch = audio.channels();
    let end = end.min(audio.frames());
    let start = start.min(end);
    let span = end - start;
    let edited = Edited::new(source, audio);
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
        edited.range(a, b, &mut acc, &mut scratch);
        for (c, m) in acc.iter().enumerate() {
            out[c * columns + col] = *m;
        }
    }
    out
}

/// `audio` read through its source's pyramid.
struct Edited<'a> {
    pyramid: &'a PeakPyramid,
    audio: &'a Audio,
    /// Untrimmed position of each segment's start.
    starts: Vec<u64>,
    /// Untrimmed crossfade regions `(start, end, index of the segment after the join)`.
    zones: Vec<(u64, u64, usize)>,
}

impl<'a> Edited<'a> {
    fn new(pyramid: &'a PeakPyramid, audio: &'a Audio) -> Self {
        let starts = audio.edits().starts();
        let zones = audio
            .edits()
            .segments()
            .iter()
            .enumerate()
            .filter_map(|(k, s)| {
                let half = s.xfade.filter(|x| x.len > 0)?.len / 2;
                Some((starts[k] - half, starts[k] + half, k))
            })
            .collect();
        Self {
            pyramid,
            audio,
            starts,
            zones,
        }
    }

    /// Merge output frames `[a, b)` into `acc`.
    fn range(&self, a: u64, b: u64, acc: &mut [MinMax], scratch: &mut Vec<f32>) {
        let spans = self.audio.spans();
        let mut i = spans.partition_point(|s| s.start + s.len <= a);
        while let Some(s) = spans.get(i).filter(|s| s.start < b) {
            let (p, q) = (s.start.max(a), (s.start + s.len).min(b));
            match s.src {
                Some(from) => {
                    let (s0, s1) = (from + (p - s.start), from + (q - s.start));
                    self.pyramid
                        .range(self.audio.source().as_ref(), s0, s1, acc, scratch);
                }
                None if q - p <= EXACT_READ_FRAMES => {
                    self.pyramid.raw(self.audio, p, q, acc, scratch)
                }
                None => self.bound(p, q, acc, scratch),
            }
            i += 1;
        }
    }

    /// Merge a bound of changed output frames `[p, q)` into `acc`, piece by piece:
    /// crossfade regions, and the parts of segments outside them.
    fn bound(&self, p: u64, q: u64, acc: &mut [MinMax], scratch: &mut Vec<f32>) {
        let edits = self.audio.edits();
        let segments = edits.segments();
        let t0 = edits.trim().start;
        let (mut u, u_end) = (t0 + p, t0 + q);
        while u < u_end {
            let z = self.zones.partition_point(|z| z.1 <= u);
            let zone = self.zones.get(z).copied();
            let (v, piece) = match zone {
                Some((zs, ze, k)) if zs <= u => {
                    let v = ze.min(u_end);
                    (v, self.crossfade(k, u, v, scratch))
                }
                _ => {
                    let i = self.starts.partition_point(|&s| s <= u) - 1;
                    let seg_end = self.starts[i] + segments[i].len;
                    let v = seg_end.min(zone.map_or(u64::MAX, |z| z.0)).min(u_end);
                    (v, self.segment(i, u, v, scratch))
                }
            };
            let (lo, hi) = edits.clip_gain_range(u - t0, v - t0);
            for (a, m) in acc.iter_mut().zip(piece) {
                *a = a.merge(scale(m, lo, hi));
            }
            u = v;
        }
    }

    /// Bound of segment `i` over untrimmed `[u, v)`, outside any crossfade.
    fn segment(&self, i: usize, u: u64, v: u64, scratch: &mut Vec<f32>) -> Vec<MinMax> {
        let seg = &self.audio.edits().segments()[i];
        let (l0, l1) = (u - self.starts[i], v - self.starts[i]);
        let (lo, hi) = seg.gain_range(l0, l1);
        self.source_range(seg.src_start + l0, seg.src_start + l1, scratch)
            .into_iter()
            .map(|m| scale(m, lo, hi))
            .collect()
    }

    /// Bound of the crossfade at the join before segment `k` over untrimmed `[u, v)`: the
    /// sum of both sides' bounds.
    fn crossfade(&self, k: usize, u: u64, v: u64, scratch: &mut Vec<f32>) -> Vec<MinMax> {
        let segments = self.audio.edits().segments();
        let (a, b) = (&segments[k - 1], &segments[k]);
        let xf = b.xfade.expect("a crossfade zone has a crossfade");
        let join = self.starts[k] as i64;
        let half = (xf.len / 2) as i64;
        let (d0, d1) = (u as i64 - join, v as i64 - join);
        let t = |d: i64| (d + half) as f64 / xf.len as f64;
        let ends = |dir| {
            let (x, y) = (xf.curve.gain(dir, t(d0)), xf.curve.gain(dir, t(d1 - 1)));
            (x.min(y), x.max(y))
        };
        // Each side plays on past the cut, with its gain at the cut held beyond it.
        let a_len = a.len as i64;
        let (ga_lo, ga_hi) = a.gain_range(
            (a_len + d0).min(a_len - 1) as u64,
            (a_len + d1).min(a_len) as u64,
        );
        let (gb_lo, gb_hi) = b.gain_range(d0.max(0) as u64, d1.max(1) as u64);
        let (xa, xb) = (ends(Dir::Out), ends(Dir::In));
        let a_src = (a.src_start as i64 + a_len + d0) as u64;
        let b_src = (b.src_start as i64 + d0) as u64;
        let n = (d1 - d0) as u64;
        let side_a = self.source_range(a_src, a_src + n, scratch);
        let side_b = self.source_range(b_src, b_src + n, scratch);
        side_a
            .into_iter()
            .zip(side_b)
            .map(|(ma, mb)| {
                let ma = scale(ma, ga_lo * xa.0, ga_hi * xa.1);
                let mb = scale(mb, gb_lo * xb.0, gb_hi * xb.1);
                MinMax {
                    min: ma.min + mb.min,
                    max: ma.max + mb.max,
                }
            })
            .collect()
    }

    /// Min/max of source frames `[s0, s1)` per channel.
    fn source_range(&self, s0: u64, s1: u64, scratch: &mut Vec<f32>) -> Vec<MinMax> {
        let mut mm = vec![MinMax::EMPTY; self.pyramid.channels];
        self.pyramid
            .range(self.audio.source().as_ref(), s0, s1, &mut mm, scratch);
        mm
    }
}

/// Bound of `x · g` for `x` in `m` and `g` in `[lo, hi]` (`0 ≤ lo ≤ hi`).
fn scale(m: MinMax, lo: f64, hi: f64) -> MinMax {
    if m.is_empty() {
        return m;
    }
    let (min, max) = (m.min as f64, m.max as f64);
    MinMax {
        min: (if min <= 0.0 { min * hi } else { min * lo }) as f32,
        max: (if max >= 0.0 { max * hi } else { max * lo }) as f32,
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

    fn build(src: &Audio) -> PeakPyramid {
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
            Audio::from_bytes(WavBuilder::new(Encoding::Float, 32, 1, 48_000).build(&samples))
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
        let src = Audio::from_bytes(
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

    use crate::edit::{ClipFade, Crossfade, Curve, EditList};
    use std::sync::Arc;

    /// A random edit list over `frames` frames: deletes with short and long crossfades,
    /// silence, straight and curved fades, a trim and clip fades.
    fn random_edits(rng: &mut Rng, frames: u64) -> EditList {
        let mut list = EditList::identity(frames);
        for _ in 0..1 + rng.next() % 6 {
            let n = list.frames();
            let a = rng.next() % n;
            let b = (a + 1 + rng.next() % (n / 4)).min(n);
            let curve = Curve::new(rng.unit());
            let next = match rng.next() % 5 {
                0 => list.delete(a, b, [0, 64, 20_000][(rng.next() % 3) as usize]),
                1 => list.silence(a, b),
                2 => list.fade_in_range(a, b, curve),
                3 => list.fade_out_range(a, b, curve),
                _ => list.delete(a, b, 9_000).and_then(|l| {
                    let j = l.joins().first().map(|j| j.index).unwrap_or(0);
                    l.set_crossfade(j, Crossfade { len: 30_000, curve })
                }),
            };
            if let Ok(next) = next {
                list = next;
            }
        }
        if rng.next().is_multiple_of(2) {
            let n = list.untrimmed_frames();
            list = list.set_trim(rng.next() % (n / 10), n - rng.next() % (n / 10));
        }
        if rng.next().is_multiple_of(2) {
            let f = |len| ClipFade {
                len,
                curve: Curve::new(0.5),
            };
            list = list.set_clip_fades(f(rng.next() % 20_000), f(rng.next() % 20_000));
        }
        list
    }

    #[test]
    fn edited_peaks_are_exact_or_never_hide_a_peak() {
        let mut rng = Rng(0x2545_F491_4F6C_DD1D);
        let frames = 200_003usize;
        let left: Vec<f64> = (0..frames).map(|_| rng.unit()).collect();
        let right: Vec<f64> = (0..frames).map(|_| rng.unit() * 0.3 + 0.2).collect();
        let source = Audio::from_bytes(
            WavBuilder::new(Encoding::Float, 32, 2, 44_100).build(&interleave(&[left, right])),
        )
        .unwrap();
        let pyr = build(&source);
        let (mut exact_cols, mut bounded_cols) = (0, 0);
        for round in 0..40 {
            let list = random_edits(&mut rng, frames as u64);
            let audio = Audio::with_edits(source.source().clone(), Arc::new(list.clone()));
            let mut all = Vec::new();
            audio.read(0, audio.frames() as usize, &mut all);
            let n = audio.frames();
            for _ in 0..10 {
                let a = rng.next() % n;
                let b = a + 1 + rng.next() % (n - a);
                // Half zoomed out, so that long changed runs get bounded.
                let most = if rng.next().is_multiple_of(2) { 8 } else { 300 };
                let columns = 1 + (rng.next() % most) as usize;
                let out = query_edited(&pyr, &audio, a, b, columns);
                for col in 0..columns {
                    let ca = a + (b - a) * col as u64 / columns as u64;
                    let cb = a + (b - a) * (col as u64 + 1) / columns as u64;
                    if ca == cb {
                        continue;
                    }
                    // Exact unless some changed run in the column is longer than the limit.
                    let exact = audio.spans().iter().all(|s| {
                        let overlap = (s.start + s.len).min(cb).saturating_sub(s.start.max(ca));
                        s.src.is_some() || overlap <= EXACT_READ_FRAMES
                    });
                    if exact {
                        exact_cols += 1;
                    } else {
                        bounded_cols += 1;
                    }
                    for ch in 0..2 {
                        let got = out[ch * columns + col];
                        let mut want = MinMax::EMPTY;
                        for f in ca..cb {
                            want.add(all[f as usize * 2 + ch]);
                        }
                        let ctx = || format!("round {round} {list:?} cols {ca}..{cb} ch {ch}");
                        if exact {
                            assert_eq!(got, want, "{}", ctx());
                        } else {
                            assert!(got.max >= want.max - 1e-6, "max {got:?} {want:?} {}", ctx());
                            assert!(got.min <= want.min + 1e-6, "min {got:?} {want:?} {}", ctx());
                        }
                    }
                }
            }
        }
        assert!(
            exact_cols > 1000 && bounded_cols > 200,
            "{exact_cols} {bounded_cols}"
        );
    }

    #[test]
    fn transient_in_a_long_fade_is_not_hidden() {
        let frames = 1_000_000usize;
        let at = 500_123u64;
        let mut samples = vec![0.0; frames];
        samples[at as usize] = 0.9;
        let source =
            Audio::from_bytes(WavBuilder::new(Encoding::Float, 32, 1, 48_000).build(&samples))
                .unwrap();
        let pyr = build(&source);
        let list = EditList::identity(frames as u64)
            .fade_out_range(100_000, 900_000, Curve::new(-0.7))
            .unwrap();
        let audio = Audio::with_edits(source.source().clone(), Arc::new(list));
        let mut one = Vec::new();
        audio.read(at, 1, &mut one);
        let faded = one[0];
        assert!(faded > 0.0 && faded < 0.9);
        for columns in [1, 7, 800, 1920] {
            let out = query_edited(&pyr, &audio, 0, frames as u64, columns);
            let col = (at as usize * columns) / frames;
            assert!(
                out[col].max >= faded,
                "columns {columns}: {} < {faded}",
                out[col].max
            );
        }
        // Zoomed in to a sample per column, every sample is exact.
        let out = query_edited(&pyr, &audio, at - 5, at + 5, 10);
        assert_eq!(out[5].max, faded);
        assert_eq!(out[5].min, faded);
        // Silenced audio is flat even when bounded.
        let silenced = audio.edits().silence(0, 400_000).unwrap();
        let audio = Audio::with_edits(source.source().clone(), Arc::new(silenced));
        let out = query_edited(&pyr, &audio, 0, 400_000, 4);
        assert!(out.iter().all(|m| m.min == 0.0 && m.max == 0.0), "{out:?}");
    }
}
