//! Non-destructive edits: the edited audio is a list of segments of the original source,
//! each optionally silenced or scaled by linear gain ramps. Operations return a new list,
//! so undo history is just a list of `EditList`s.

mod audio;

pub use audio::Audio;

/// A linear gain ramp across a whole segment: `g0` at its first frame, reaching `g1` at
/// its end (one past the last frame).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ramp {
    pub g0: f64,
    pub g1: f64,
}

impl Ramp {
    /// Gain at fraction `t` (0..=1) of the segment.
    fn at(self, t: f64) -> f64 {
        self.g0 + (self.g1 - self.g0) * t
    }
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
}

impl Segment {
    /// Plays the source unchanged.
    pub fn is_plain(&self) -> bool {
        !self.silent && self.ramps.is_empty()
    }

    /// Gain of segment-local frame `i`.
    pub fn gain(&self, i: u64) -> f64 {
        if self.silent {
            return 0.0;
        }
        let t = i as f64 / self.len as f64;
        self.ramps.iter().map(|r| r.at(t)).product()
    }

    /// Split before local frame `at` (0 < at < len); each half keeps its share of the ramps.
    fn split(&self, at: u64) -> (Segment, Segment) {
        let t = at as f64 / self.len as f64;
        let left = Segment {
            src_start: self.src_start,
            len: at,
            silent: self.silent,
            ramps: self
                .ramps
                .iter()
                .map(|r| Ramp {
                    g0: r.g0,
                    g1: r.at(t),
                })
                .collect(),
        };
        let right = Segment {
            src_start: self.src_start + at,
            len: self.len - at,
            silent: self.silent,
            ramps: self
                .ramps
                .iter()
                .map(|r| Ramp {
                    g0: r.at(t),
                    g1: r.g1,
                })
                .collect(),
        };
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
}

/// The edited audio, as segments of a source with `source_frames` frames.
#[derive(Debug, Clone, PartialEq)]
pub struct EditList {
    source_frames: u64,
    segments: Vec<Segment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fade {
    In,
    Out,
}

impl EditList {
    /// The source as it is.
    pub fn identity(source_frames: u64) -> Self {
        let segments = if source_frames == 0 {
            Vec::new()
        } else {
            vec![Segment {
                src_start: 0,
                len: source_frames,
                silent: false,
                ramps: Vec::new(),
            }]
        };
        Self {
            source_frames,
            segments,
        }
    }

    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    pub fn frames(&self) -> u64 {
        self.segments.iter().map(|s| s.len).sum()
    }

    /// No edits: plays the whole source unchanged.
    pub fn is_identity(&self) -> bool {
        self.frames() == self.source_frames && self.segments.iter().all(Segment::is_plain) && {
            // Plain segments covering every frame are contiguous only if in source order.
            let mut next = 0;
            self.segments.iter().all(|s| {
                let ok = s.src_start == next;
                next += s.len;
                ok
            })
        }
    }

    /// Keep only `[start, end)`.
    pub fn crop(&self, start: u64, end: u64) -> Result<Self, EditError> {
        self.check(start, end)?;
        if start == 0 && end == self.frames() {
            return Err(EditError::WholeFile);
        }
        let (mut list, a, b) = self.split_range(start, end);
        list.segments.truncate(b);
        list.segments.drain(..a);
        Ok(list)
    }

    /// Remove `[start, end)`, joining what is either side.
    pub fn delete(&self, start: u64, end: u64) -> Result<Self, EditError> {
        self.check(start, end)?;
        if start == 0 && end == self.frames() {
            return Err(EditError::WouldBeEmpty);
        }
        let (mut list, a, b) = self.split_range(start, end);
        list.segments.drain(a..b);
        Ok(list)
    }

    /// Replace `[start, end)` with digital silence.
    pub fn silence(&self, start: u64, end: u64) -> Result<Self, EditError> {
        self.check(start, end)?;
        let (mut list, a, b) = self.split_range(start, end);
        for s in &mut list.segments[a..b] {
            s.silent = true;
            s.ramps.clear();
        }
        Ok(list)
    }

    /// Gain rising linearly from 0 at `start` to 1 at `end`.
    pub fn fade_in(&self, start: u64, end: u64) -> Result<Self, EditError> {
        self.fade(start, end, Fade::In)
    }

    /// Gain falling linearly from 1 at `start` to 0 at `end`.
    pub fn fade_out(&self, start: u64, end: u64) -> Result<Self, EditError> {
        self.fade(start, end, Fade::Out)
    }

