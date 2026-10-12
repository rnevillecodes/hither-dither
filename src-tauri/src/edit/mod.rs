//! Non-destructive edits: the edited audio is a list of segments of the original source,
//! each optionally silenced or scaled by fade curves, with crossfades where Delete joined
//! two segments. A trim window and fades at either end apply on top, like the edges of a
//! clip. Operations return a new list, so undo history is just a list of `EditList`s.
//!
//! Positions in the public operations are output frames (after trimming). Internally,
//! segments are addressed on the "untrimmed" timeline: their plain concatenation.

mod audio;
pub mod curve;

use std::ops::Range;

pub use audio::{Audio, Span};
pub use curve::{Curve, Dir};
use serde::{Deserialize, Serialize};

/// A section `[t0, t1]` of a fade curve, stretched across a whole segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ramp {
    pub t0: f64,
    pub t1: f64,
    pub dir: Dir,
    pub curve: Curve,
}

impl Ramp {
    /// Gain at fraction `u` (0..=1) of the segment.
    fn at(self, u: f64) -> f64 {
        self.curve.gain(self.dir, self.t0 + (self.t1 - self.t0) * u)
    }

    fn position(self, u: f64) -> f64 {
        self.t0 + (self.t1 - self.t0) * u
    }
}

/// A crossfade between a segment and the one before it, centred on the join.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Crossfade {
    /// Frames, even (half each side of the join); 0 is a hard join.
    pub len: u64,
    pub curve: Curve,
}

/// A fade at one end of the audio.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ClipFade {
    /// Frames; 0 means no fade.
    pub len: u64,
    pub curve: Curve,
}

/// `len` frames of the source starting at `src_start`.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub src_start: u64,
    pub len: u64,
    /// Every frame is digital silence (`ramps` is then empty).
    pub silent: bool,
    /// Gains multiplied together.
    pub ramps: Vec<Ramp>,
    /// Set when Delete joined this segment to the previous one.
    pub xfade: Option<Crossfade>,
}

impl Segment {
    fn new(src_start: u64, len: u64) -> Self {
        Self {
            src_start,
            len,
            silent: false,
            ramps: Vec::new(),
            xfade: None,
        }
    }

    /// Plays the source unchanged (ignoring any crossfade at its start).
    pub fn is_plain(&self) -> bool {
        !self.silent && self.ramps.is_empty()
    }

    /// Gain of segment-local frame `i`.
    pub fn gain(&self, i: u64) -> f64 {
        if self.silent {
            return 0.0;
        }
        let u = i as f64 / self.len as f64;
        self.ramps.iter().map(|r| r.at(u)).product()
    }

    /// Bounds `(lo, hi)` of [`gain`](Self::gain) over local frames `[l0, l1)`. Each ramp
    /// is monotonic, so its extremes are at the ends, and the product's bounds are the
    /// products of the ramps' bounds (gains are never negative).
    pub fn gain_range(&self, l0: u64, l1: u64) -> (f64, f64) {
        if self.silent {
            return (0.0, 0.0);
        }
        let (u0, u1) = (
            l0 as f64 / self.len as f64,
            (l1 - 1) as f64 / self.len as f64,
        );
        self.ramps.iter().fold((1.0, 1.0), |(lo, hi), r| {
            let (a, b) = (r.at(u0), r.at(u1));
            (lo * a.min(b), hi * a.max(b))
        })
    }

