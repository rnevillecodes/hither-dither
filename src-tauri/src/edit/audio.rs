//! Reading edited audio: maps frame ranges of the edit list onto the source.

use std::sync::Arc;

use super::EditList;
use crate::wav::{SampleSource, WavError, WavInfo};

/// A source as seen through an edit list. Peaks, stats and playback read through this, so
/// they always see the edited audio.
pub struct Audio {
    source: Arc<SampleSource>,
    edits: Arc<EditList>,
    /// The source's info with `frames` set to the edited length.
    info: WavInfo,
    /// Output frame at which each segment starts.
    starts: Vec<u64>,
    identity: bool,
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
        let mut starts = Vec::with_capacity(edits.segments().len());
        let mut pos = 0;
        for s in edits.segments() {
            starts.push(pos);
            pos += s.len;
        }
        Self {
            identity: edits.is_identity(),
            source,
            edits,
            info,
            starts,
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
            self.source.read(start, count, out);
        } else {
            self.decode(start, count, out, None);
        }
    }

    /// Like [`read`](Self::read), and also set `full_scale[i]` when sample `i` is at full
    /// scale in the source and was not scaled down by an edit.
    pub fn read_with_full_scale(
        &self,
        start: u64,
        count: usize,
        out: &mut Vec<f32>,
        full_scale: &mut Vec<bool>,
    ) {
        if self.identity {
            self.source
                .read_with_full_scale(start, count, out, full_scale);
        } else {
            self.decode(start, count, out, Some(full_scale));
        }
    }

    fn decode(&self, start: u64, count: usize, out: &mut Vec<f32>, mut fs: Option<&mut Vec<bool>>) {
        out.clear();
        if let Some(fs) = fs.as_deref_mut() {
            fs.clear();
        }
        let ch = self.channels();
        let end = start.saturating_add(count as u64).min(self.frames());
        let mut pos = start.min(end);
        let segments = self.edits.segments();
        // The segment containing `pos`.
        let mut i = self.starts.partition_point(|&s| s <= pos).saturating_sub(1);
        let mut buf = Vec::new();
        let mut fbuf = Vec::new();
        while pos < end {
            let seg = &segments[i];
            let local = pos - self.starts[i];
            let n = (seg.len - local).min(end - pos);
            let samples = n as usize * ch;
            if seg.silent {
                out.resize(out.len() + samples, 0.0);
                if let Some(fs) = fs.as_deref_mut() {
                    fs.resize(fs.len() + samples, false);
                }
            } else {
                match fs.as_deref_mut() {
                    Some(fs) => {
                        self.source.read_with_full_scale(
                            seg.src_start + local,
                            n as usize,
                            &mut buf,
                            &mut fbuf,
                        );
                        if seg.ramps.is_empty() {
                            fs.extend_from_slice(&fbuf);
                        }
                    }
                    None => self
                        .source
                        .read(seg.src_start + local, n as usize, &mut buf),
                }
                if seg.ramps.is_empty() {
                    out.extend_from_slice(&buf);
                } else {
                    for (f, frame) in buf.chunks_exact(ch).enumerate() {
                        let g = seg.gain(local + f as u64);
                        out.extend(frame.iter().map(|&v| (v as f64 * g) as f32));
                        if let Some(fs) = fs.as_deref_mut() {
                            let unity = g == 1.0;
                            fs.extend(fbuf[f * ch..(f + 1) * ch].iter().map(|&b| b && unity));
                        }
                    }
                }
            }
            pos += n;
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    /// Expected output: (source frame, gain) for each output frame.
    fn reference(src: &SampleSource, list: &EditList) -> Vec<f32> {
        let mut all = Vec::new();
        src.read(0, src.frames() as usize, &mut all);
        let mut out = Vec::new();
        for s in list.segments() {
            for i in 0..s.len {
                let g = s.gain(i);
                let f = (s.src_start + i) as usize;
                out.extend([all[2 * f], all[2 * f + 1]].map(|v| (v as f64 * g) as f32));
            }
        }
        out
    }

    fn check(list: EditList) {
        let src = source(100);
        let audio = Audio::with_edits(src.clone(), Arc::new(list.clone()));
        let expected = reference(&src, &list);
        assert_eq!(audio.frames(), list.frames());
        assert_eq!(audio.info().frames, list.frames());
        // Whole file in one read, and in odd-sized pieces that straddle segment boundaries.
        let mut out = Vec::new();
        audio.read(0, audio.frames() as usize, &mut out);
        assert_eq!(out, expected);
        let mut joined = Vec::new();
        let mut pos = 0;
        while pos < audio.frames() {
            audio.read(pos, 7, &mut out);
            joined.extend_from_slice(&out);
            pos += 7;
        }
        assert_eq!(joined, expected);
    }

    #[test]
    fn reads_every_kind_of_edit() {
        let id = EditList::identity(100);
        check(id.clone());
        check(id.crop(10, 90).unwrap());
        check(id.delete(30, 45).unwrap());
        check(id.silence(5, 20).unwrap());
        check(id.fade_in(0, 50).unwrap().fade_out(60, 100).unwrap());
        check(
            id.delete(30, 45)
                .unwrap()
                .fade_out(20, 40)
                .unwrap()
                .crop(5, 80)
                .unwrap(),
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
    fn full_scale_survives_only_at_unity_gain() {
        // Full-scale ±1.0 everywhere.
        let samples = vec![1.0; 20];
        let src = Arc::new(
            SampleSource::from_bytes(WavBuilder::new(Encoding::Pcm, 16, 1, 8000).build(&samples))
                .unwrap(),
        );
        let list = EditList::identity(20)
            .fade_out(10, 20)
            .unwrap()
            .silence(0, 2)
            .unwrap()
            .delete(4, 6)
            .unwrap();
        let audio = Audio::with_edits(src, Arc::new(list));
        let (mut out, mut fs) = (Vec::new(), Vec::new());
        audio.read_with_full_scale(0, 100, &mut out, &mut fs);
        assert_eq!(out.len(), fs.len());
        // Silenced: no; untouched: yes; first fade frame has gain 1.0 exactly: yes; rest: no.
        let expected: Vec<bool> = (0..18).map(|i| (2..9).contains(&i)).collect();
        assert_eq!(fs, expected);
    }
}