    fn fade(&self, start: u64, end: u64, fade: Fade) -> Result<Self, EditError> {
        self.check(start, end)?;
        let (mut list, a, b) = self.split_range(start, end);
        let total = (end - start) as f64;
        let mut offset = 0u64;
        for s in &mut list.segments[a..b] {
            let t0 = offset as f64 / total;
            let t1 = (offset + s.len) as f64 / total;
            offset += s.len;
            if s.silent {
                continue;
            }
            s.ramps.push(match fade {
                Fade::In => Ramp { g0: t0, g1: t1 },
                Fade::Out => Ramp {
                    g0: 1.0 - t0,
                    g1: 1.0 - t1,
                },
            });
        }
        Ok(list)
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

    /// A copy split at `start` and `end`, with the index range of segments inside them.
    fn split_range(&self, start: u64, end: u64) -> (Self, usize, usize) {
        let mut list = self.clone();
        let a = list.split_at(start);
        let b = list.split_at(end);
        (list, a, b)
    }

    /// Make `pos` fall on a segment boundary; returns the index of the segment starting there
    /// (or `segments.len()` at the end).
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

    /// Gain of each output frame, by walking the segments.
    fn gains(list: &EditList) -> Vec<f64> {
        list.segments()
            .iter()
            .flat_map(|s| (0..s.len).map(move |i| s.gain(i)))
            .collect()
    }

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
        let list = EditList::identity(100).delete(40, 50).unwrap();
        assert_eq!(spans(&list), [(0, 40), (50, 50)]);
        assert_eq!(list.frames(), 90);
    }

    #[test]
    fn delete_at_the_start_and_end() {
        let list = EditList::identity(100).delete(0, 10).unwrap();
        assert_eq!(spans(&list), [(10, 90)]);
        let list = list.delete(80, 90).unwrap();
        assert_eq!(spans(&list), [(10, 80)]);
    }

    #[test]
    fn edits_compose_in_output_positions() {
        // Delete 40..50, then crop output 30..60, which spans the join.
        let list = EditList::identity(100)
            .delete(40, 50)
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
    fn fade_in_and_out_are_linear() {
        let list = EditList::identity(4).fade_in(0, 4).unwrap();
        assert_eq!(gains(&list), [0.0, 0.25, 0.5, 0.75]);
        let list = EditList::identity(4).fade_out(0, 4).unwrap();
        assert_eq!(gains(&list), [1.0, 0.75, 0.5, 0.25]);
    }

    #[test]
    fn fading_twice_squares_the_gain() {
        let once = gains(&EditList::identity(8).fade_out(2, 6).unwrap());
        let twice = gains(
            &EditList::identity(8)
                .fade_out(2, 6)
                .unwrap()
                .fade_out(2, 6)
                .unwrap(),
        );
        for (a, b) in once.iter().zip(&twice) {
            assert!((a * a - b).abs() < 1e-12, "{a}² != {b}");
        }
    }

    #[test]
    fn splitting_a_faded_segment_keeps_its_gains() {
        let faded = EditList::identity(10).fade_in(0, 10).unwrap();
        let before = gains(&faded);
        // Silence a frame inside the fade, then remove it: the rest keeps the same gains.
        let cut = faded.delete(4, 5).unwrap();
        let mut expected = before.clone();
        expected.remove(4);
        let after = gains(&cut);
        assert_eq!(after.len(), expected.len());
        for (a, b) in after.iter().zip(&expected) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn fade_spanning_segments_is_one_continuous_line() {
        let list = EditList::identity(10)
            .delete(4, 6)
            .unwrap()
            .fade_in(0, 8)
            .unwrap();
        let g = gains(&list);
        for (i, v) in g.iter().enumerate() {
            assert!((v - i as f64 / 8.0).abs() < 1e-12);
        }
    }

    #[test]
    fn silence_over_a_fade_and_fade_over_silence() {
        let list = EditList::identity(6)
            .fade_in(0, 6)
            .unwrap()
            .silence(2, 4)
            .unwrap();
        assert_eq!(&gains(&list)[2..4], [0.0, 0.0]);
        let list = list.fade_out(0, 6).unwrap();
        assert_eq!(&gains(&list)[2..4], [0.0, 0.0]);
    }

    #[test]
    fn rejects_bad_ranges() {
        let list = EditList::identity(10);
        assert_eq!(list.silence(3, 3), Err(EditError::EmptyRange));
        assert_eq!(list.fade_in(5, 11), Err(EditError::OutOfRange));
        assert_eq!(list.delete(0, 10), Err(EditError::WouldBeEmpty));
        assert_eq!(list.crop(0, 10), Err(EditError::WholeFile));
    }
}