    /// Split before local frame `at` (0 < at < len); each half keeps its share of the
    /// ramps, and the crossfade stays with the left half (it belongs to the start).
    fn split(&self, at: u64) -> (Segment, Segment) {
        let u = at as f64 / self.len as f64;
        let part = |src_start, len, xfade, f: &dyn Fn(Ramp) -> Ramp| Segment {
            src_start,
            len,
            silent: self.silent,
            ramps: self.ramps.iter().map(|&r| f(r)).collect(),
            xfade,
        };
        let left = part(self.src_start, at, self.xfade, &|r| Ramp {
            t1: r.position(u),
            ..r
        });
        let right = part(self.src_start + at, self.len - at, None, &|r| Ramp {
            t0: r.position(u),
            ..r
        });
        (left, right)
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EditError {
    #[error("the selection is empty")]
    EmptyRange,
    #[error("the selection is outside the file")]
    OutOfRange,
    #[error("the file can't be left with no audio")]
    WouldBeEmpty,
    #[error("the selection already covers the whole file")]
    WholeFile,
    #[error("there is no join there")]
    NotAJoin,
}

/// A join as the handles need it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct JoinInfo {
    /// Index of the segment after the join (identifies it to `set_crossfade`).
    pub index: usize,
    /// Output frame of the join.
    pub position: i64,
    pub xfade: Crossfade,
    /// Longest crossfade the audio on both sides allows.
    pub max_len: u64,
}

/// Where a source frame ended up in the edited audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Located {
    /// Heard at this output frame.
    At(u64),
    /// Not heard; the audio after it starts at `next`, if any is heard.
    Removed { next: Option<u64> },
}

/// The edited audio, as segments of a source with `source_frames` frames.
#[derive(Debug, Clone, PartialEq)]
pub struct EditList {
    source_frames: u64,
    segments: Vec<Segment>,
    /// Window of the untrimmed timeline that is heard.
    trim: Range<u64>,
    fade_in: ClipFade,
    fade_out: ClipFade,
}

impl EditList {
    /// The source as it is.
    pub fn identity(source_frames: u64) -> Self {
        let segments = if source_frames == 0 {
            Vec::new()
        } else {
            vec![Segment::new(0, source_frames)]
        };
        Self {
            source_frames,
            segments,
            trim: 0..source_frames,
            fade_in: ClipFade::default(),
            fade_out: ClipFade::default(),
        }
    }

    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    pub fn source_frames(&self) -> u64 {
        self.source_frames
    }

    /// Frames heard (after trimming).
    pub fn frames(&self) -> u64 {
        self.trim.end - self.trim.start
    }

    /// Length of the segments before trimming.
    pub fn untrimmed_frames(&self) -> u64 {
        self.segments.iter().map(|s| s.len).sum()
    }

    pub fn trim(&self) -> Range<u64> {
        self.trim.clone()
    }

    pub fn fade_in(&self) -> ClipFade {
        self.fade_in
    }

    pub fn fade_out(&self) -> ClipFade {
        self.fade_out
    }

    /// No edits: plays the whole source unchanged.
    pub fn is_identity(&self) -> bool {
        self.untrimmed_frames() == self.source_frames
            && self.frames() == self.source_frames
            && self.fade_in.len == 0
            && self.fade_out.len == 0
            && self
                .segments
                .iter()
                .all(|s| s.is_plain() && s.xfade.is_none())
            && {
                // Plain segments covering every frame are contiguous only if in source order.
                let mut next = 0;
                self.segments.iter().all(|s| {
                    let ok = s.src_start == next;
                    next += s.len;
                    ok
                })
            }
    }

    // --- Edits on a selection (output positions) ----------------------------------------

    /// Keep only `[start, end)`. Trimmed audio outside it is discarded too.
    pub fn crop(&self, start: u64, end: u64) -> Result<Self, EditError> {
        self.check(start, end)?;
        if start == 0 && end == self.frames() {
            return Err(EditError::WholeFile);
        }
        let (mut list, a, b) = self.split_range(start, end);
        list.segments.truncate(b);
        list.segments.drain(..a);
        list.trim = 0..list.untrimmed_frames();
        Ok(list.normalized())
    }

    /// Remove `[start, end)`, joining what is either side with a crossfade of
    /// `xfade_len` frames (shortened to what the audio allows).
    pub fn delete(&self, start: u64, end: u64, xfade_len: u64) -> Result<Self, EditError> {
        self.check(start, end)?;
        if start == 0 && end == self.frames() {
            return Err(EditError::WouldBeEmpty);
        }
        let (mut list, a, b) = self.split_range(start, end);
        list.segments.drain(a..b);
        list.trim.end -= end - start;
        if a > 0 && a < list.segments.len() {
            list.segments[a].xfade = Some(Crossfade {
                len: xfade_len,
                curve: Curve::LINEAR,
            });
        }
        Ok(list.normalized())
    }

