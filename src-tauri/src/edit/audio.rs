//! Reading edited audio: maps output frames through the trim window onto segments of the
//! source, then mixes crossfades at joins and applies the clip fades.

use std::sync::Arc;

use super::{Curve, Dir, EditList};
use crate::wav::{SampleSource, WavError, WavInfo};

/// A run of output frames: either a verbatim copy of source frames starting at `src`, or
/// (`src: None`) frames that an edit, fade or crossfade changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u64,
    pub len: u64,
    pub src: Option<u64>,
}

/// Sample units for rendering: normalised floats (as `SampleSource::read`) or the
/// source's own integer/float values (for writing without losing precision).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Units {
    Normal,
    Native,
}

/// A source as seen through an edit list. Peaks, stats and playback read through this, so
/// they always see the edited audio.
pub struct Audio {
    source: Arc<SampleSource>,
    edits: Arc<EditList>,
    /// The source's info with `frames` set to the edited length.
    info: WavInfo,
    /// Untrimmed position at which each segment starts.
    starts: Vec<u64>,
    identity: bool,
    /// The output as verbatim copies and changed runs (see [`spans`](Self::spans)).
    spans: Vec<Span>,
}

impl Audio {
    /// The source unedited.
    pub fn new(source: Arc<SampleSource>) -> Self {
        let edits = Arc::new(EditList::identity(source.frames()));
        Self::with_edits(source, edits)
    }