    /// Replace `[start, end)` with digital silence.
    pub fn silence(&self, start: u64, end: u64) -> Result<Self, EditError> {
        self.check(start, end)?;
        let (mut list, a, b) = self.split_range(start, end);
        for s in &mut list.segments[a..b] {
            s.silent = true;
            s.ramps.clear();
        }
        Ok(list.normalized())
    }

    /// Gain rising along `curve` from 0 at `start` to 1 at `end`.
    pub fn fade_in_range(&self, start: u64, end: u64, curve: Curve) -> Result<Self, EditError> {
        self.fade(start, end, Dir::In, curve)
    }

    /// Gain falling along `curve` from 1 at `start` to 0 at `end`.
    pub fn fade_out_range(&self, start: u64, end: u64, curve: Curve) -> Result<Self, EditError> {
        self.fade(start, end, Dir::Out, curve)
    }

    fn fade(&self, start: u64, end: u64, dir: Dir, curve: Curve) -> Result<Self, EditError> {
        self.check(start, end)?;
        let (mut list, a, b) = self.split_range(start, end);
        let total = (end - start) as f64;
        let mut offset = 0u64;
        for s in &mut list.segments[a..b] {
            let t0 = offset as f64 / total;
            let t1 = (offset + s.len) as f64 / total;
            offset += s.len;
            if !s.silent {
                s.ramps.push(Ramp { t0, t1, dir, curve });
            }
        }
        Ok(list.normalized())
    }

    // --- Clip settings ------------------------------------------------------------------

    /// Hear `[start, end)` of the untrimmed timeline (clamped to keep at least one frame).
    pub fn set_trim(&self, start: u64, end: u64) -> Self {
        let mut list = self.clone();
        let total = list.untrimmed_frames();
        let start = start.min(total.saturating_sub(1));
        let end = end.clamp(start + 1, total.max(start + 1));
        list.trim = start..end;
        list.normalized()
    }

    /// Replace the fades at either end (clamped so they fit the audio without overlapping).
    pub fn set_clip_fades(&self, fade_in: ClipFade, fade_out: ClipFade) -> Self {
        let mut list = self.clone();
        list.fade_in = fade_in;
        list.fade_out = fade_out;
        list.normalized()
    }

    /// Replace the crossfade at the join before segment `join` (length clamped).
    pub fn set_crossfade(&self, join: usize, xfade: Crossfade) -> Result<Self, EditError> {
        if self.segments.get(join).is_none_or(|s| s.xfade.is_none()) {
            return Err(EditError::NotAJoin);
        }
        let mut list = self.clone();
        list.segments[join].xfade = Some(xfade);
        Ok(list.normalized())
    }

    /// Longest crossfade at the join before segment `join`: half each side, limited by
    /// half of each neighbour (so crossfades never overlap) and by the source audio
    /// available beyond the cut on each side.
    pub fn max_crossfade(&self, join: usize) -> u64 {
        let (Some(a), Some(b)) = (
            join.checked_sub(1).and_then(|i| self.segments.get(i)),
            self.segments.get(join),
        ) else {
            return 0;
        };
        let after_a = self.source_frames - (a.src_start + a.len);
        let half = (a.len / 2).min(b.len / 2).min(after_a).min(b.src_start);
        half * 2
    }

    /// Untrimmed position of each segment's start.
    pub(crate) fn starts(&self) -> Vec<u64> {
        let mut pos = 0;
        self.segments
            .iter()
            .map(|s| {
                let p = pos;
                pos += s.len;
                p
            })
            .collect()
    }

    /// Joins with their output position (negative or past the end when trimmed away).
    pub fn joins(&self) -> Vec<JoinInfo> {
        let starts = self.starts();
        self.segments
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                s.xfade.map(|xfade| JoinInfo {
                    index: i,
                    position: starts[i] as i64 - self.trim.start as i64,
                    xfade,
                    max_len: self.max_crossfade(i),
                })
            })
            .collect()
    }

    /// Gain of the clip fades at output frame `f`.
    pub fn clip_gain(&self, f: u64) -> f64 {
        let (fi, fo, frames) = (self.fade_in, self.fade_out, self.frames());
        let mut g = 1.0;
        if f < fi.len {
            g *= fi.curve.gain(Dir::In, f as f64 / fi.len as f64);
        }
        if fo.len > 0 && f >= frames - fo.len {
            let from = frames - fo.len;
            g *= fo.curve.gain(Dir::Out, (f - from) as f64 / fo.len as f64);
        }
        g
    }

    /// Bounds `(lo, hi)` of [`clip_gain`](Self::clip_gain) over output frames `[o0, o1)`.
    /// The gain rises through the fade-in, holds, then falls through the fade-out, so the
    /// least is at an end and the most at an end or where a fade meets the middle.
    pub fn clip_gain_range(&self, o0: u64, o1: u64) -> (f64, f64) {
        let last = o1 - 1;
        let (a, b) = (self.clip_gain(o0), self.clip_gain(last));
        let lo = a.min(b);
        let peak_from = self.fade_in.len.clamp(o0, last);
        let peak_to = (self.frames() - self.fade_out.len)
            .saturating_sub(1)
            .clamp(o0, last);
        let hi = a
            .max(b)
            .max(self.clip_gain(peak_from))
            .max(self.clip_gain(peak_to));
        (lo, hi)
    }

    // --- Position mapping ---------------------------------------------------------------

    /// Source frame heard at output frame `out` (`None` past the end). Inside a crossfade
    /// this is the frame of the segment that owns the position, not the blended neighbour.
    pub fn source_at(&self, out: u64) -> Option<u64> {
        if out >= self.frames() {
            return None;
        }
        let u = self.trim.start + out;
        let starts = self.starts();
        let i = starts.partition_point(|&s| s <= u) - 1;
        Some(self.segments[i].src_start + (u - starts[i]))
    }

    /// Where source frame `src` is heard. A source frame appears at most once, so this is
    /// unambiguous; if it was removed, `next` is where the audio that followed it now
    /// starts (the join that replaced it), or `None` if nothing after it is heard.
    pub fn output_of(&self, src: u64) -> Located {
        let starts = self.starts();
        // Segments are in source order, so the first one ending past `src` holds it or,
        // if it starts after `src`, is the audio that followed it.
        let Some(i) = self.segments.iter().position(|s| s.src_start + s.len > src) else {
            return Located::Removed { next: None };
        };
        let seg = &self.segments[i];
        let (u, kept) = if seg.src_start <= src {
            (starts[i] + (src - seg.src_start), true)
        } else {
            (starts[i], false)
        };
        let next = if u < self.trim.start {
            Some(0)
        } else if u < self.trim.end {
            Some(u - self.trim.start)
        } else {
            None
        };
        match next {
            Some(out) if kept && u >= self.trim.start => Located::At(out),
            next => Located::Removed { next },
        }
    }

    // --- Internals ----------------------------------------------------------------------

    /// Re-establish every limit: no crossfade on the first segment, crossfades within
    /// what the audio allows (even lengths), trim inside the audio, fades that fit.
    fn normalized(mut self) -> Self {
        if let Some(first) = self.segments.first_mut() {
            first.xfade = None;
        }
        for i in 1..self.segments.len() {
            let max = self.max_crossfade(i);
            if let Some(x) = &mut self.segments[i].xfade {
                x.len = x.len.min(max) & !1;
            }
        }
        let total = self.untrimmed_frames();
        self.trim.end = self.trim.end.min(total);
        self.trim.start = self.trim.start.min(self.trim.end.saturating_sub(1));
        let frames = self.frames();
        self.fade_in.len = self.fade_in.len.min(frames);
        self.fade_out.len = self.fade_out.len.min(frames - self.fade_in.len);
        self
    }

    fn check(&self, start: u64, end: u64) -> Result<(), EditError> {
        if end > self.frames() || start > end {
            return Err(EditError::OutOfRange);
        }
        if start == end {
            return Err(EditError::EmptyRange);
        }
        Ok(())
    }

    /// A copy split at output positions `start` and `end`, with the index range of the
    /// segments inside them.
    fn split_range(&self, start: u64, end: u64) -> (Self, usize, usize) {
        let mut list = self.clone();
        let a = list.split_at(self.trim.start + start);
        let b = list.split_at(self.trim.start + end);
        (list, a, b)
    }

    /// Make untrimmed position `pos` fall on a segment boundary; returns the index of the
    /// segment starting there (or `segments.len()` at the end).
    fn split_at(&mut self, pos: u64) -> usize {
        let mut seg_start = 0;
        for i in 0..self.segments.len() {
            let len = self.segments[i].len;
            if pos == seg_start {
                return i;
            }
            if pos < seg_start + len {
                let (l, r) = self.segments[i].split(pos - seg_start);
                self.segments[i] = l;
                self.segments.insert(i + 1, r);
                return i + 1;
            }
            seg_start += len;
        }
        self.segments.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (src_start, len) of each segment.
    fn spans(list: &EditList) -> Vec<(u64, u64)> {
        list.segments()
            .iter()
            .map(|s| (s.src_start, s.len))
            .collect()
    }

    /// Segment gain of each untrimmed frame.
    fn gains(list: &EditList) -> Vec<f64> {
        list.segments()
            .iter()
            .flat_map(|s| (0..s.len).map(move |i| s.gain(i)))
            .collect()
    }

    const LIN: Curve = Curve::LINEAR;

    #[test]
    fn identity_is_one_plain_segment() {
        let list = EditList::identity(100);
        assert_eq!(spans(&list), [(0, 100)]);
        assert!(list.is_identity());
        assert_eq!(list.frames(), 100);
    }

    #[test]
    fn crop_keeps_the_range() {
        let list = EditList::identity(100).crop(20, 75).unwrap();
        assert_eq!(spans(&list), [(20, 55)]);
        assert_eq!(list.frames(), 55);
        assert!(!list.is_identity());
    }

    #[test]
    fn delete_joins_either_side() {
        let list = EditList::identity(100).delete(40, 50, 0).unwrap();
        assert_eq!(spans(&list), [(0, 40), (50, 50)]);
        assert_eq!(list.frames(), 90);
    }

    #[test]
    fn delete_at_the_start_and_end() {
        let list = EditList::identity(100).delete(0, 10, 4).unwrap();
        assert_eq!(spans(&list), [(10, 90)]);
        let list = list.delete(80, 90, 4).unwrap();
        assert_eq!(spans(&list), [(10, 80)]);
        assert!(list.joins().is_empty(), "no join at either end");
    }

    #[test]
    fn edits_compose_in_output_positions() {
        // Delete 40..50, then crop output 30..60, which spans the join.
        let list = EditList::identity(100)
            .delete(40, 50, 0)
            .unwrap()
            .crop(30, 60)
            .unwrap();
        assert_eq!(spans(&list), [(30, 10), (50, 20)]);
    }

    #[test]
    fn silence_keeps_length() {
        let list = EditList::identity(10).silence(2, 5).unwrap();
        assert_eq!(list.frames(), 10);
        assert_eq!(
            gains(&list),
            [1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0]
        );
    }

    #[test]
    fn linear_fades() {
        let list = EditList::identity(4).fade_in_range(0, 4, LIN).unwrap();
        assert_eq!(gains(&list), [0.0, 0.25, 0.5, 0.75]);
        let list = EditList::identity(4).fade_out_range(0, 4, LIN).unwrap();
        assert_eq!(gains(&list), [1.0, 0.75, 0.5, 0.25]);
    }

    #[test]
    fn curved_fades_follow_the_curve() {
        let curve = Curve::new(0.7);
        let list = EditList::identity(8).fade_in_range(0, 8, curve).unwrap();
        for (i, g) in gains(&list).iter().enumerate() {
            assert!((g - curve.gain(Dir::In, i as f64 / 8.0)).abs() < 1e-12);
        }
    }

    #[test]
    fn fading_twice_squares_the_gain() {
        let once = gains(&EditList::identity(8).fade_out_range(2, 6, LIN).unwrap());
        let twice = gains(
            &EditList::identity(8)
                .fade_out_range(2, 6, LIN)
                .unwrap()
                .fade_out_range(2, 6, LIN)
                .unwrap(),
        );
        for (a, b) in once.iter().zip(&twice) {
            assert!((a * a - b).abs() < 1e-12, "{a}² != {b}");
        }
    }

    #[test]
    fn splitting_a_curved_fade_keeps_its_gains() {
        let faded = EditList::identity(10)
            .fade_in_range(0, 10, Curve::new(-0.6))
            .unwrap();
        let before = gains(&faded);
        let cut = faded.delete(4, 5, 0).unwrap();
        let mut expected = before.clone();
        expected.remove(4);
        let after = gains(&cut);
        assert_eq!(after.len(), expected.len());
        for (a, b) in after.iter().zip(&expected) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn fade_spanning_segments_is_one_continuous_curve() {
        let list = EditList::identity(10)
            .delete(4, 6, 0)
            .unwrap()
            .fade_in_range(0, 8, LIN)
            .unwrap();
        for (i, v) in gains(&list).iter().enumerate() {
            assert!((v - i as f64 / 8.0).abs() < 1e-12);
        }
    }

    #[test]
    fn silence_over_a_fade_and_fade_over_silence() {
        let list = EditList::identity(6)
            .fade_in_range(0, 6, LIN)
            .unwrap()
            .silence(2, 4)
            .unwrap();
        assert_eq!(&gains(&list)[2..4], [0.0, 0.0]);
        let list = list.fade_out_range(0, 6, LIN).unwrap();
        assert_eq!(&gains(&list)[2..4], [0.0, 0.0]);
    }

    #[test]
    fn rejects_bad_ranges() {
        let list = EditList::identity(10);
        assert_eq!(list.silence(3, 3), Err(EditError::EmptyRange));
        assert_eq!(list.fade_in_range(5, 11, LIN), Err(EditError::OutOfRange));
        assert_eq!(list.delete(0, 10, 0), Err(EditError::WouldBeEmpty));
        assert_eq!(list.crop(0, 10), Err(EditError::WholeFile));
        assert_eq!(
            list.set_crossfade(0, Crossfade::default()),
            Err(EditError::NotAJoin)
        );
    }

    #[test]
    fn delete_adds_a_clamped_crossfade() {
        let list = EditList::identity(1_000).delete(400, 500, 20).unwrap();
        let joins = list.joins();
        assert_eq!(joins.len(), 1);
        assert_eq!((joins[0].index, joins[0].position), (1, 400));
        assert_eq!(joins[0].xfade.len, 20);
        // Near the start of the source there is little audio before the join.
        let list = EditList::identity(1_000).delete(3, 500, 20).unwrap();
        assert_eq!(list.joins()[0].xfade.len, 2, "limited to 1 frame each side");
        // An odd request becomes even (equal halves).
        let list = EditList::identity(1_000).delete(400, 500, 21).unwrap();
        assert_eq!(list.joins()[0].xfade.len, 20);
    }

    #[test]
    fn crossfades_are_clamped_by_the_audio() {
        let list = EditList::identity(1_000).delete(400, 500, 10).unwrap();
        // Half of each side (400 and 500 frames) and the audio around the cut.
        assert_eq!(list.max_crossfade(1), 400);
        let wide = list
            .set_crossfade(
                1,
                Crossfade {
                    len: 10_000,
                    curve: Curve::new(0.5),
                },
            )
            .unwrap();
        let j = wide.joins()[0];
        assert_eq!((j.xfade.len, j.xfade.curve.bend), (400, 0.5));
        // Zero is a hard join, and it stays a join.
        let hard = list
            .set_crossfade(1, Crossfade { len: 0, curve: LIN })
            .unwrap();
        assert_eq!(hard.joins()[0].xfade.len, 0);
    }

    #[test]
    fn trimming_and_restoring() {
        let list = EditList::identity(100).set_trim(15, 100);
        assert_eq!(list.frames(), 85);
        // A delete in output positions, then restoring the trim, leaves the delete intact.
        let list = list.delete(10, 20, 0).unwrap();
        assert_eq!(list.frames(), 75);
        assert_eq!(list.trim(), 15..90);
        let restored = list.set_trim(0, list.untrimmed_frames());
        assert_eq!(restored.frames(), 90);
        assert_eq!(spans(&restored), [(0, 25), (35, 65)]);
    }

    #[test]
    fn trim_keeps_at_least_one_frame() {
        let list = EditList::identity(100).set_trim(150, 10);
        assert_eq!(list.trim(), 99..100);
        let list = EditList::identity(100).set_trim(40, 40);
        assert_eq!(list.trim(), 40..41);
    }

    #[test]
    fn clip_fades_fit_and_follow_trims() {
        let f = |len| ClipFade { len, curve: LIN };
        let list = EditList::identity(200).set_clip_fades(f(50), f(150));
        assert_eq!((list.fade_in().len, list.fade_out().len), (50, 150));
        // The fade-in can't grow past the fade-out.
        let list = list.set_clip_fades(f(120), f(150));
        assert_eq!((list.fade_in().len, list.fade_out().len), (120, 80));
        // Trimming keeps fades attached to the ends, clamped if the audio got shorter.
        let list = EditList::identity(200)
            .set_clip_fades(f(50), f(0))
            .set_trim(100, 200);
        assert_eq!(list.fade_in().len, 50);
        let list = list.set_trim(170, 200);
        assert_eq!(list.fade_in().len, 30);
    }

    #[test]
    fn delete_before_the_playhead_keeps_the_audio() {
        // Playing at 30.0 s (1 kHz frames) when 10.0–20.0 s is deleted: same audio, at 20.0 s.
        let old = EditList::identity(60_000);
        let new = old.delete(10_000, 20_000, 0).unwrap();
        let src = old.source_at(30_000).unwrap();
        assert_eq!(new.output_of(src), Located::At(20_000));
        assert_eq!(new.source_at(20_000), Some(src));
    }

    #[test]
    fn deleted_audio_continues_from_the_join() {
        let old = EditList::identity(60_000);
        let new = old.delete(10_000, 20_000, 400).unwrap();
        let src = old.source_at(15_000).unwrap();
        assert_eq!(new.output_of(src), Located::Removed { next: Some(10_000) });
        // Inside the crossfade each side maps by its own frames.
        assert_eq!(new.source_at(9_999), Some(9_999));
        assert_eq!(new.source_at(10_000), Some(20_000));
        assert_eq!(new.output_of(9_900), Located::At(9_900));
        assert_eq!(new.output_of(20_100), Located::At(10_100));
    }

    #[test]
    fn mapping_through_trims_and_crops() {
        let list = EditList::identity(100).set_trim(10, 80);
        assert_eq!(list.source_at(0), Some(10));
        assert_eq!(list.source_at(70), None);
        assert_eq!(list.output_of(10), Located::At(0));
        // Before the trim window: the audio after it starts at 0.
        assert_eq!(list.output_of(5), Located::Removed { next: Some(0) });
        // An end trim before a position: nothing after it is heard.
        assert_eq!(list.output_of(85), Located::Removed { next: None });
        let cropped = EditList::identity(100).crop(20, 60).unwrap();
        assert_eq!(cropped.output_of(10), Located::Removed { next: Some(0) });
        assert_eq!(cropped.output_of(30), Located::At(10));
        assert_eq!(cropped.output_of(60), Located::Removed { next: None });
        // A delete followed by a trim that starts after the join.
        let list = EditList::identity(100)
            .delete(40, 50, 0)
            .unwrap()
            .set_trim(45, 90);
        assert_eq!(list.output_of(45), Located::Removed { next: Some(0) });
        assert_eq!(list.output_of(60), Located::At(5));
        for out in 0..list.frames() {
            assert_eq!(
                list.output_of(list.source_at(out).unwrap()),
                Located::At(out)
            );
        }
    }

    #[test]
    fn crop_resets_the_trim() {
        let list = EditList::identity(100)
            .set_trim(10, 90)
            .crop(5, 50)
            .unwrap();
        assert_eq!(spans(&list), [(15, 45)]);
        assert_eq!(list.trim(), 0..45);
    }
}