    /// Parse an in-memory file image, unedited.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, WavError> {
        Ok(Self::new(Arc::new(SampleSource::from_bytes(bytes)?)))
    }

    pub fn with_edits(source: Arc<SampleSource>, edits: Arc<EditList>) -> Self {
        let mut info = source.info().clone();
        info.frames = edits.frames();
        let starts = edits.starts();
        let spans = compute_spans(&edits, &starts);
        Self {
            identity: edits.is_identity(),
            starts,
            source,
            edits,
            info,
            spans,
        }
    }

    pub fn source(&self) -> &Arc<SampleSource> {
        &self.source
    }

    pub fn edits(&self) -> &Arc<EditList> {
        &self.edits
    }

    /// Whether both read the same underlying file mapping.
    pub fn same_source(&self, other: &Audio) -> bool {
        Arc::ptr_eq(&self.source, &other.source)
    }

    pub fn info(&self) -> &WavInfo {
        &self.info
    }

    pub fn frames(&self) -> u64 {
        self.info.frames
    }

    pub fn channels(&self) -> usize {
        self.info.channels as usize
    }

    /// Decode frames `[start, start + count)` as interleaved `f32` into `out` (replacing its
    /// contents). Requests past the end are clamped, so `out` may be shorter than asked.
    pub fn read(&self, start: u64, count: usize, out: &mut Vec<f32>) {
        if self.identity {
            return self.source.read(start, count, out);
        }
        let mut rendered = Vec::new();
        self.render(start, count, Units::Normal, &mut rendered, None);
        out.clear();
        out.extend(rendered.iter().map(|&v| v as f32));
    }

    /// Like [`read`](Self::read), and also set `full_scale[i]` when sample `i` is at full
    /// scale in the source and reaches the output unchanged.
    pub fn read_with_full_scale(
        &self,
        start: u64,
        count: usize,
        out: &mut Vec<f32>,
        full_scale: &mut Vec<bool>,
    ) {
        if self.identity {
            return self
                .source
                .read_with_full_scale(start, count, out, full_scale);
        }
        let mut rendered = Vec::new();
        self.render(start, count, Units::Normal, &mut rendered, Some(full_scale));
        out.clear();
        out.extend(rendered.iter().map(|&v| v as f32));
    }

    /// Frames `[start, start + count)` in the source's own units: integer sample values
    /// for PCM (8-bit centred on zero), float values for IEEE float.
    pub fn render_native(&self, start: u64, count: usize, out: &mut Vec<f64>) {
        self.render(start, count, Units::Native, out, None);
    }

    /// The output as runs of verbatim source copies and changed frames, in order.
    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    /// Source frames `[start, start + n)` in `units`, replacing `out` (and `fs`).
    fn fetch(
        &self,
        start: u64,
        n: usize,
        units: Units,
        out: &mut Vec<f64>,
        fs: Option<&mut Vec<bool>>,
    ) {
        match units {
            Units::Native => self.source.read_native(start, n, out),
            Units::Normal => {
                let mut buf = Vec::new();
                match fs {
                    Some(fs) => self.source.read_with_full_scale(start, n, &mut buf, fs),
                    None => self.source.read(start, n, &mut buf),
                }
                out.clear();
                out.extend(buf.iter().map(|&v| v as f64));
            }
        }
    }

    fn render(
        &self,
        start: u64,
        count: usize,
        units: Units,
        out: &mut Vec<f64>,
        mut fs: Option<&mut Vec<bool>>,
    ) {
        out.clear();
        if let Some(fs) = fs.as_deref_mut() {
            fs.clear();
        }
        let ch = self.channels();
        let frames = self.frames();
        let end = start.saturating_add(count as u64).min(frames);
        let start = start.min(end);
        if start == end {
            return;
        }
        let edits = &self.edits;
        let segments = edits.segments();
        let trim = edits.trim();
        let (u0, u1) = (trim.start + start, trim.start + end);

        // 1. Each segment's own audio and gains.
        let mut i = self.starts.partition_point(|&s| s <= u0) - 1;
        let mut u = u0;
        let (mut buf, mut fbuf) = (Vec::new(), Vec::new());
        while u < u1 {
            let seg = &segments[i];
            let local = u - self.starts[i];
            let n = (seg.len - local).min(u1 - u);
            if seg.silent {
                out.resize(out.len() + n as usize * ch, 0.0);
                if let Some(fs) = fs.as_deref_mut() {
                    fs.resize(fs.len() + n as usize * ch, false);
                }
            } else {
                let want_fs = if fs.is_some() { Some(&mut fbuf) } else { None };
                self.fetch(seg.src_start + local, n as usize, units, &mut buf, want_fs);
                if seg.ramps.is_empty() {
                    out.extend_from_slice(&buf);
                    if let Some(fs) = fs.as_deref_mut() {
                        fs.extend_from_slice(&fbuf);
                    }
                } else {
                    for (f, frame) in buf.chunks_exact(ch).enumerate() {
                        let g = seg.gain(local + f as u64);
                        out.extend(frame.iter().map(|&v| v * g));
                        if let Some(fs) = fs.as_deref_mut() {
                            let unity = g == 1.0;
                            fs.extend(fbuf[f * ch..(f + 1) * ch].iter().map(|&b| b && unity));
                        }
                    }
                }
            }
            u += n;
            i += 1;
        }

        // 2. Crossfades: across a join each side plays on past the cut, into the audio
        //    the delete removed (before it) and the audio just before it (after it).
        let (mut a_buf, mut b_buf) = (Vec::new(), Vec::new());
        for k in 1..segments.len() {
            let Some(xf) = segments[k].xfade.filter(|x| x.len > 0) else {
                continue;
            };
            let join = self.starts[k];
            let half = xf.len / 2;
            let (r0, r1) = ((join - half).max(u0), (join + half).min(u1));
            if r0 >= r1 {
                continue;
            }
            let (a, b) = (&segments[k - 1], &segments[k]);
            let n = (r1 - r0) as usize;
            // Offset of r0 from the join (negative before it).
            let d0 = r0 as i64 - join as i64;
            let a_src = (a.src_start + a.len) as i64 + d0;
            let b_src = b.src_start as i64 + d0;
            self.fetch(a_src as u64, n, units, &mut a_buf, None);
            self.fetch(b_src as u64, n, units, &mut b_buf, None);
            for f in 0..n {
                let d = d0 + f as i64;
                let t = (d + half as i64) as f64 / xf.len as f64;
                let ga = if d < 0 {
                    a.gain((a.len as i64 + d) as u64)
                } else {
                    a.gain(a.len - 1)
                } * xf.curve.gain(Dir::Out, t);
                let gb =
                    if d >= 0 { b.gain(d as u64) } else { b.gain(0) } * xf.curve.gain(Dir::In, t);
                let o = ((r0 - u0) as usize + f) * ch;
                for c in 0..ch {
                    out[o + c] = a_buf[f * ch + c] * ga + b_buf[f * ch + c] * gb;
                }
                if let Some(fs) = fs.as_deref_mut() {
                    fs[o..o + ch].fill(false);
                }
            }
        }

        // 3. Fades at either end of the (trimmed) audio.
        let mut apply = |from: u64, to: u64, dir: Dir, curve: Curve| {
            let len = to - from;
            for f in from.max(start)..to.min(end) {
                let g = curve.gain(dir, (f - from) as f64 / len as f64);
                let o = (f - start) as usize * ch;
                out[o..o + ch].iter_mut().for_each(|v| *v *= g);
                if g != 1.0
                    && let Some(fs) = fs.as_deref_mut()
                {
                    fs[o..o + ch].fill(false);
                }
            }
        };
        let fade_in = edits.fade_in();
        if fade_in.len > 0 {
            apply(0, fade_in.len, Dir::In, fade_in.curve);
        }
        let fade_out = edits.fade_out();
        if fade_out.len > 0 {
            apply(frames - fade_out.len, frames, Dir::Out, fade_out.curve);
        }
    }
}

/// Split the output of `edits` into verbatim source copies and changed runs.
fn compute_spans(edits: &EditList, starts: &[u64]) -> Vec<Span> {
    let trim = edits.trim();
    // Untrimmed ranges whose frames are computed, not copied.
    let mut changed: Vec<(u64, u64)> = Vec::new();
    for (seg, &s) in edits.segments().iter().zip(starts) {
        if !seg.is_plain() {
            changed.push((s, s + seg.len));
        }
        if let Some(x) = seg.xfade.filter(|x| x.len > 0) {
            changed.push((s - x.len / 2, s + x.len / 2));
        }
    }
    let (fi, fo) = (edits.fade_in().len, edits.fade_out().len);
    if fi > 0 {
        changed.push((trim.start, trim.start + fi));
    }
    if fo > 0 {
        changed.push((trim.end - fo, trim.end));
    }
    // Cut the trim window at every segment start and changed-range bound.
    let mut cuts: Vec<u64> = starts.to_vec();
    cuts.extend(changed.iter().flat_map(|&(a, b)| [a, b]));
    cuts.extend([trim.start, trim.end]);
    cuts.retain(|c| (trim.start..=trim.end).contains(c));
    cuts.sort_unstable();
    cuts.dedup();

    let mut out: Vec<Span> = Vec::new();
    for w in cuts.windows(2) {
        let (p, q) = (w[0], w[1]);
        let i = starts.partition_point(|&s| s <= p) - 1;
        let seg = &edits.segments()[i];
        let copied = seg.is_plain() && !changed.iter().any(|&(a, b)| a <= p && q <= b);
        let span = Span {
            start: p - trim.start,
            len: q - p,
            src: copied.then(|| seg.src_start + (p - starts[i])),
        };
        let joins_last = out.last().is_some_and(|last| match (last.src, span.src) {
            (None, None) => true,
            (Some(a), Some(b)) => a + last.len == b,
            _ => false,
        });
        if joins_last {
            out.last_mut().unwrap().len += span.len;
        } else {
            out.push(span);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::{ClipFade, Crossfade};
    use crate::wav::synth::{Encoding, WavBuilder};

    /// A stereo float source whose left channel at frame i is `i / 1000` and right is its
    /// negation, so every sample identifies its source frame.
    fn source(frames: usize) -> Arc<SampleSource> {
        let samples: Vec<f64> = (0..frames)
            .flat_map(|i| [i as f64 / 1000.0, -(i as f64) / 1000.0])
            .collect();
        Arc::new(
            SampleSource::from_bytes(WavBuilder::new(Encoding::Float, 32, 2, 8000).build(&samples))
                .unwrap(),
        )
    }

    /// An independent per-frame model of the edited audio, written straight from the spec.
    fn reference(src: &SampleSource, list: &EditList) -> Vec<f32> {
        let mut all = Vec::new();
        src.read(0, src.frames() as usize, &mut all);
        let sample = |frame: i64, c: usize| all[frame as usize * 2 + c] as f64;
        let segs = list.segments();
        let starts = list.starts();
        let trim = list.trim();
        let frames = list.frames();
        let mut out = Vec::new();
        for f in 0..frames {
            let u = trim.start + f;
            let k = (0..segs.len()).rev().find(|&k| starts[k] <= u).unwrap();
            let seg = &segs[k];
            let local = u - starts[k];
            let mut v = [0.0; 2];
            for (c, x) in v.iter_mut().enumerate() {
                *x = sample((seg.src_start + local) as i64, c) * seg.gain(local);
            }
            // A crossfade region around the join at the start of this segment or the next.
            for j in [k, k + 1] {
                let Some(xf) = segs.get(j).and_then(|s| s.xfade).filter(|x| x.len > 0) else {
                    continue;
                };
                let (a, b, join) = (&segs[j - 1], &segs[j], starts[j] as i64);
                let d = u as i64 - join;
                let half = (xf.len / 2) as i64;
                if d < -half || d >= half {
                    continue;
                }
                let t = (d + half) as f64 / xf.len as f64;
                let ga = if d < 0 {
                    a.gain((a.len as i64 + d) as u64)
                } else {
                    a.gain(a.len - 1)
                };
                let gb = if d >= 0 { b.gain(d as u64) } else { b.gain(0) };
                for (c, x) in v.iter_mut().enumerate() {
                    *x = sample((a.src_start + a.len) as i64 + d, c)
                        * ga
                        * xf.curve.gain(Dir::Out, t)
                        + sample(b.src_start as i64 + d, c) * gb * xf.curve.gain(Dir::In, t);
                }
            }
            let (fi, fo) = (list.fade_in(), list.fade_out());
            let mut g = 1.0;
            if f < fi.len {
                g *= fi.curve.gain(Dir::In, f as f64 / fi.len as f64);
            }
            if f >= frames - fo.len {
                g *= fo
                    .curve
                    .gain(Dir::Out, (f - (frames - fo.len)) as f64 / fo.len as f64);
            }
            out.extend(v.map(|x| (x * g) as f32));
        }
        out
    }

    fn check(list: EditList) {
        let src = source(100);
        let audio = Audio::with_edits(src.clone(), Arc::new(list.clone()));
        let expected = reference(&src, &list);
        assert_eq!(audio.frames(), list.frames());
        assert_eq!(audio.info().frames, list.frames());
        let close = |got: &[f32]| {
            assert_eq!(got.len(), expected.len(), "{list:?}");
            for (i, (g, w)) in got.iter().zip(&expected).enumerate() {
                assert!((g - w).abs() < 1e-6, "sample {i}: {g} vs {w} in {list:?}");
            }
        };
        // Whole file in one read, and in odd-sized pieces that straddle every boundary.
        let mut out = Vec::new();
        audio.read(0, audio.frames() as usize, &mut out);
        close(&out);
        let mut joined = Vec::new();
        let mut pos = 0;
        while pos < audio.frames() {
            audio.read(pos, 7, &mut out);
            joined.extend_from_slice(&out);
            pos += 7;
        }
        close(&joined);
        // Spans cover the output in order, and copied spans really are source copies.
        let mut next = 0;
        let mut one = Vec::new();
        for s in audio.spans() {
            assert_eq!(s.start, next, "{list:?}");
            next += s.len;
            if let Some(from) = s.src {
                for f in 0..s.len {
                    src.read(from + f, 1, &mut one);
                    let o = (s.start + f) as usize * 2;
                    assert_eq!(
                        &expected[o..o + 2],
                        &one[..],
                        "copy at frame {}",
                        s.start + f
                    );
                }
            }
        }
        assert_eq!(next, audio.frames());
    }

    const LIN: Curve = Curve::LINEAR;

    #[test]
    fn reads_every_kind_of_edit() {
        let id = EditList::identity(100);
        check(id.clone());
        check(id.crop(10, 90).unwrap());
        check(id.delete(30, 45, 0).unwrap());
        check(id.silence(5, 20).unwrap());
        check(
            id.fade_in_range(0, 50, Curve::new(0.6))
                .unwrap()
                .fade_out_range(60, 100, LIN)
                .unwrap(),
        );
        check(
            id.delete(30, 45, 0)
                .unwrap()
                .fade_out_range(20, 40, LIN)
                .unwrap()
                .crop(5, 80)
                .unwrap(),
        );
    }

    #[test]
    fn reads_trims_clip_fades_and_crossfades() {
        let id = EditList::identity(100);
        let f = |len, bend| ClipFade {
            len,
            curve: Curve::new(bend),
        };
        check(id.set_trim(12, 91));
        check(id.set_clip_fades(f(10, 0.0), f(25, -0.8)));
        check(id.set_trim(5, 95).set_clip_fades(f(20, 1.0), f(7, 0.0)));
        // Default-style crossfade, a wide curved one, and a hard join.
        let cut = id.delete(40, 55, 10).unwrap();
        check(cut.clone());
        check(
            cut.set_crossfade(
                1,
                Crossfade {
                    len: 30,
                    curve: Curve::new(0.5),
                },
            )
            .unwrap(),
        );
        check(
            cut.set_crossfade(1, Crossfade { len: 0, curve: LIN })
                .unwrap(),
        );
        // Crossfades next to faded and silent neighbours, with a trim and fades on top.
        check(
            id.fade_out_range(20, 40, LIN)
                .unwrap()
                .silence(60, 80)
                .unwrap()
                .delete(35, 65, 12)
                .unwrap()
                .set_trim(3, 68)
                .set_clip_fades(f(5, 0.0), f(5, 0.0)),
        );
        // Two joins.
        check(id.delete(20, 30, 8).unwrap().delete(50, 60, 8).unwrap());
    }

    #[test]
    fn hard_join_is_a_plain_concatenation() {
        let list = EditList::identity(100)
            .delete(40, 50, 10)
            .unwrap()
            .set_crossfade(1, Crossfade { len: 0, curve: LIN })
            .unwrap();
        let audio = Audio::with_edits(source(100), Arc::new(list));
        assert_eq!(
            audio.spans(),
            [
                Span {
                    start: 0,
                    len: 40,
                    src: Some(0)
                },
                Span {
                    start: 40,
                    len: 50,
                    src: Some(50)
                },
            ]
        );
    }

    #[test]
    fn crossfade_blends_both_sides_at_the_join() {
        let list = EditList::identity(100).delete(40, 50, 10).unwrap();
        let audio = Audio::with_edits(source(100), Arc::new(list));
        let mut out = Vec::new();
        // Output frame 40 is the join: half way, so linear weights of 0.5 each side. The
        // outgoing side has played on into the deleted audio, reaching its first frame
        // (source frame 40); the incoming side is at its own start (source frame 50).
        audio.read(40, 1, &mut out);
        assert!((out[0] as f64 - (0.5 * 0.040 + 0.5 * 0.050)).abs() < 1e-6);
        // Just before the crossfade starts, the audio is untouched.
        audio.read(34, 1, &mut out);
        assert_eq!(out[0], 34.0 / 1000.0);
        assert_eq!(
            audio.spans()[1],
            Span {
                start: 35,
                len: 10,
                src: None
            }
        );
    }

    #[test]
    fn clamps_reads_past_the_end() {
        let audio = Audio::with_edits(
            source(100),
            Arc::new(EditList::identity(100).crop(0, 50).unwrap()),
        );
        let mut out = Vec::new();
        audio.read(45, 20, &mut out);
        assert_eq!(out.len(), 5 * 2);
        audio.read(60, 10, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn full_scale_survives_only_unchanged() {
        // Full-scale ±1.0 everywhere.
        let samples = vec![1.0; 20];
        let src = Arc::new(
            SampleSource::from_bytes(WavBuilder::new(Encoding::Pcm, 16, 1, 8000).build(&samples))
                .unwrap(),
        );
        let list = EditList::identity(20)
            .fade_out_range(10, 20, LIN)
            .unwrap()
            .silence(0, 2)
            .unwrap()
            .delete(4, 6, 0)
            .unwrap();
        let audio = Audio::with_edits(src, Arc::new(list));
        let (mut out, mut fs) = (Vec::new(), Vec::new());
        audio.read_with_full_scale(0, 100, &mut out, &mut fs);
        assert_eq!(out.len(), fs.len());
        // Silenced: no; untouched: yes; first fade frame has gain 1.0 exactly: yes; rest: no.
        let expected: Vec<bool> = (0..18).map(|i| (2..9).contains(&i)).collect();
        assert_eq!(fs, expected);
    }

    #[test]
    fn native_render_matches_normal_render() {
        let src = Arc::new(
            SampleSource::from_bytes(
                WavBuilder::new(Encoding::Pcm, 16, 1, 8000)
                    .build(&[0.5, -0.25, 0.125, 1.0, -1.0, 0.0]),
            )
            .unwrap(),
        );
        let list = EditList::identity(6).fade_in_range(0, 6, LIN).unwrap();
        let audio = Audio::with_edits(src, Arc::new(list));
        let (mut native, mut normal) = (Vec::new(), Vec::new());
        audio.render_native(0, 6, &mut native);
        audio.read(0, 6, &mut normal);
        for (n, f) in native.iter().zip(&normal) {
            assert!((n / 32_768.0 - *f as f64).abs() < 1e-6);
        }
    }
}
