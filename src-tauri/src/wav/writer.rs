//! Writing audio as a WAV file in its source's format: the source's `fmt ` chunk and a
//! `data` chunk, nothing else.

use std::io::{self, Write};

use super::{SampleFormat, WavInfo};
use crate::edit::Audio;

/// Frames converted per write.
const CHUNK_FRAMES: u64 = 65_536;

#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("the result is too large for a WAV file (over 4 GiB)")]
    TooLarge,
    #[error("{0}")]
    Io(#[from] io::Error),
}

/// The RIFF chunk size for a `fmt ` body of `fmt_bytes` and `data_bytes` of samples, or
/// `None` if it doesn't fit the format's 32-bit size field.
fn riff_size(fmt_bytes: u64, data_bytes: u64) -> Option<u32> {
    let chunk = |body: u64| 8 + body + (body & 1);
    u32::try_from(4 + chunk(fmt_bytes) + chunk(data_bytes)).ok()
}

/// Fail early, before creating any file, if `audio` can't be written as a WAV file.
pub fn check_size(audio: &Audio) -> Result<(), WriteError> {
    let data = audio.frames() * audio.info().block_align() as u64;
    riff_size(audio.source().fmt_bytes().len() as u64, data)
        .map(|_| ())
        .ok_or(WriteError::TooLarge)
}

/// Frames as the writer takes them.
pub enum Frames<'a> {
    /// The source file's own bytes, copied as they are.
    Bytes(&'a [u8]),
    /// Interleaved samples in the file's own units (see `Audio::render_native`), encoded with
    /// integers rounded and clamped.
    Native(&'a [f64]),
}

/// Audio the writer can save: its format, and its frames in the file's own units.
pub trait NativeFrames {
    /// The format, with `frames` the number of frames to write.
    fn info(&self) -> &WavInfo;

    /// The body of the `fmt ` chunk to write.
    fn fmt_bytes(&self) -> &[u8];

    /// Hands every frame to `sink`, in order, stopping at its first error.
    fn emit(
        &self,
        sink: &mut dyn FnMut(Frames<'_>) -> Result<(), WriteError>,
    ) -> Result<(), WriteError>;
}

/// Unchanged samples are copied byte for byte; changed ones (edits, fades, crossfades) are
/// rendered in the file's own units.
impl NativeFrames for Audio {
    fn info(&self) -> &WavInfo {
        Audio::info(self)
    }

    fn fmt_bytes(&self) -> &[u8] {
        self.source().fmt_bytes()
    }

    fn emit(
        &self,
        sink: &mut dyn FnMut(Frames<'_>) -> Result<(), WriteError>,
    ) -> Result<(), WriteError> {
        let src = self.source();
        let mut rendered = Vec::new();
        for span in self.spans().iter().copied() {
            let mut done = 0;
            while done < span.len {
                let n = (span.len - done).min(CHUNK_FRAMES);
                match span.src {
                    Some(from) => sink(Frames::Bytes(src.frame_bytes(from + done, n as usize)))?,
                    None => {
                        self.render_native(span.start + done, n as usize, &mut rendered);
                        sink(Frames::Native(&rendered))?;
                    }
                }
                done += n;
            }
        }
        Ok(())
    }
}

impl<T: NativeFrames + ?Sized> NativeFrames for std::sync::Arc<T> {
    fn info(&self) -> &WavInfo {
        (**self).info()
    }

    fn fmt_bytes(&self) -> &[u8] {
        (**self).fmt_bytes()
    }

    fn emit(
        &self,
        sink: &mut dyn FnMut(Frames<'_>) -> Result<(), WriteError>,
    ) -> Result<(), WriteError> {
        (**self).emit(sink)
    }
}

/// Write `audio` as a complete WAV file.
pub fn write_wav(audio: &impl NativeFrames, out: impl Write) -> Result<(), WriteError> {
    write_wav_with_progress(audio, out, |_| {})
}

/// Like [`write_wav`], calling `progress` with the number of frames written so far.
pub fn write_wav_with_progress(
    audio: &impl NativeFrames,
    out: impl Write,
    mut progress: impl FnMut(u64),
) -> Result<(), WriteError> {
    let info = audio.info();
    let fmt = audio.fmt_bytes();
    let data_bytes = info.frames * info.block_align() as u64;
    let riff = riff_size(fmt.len() as u64, data_bytes).ok_or(WriteError::TooLarge)?;

    let mut out = io::BufWriter::with_capacity(1 << 20, out);
    out.write_all(b"RIFF")?;
    out.write_all(&riff.to_le_bytes())?;
    out.write_all(b"WAVE")?;
    out.write_all(b"fmt ")?;
    out.write_all(&(fmt.len() as u32).to_le_bytes())?;
    out.write_all(fmt)?;
    if fmt.len() % 2 == 1 {
        out.write_all(&[0])?;
    }
    out.write_all(b"data")?;
    out.write_all(&(data_bytes as u32).to_le_bytes())?;

    let (block, channels) = (info.block_align(), info.channels as usize);
    let mut encoded = Vec::new();
    let mut written = 0;
    audio.emit(&mut |frames| {
        match frames {
            Frames::Bytes(bytes) => {
                out.write_all(bytes)?;
                written += (bytes.len() / block) as u64;
            }
            Frames::Native(samples) => {
                encoded.clear();
                encode(info, samples, &mut encoded);
                out.write_all(&encoded)?;
                written += (samples.len() / channels) as u64;
            }
        }
        progress(written);
        Ok(())
    })?;
    if data_bytes % 2 == 1 {
        out.write_all(&[0])?;
    }
    out.flush()?;
    Ok(())
}

/// Converts normalised samples to the values a file's format stores, and back.
#[derive(Debug, Clone, Copy)]
pub struct Quantiser {
    float: bool,
    /// Native units per full scale.
    scale: f64,
    /// Native units per step of the significant bits.
    step: f64,
    /// Range of the significant bits, in steps.
    min: f64,
    max: f64,
}

impl Quantiser {
    pub fn new(info: &WavInfo) -> Quantiser {
        let container = (info.format.bytes() as u32 * 8).min(32);
        let valid = (info.valid_bits as u32).clamp(1, container);
        let max = ((1i64 << (valid - 1)) - 1) as f64;
        Quantiser {
            float: info.format.is_float(),
            scale: (1u64 << (container - 1)) as f64,
            step: (1u64 << (container - valid)) as f64,
            min: -max - 1.0,
            max,
        }
    }

    /// Normalised sample `x` in the file's own units as it will be stored (integers rounded
    /// on the grid of significant bits and clamped), and whether it had to be clamped. NaN is
    /// stored as silence.
    pub fn quantise(&self, x: f32) -> (f64, bool) {
        if self.float {
            return (x as f64, false);
        }
        let q = (x as f64 * self.scale / self.step).round();
        if q.is_nan() {
            return (0.0, false);
        }
        let clamped = q.clamp(self.min, self.max);
        (clamped * self.step, clamped != q)
    }

    /// A stored value as reading the file gives it back: normalised, and whether it is at
    /// full scale.
    pub fn normalise(&self, v: f64) -> (f32, bool) {
        if self.float {
            let x = v as f32;
            return (x, x.abs() >= 1.0);
        }
        let q = v / self.step;
        let x = (v as i32) as f32 * (1.0 / self.scale) as f32;
        (x, q >= self.max || q <= self.min)
    }
}

/// Encode samples in the file's own units (see `Audio::render_native`). Integers are
/// rounded on the grid of significant bits, so the unused low bits stay zero.
fn encode(info: &WavInfo, samples: &[f64], out: &mut Vec<u8>) {
    let container = (info.format.bytes() as u32 * 8).min(32);
    let valid = (info.valid_bits as u32).clamp(1, container);
    let shift = container - valid;
    let step = (1u64 << shift) as f64;
    let max = (1i64 << (valid - 1)) - 1;
    let min = -max - 1;
    for &v in samples {
        match info.format {
            SampleFormat::F32 => out.extend_from_slice(&(v as f32).to_le_bytes()),
            SampleFormat::F64 => out.extend_from_slice(&v.to_le_bytes()),
            format => {
                let q = (v / step).round().clamp(min as f64, max as f64) as i64;
                push_int(format, q << shift, out);
            }
        }
    }
}

fn push_int(format: SampleFormat, v: i64, out: &mut Vec<u8>) {
    match format {
        SampleFormat::U8 => out.push((v + 128) as u8),
        SampleFormat::I16 => out.extend_from_slice(&(v as i16).to_le_bytes()),
        SampleFormat::I24 => out.extend_from_slice(&(v as i32).to_le_bytes()[..3]),
        SampleFormat::I32 => out.extend_from_slice(&(v as i32).to_le_bytes()),
        SampleFormat::F32 | SampleFormat::F64 => unreachable!("float samples are not integers"),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::edit::{Crossfade, Curve, EditList};
    use crate::wav::synth::{Encoding, WavBuilder};
    use crate::wav::{SampleSource, parse_header};

    const FORMATS: [(Encoding, u16); 6] = [
        (Encoding::Pcm, 8),
        (Encoding::Pcm, 16),
        (Encoding::Pcm, 24),
        (Encoding::Pcm, 32),
        (Encoding::Float, 32),
        (Encoding::Float, 64),
    ];

    const LIN: Curve = Curve::LINEAR;

    /// Stereo test signal with full-scale peaks: 50 frames.
    fn samples() -> Vec<f64> {
        (0..100)
            .map(|i| match i % 7 {
                0 => 1.0,
                1 => -1.0,
                _ => ((i * 37) % 200) as f64 / 100.0 - 1.0,
            })
            .collect()
    }

    fn source(b: WavBuilder) -> Arc<SampleSource> {
        Arc::new(SampleSource::from_bytes(b.build(&samples())).unwrap())
    }

    fn write(src: &Arc<SampleSource>, list: EditList) -> Vec<u8> {
        let mut out = Vec::new();
        write_wav(&Audio::with_edits(src.clone(), Arc::new(list)), &mut out).unwrap();
        out
    }

    fn data(bytes: &[u8]) -> &[u8] {
        let i = parse_header(bytes).unwrap();
        let start = i.data_offset as usize;
        &bytes[start..start + i.frames as usize * i.block_align()]
    }

    fn chunk_ids(bytes: &[u8]) -> Vec<[u8; 4]> {
        let mut ids = Vec::new();
        let mut pos = 12;
        while pos + 8 <= bytes.len() {
            ids.push(bytes[pos..pos + 4].try_into().unwrap());
            let size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
            pos += 8 + size + (size & 1);
        }
        ids
    }

    #[test]
    fn untouched_samples_are_bit_exact_outside_the_crossfade() {
        for (enc, bits) in FORMATS {
            let src = source(WavBuilder::new(enc, bits, 2, 44_100));
            // A 4-frame crossfade covers output frames 8..12 around the join at 10.
            let out = write(&src, EditList::identity(50).delete(10, 20, 4).unwrap());
            let ba = src.info().block_align();
            let orig = src.frame_bytes(0, 50);
            let got = data(&out);
            assert_eq!(&got[..8 * ba], &orig[..8 * ba], "{enc:?} {bits} before");
            assert_eq!(&got[12 * ba..], &orig[22 * ba..], "{enc:?} {bits} after");
        }
    }

    #[test]
    fn hard_join_is_bit_exact() {
        for (enc, bits) in FORMATS {
            let src = source(WavBuilder::new(enc, bits, 2, 44_100));
            let list = EditList::identity(50)
                .delete(10, 20, 4)
                .unwrap()
                .set_crossfade(1, Crossfade { len: 0, curve: LIN })
                .unwrap();
            let out = write(&src, list);
            let ba = src.info().block_align();
            let orig = src.frame_bytes(0, 50);
            let expected = [&orig[..10 * ba], &orig[20 * ba..]].concat();
            assert_eq!(data(&out), expected, "{enc:?} {bits}");
        }
    }

    #[test]
    fn edited_file_keeps_the_format() {
        for (enc, bits) in FORMATS {
            for extensible in [false, true] {
                let src = source(WavBuilder::new(enc, bits, 2, 48_000).extensible(extensible));
                let list = EditList::identity(50)
                    .fade_in_range(0, 20, Curve::new(0.4))
                    .unwrap()
                    .silence(30, 35)
                    .unwrap()
                    .delete(36, 40, 4)
                    .unwrap()
                    .crop(5, 45)
                    .unwrap();
                let audio = Audio::with_edits(src.clone(), Arc::new(list.clone()));
                let out = write(&src, list);
                let a = src.info();
                let b = parse_header(&out).unwrap();
                assert_eq!(
                    (
                        b.format,
                        b.valid_bits,
                        b.channels,
                        b.sample_rate,
                        b.channel_mask
                    ),
                    (
                        a.format,
                        a.valid_bits,
                        a.channels,
                        a.sample_rate,
                        a.channel_mask
                    ),
                    "{enc:?} {bits} ext={extensible}"
                );
                assert_eq!(b.frames, 40);
                assert_eq!(
                    &out[b.fmt_range.start as usize..b.fmt_range.end as usize],
                    src.fmt_bytes()
                );
                // Decoded output matches the edited audio to within one step of the format.
                let reopened = SampleSource::from_bytes(out).unwrap();
                let (mut got, mut want) = (Vec::new(), Vec::new());
                reopened.read(0, 40, &mut got);
                audio.read(0, 40, &mut want);
                let step = if enc == Encoding::Float {
                    1e-6
                } else {
                    2f32.powi(1 - bits as i32).max(1e-6)
                };
                for (g, w) in got.iter().zip(&want) {
                    assert!((g - w).abs() <= step, "{enc:?} {bits}: {g} vs {w}");
                }
            }
        }
    }

    #[test]
    fn f64_fades_and_crossfades_keep_full_precision() {
        let src = source(WavBuilder::new(Encoding::Float, 64, 2, 48_000));
        let list = EditList::identity(50)
            .fade_out_range(0, 30, Curve::new(-0.3))
            .unwrap()
            .delete(30, 34, 6)
            .unwrap();
        let audio = Audio::with_edits(src.clone(), Arc::new(list.clone()));
        let out = write(&src, list);
        let mut want = Vec::new();
        audio.render_native(0, audio.frames() as usize, &mut want);
        for (n, w) in data(&out).as_chunks::<8>().0.iter().zip(&want) {
            assert!((f64::from_le_bytes(*n) - w).abs() < 1e-12);
        }
        // The native render really is in f64: it isn't rounded to f32 precision.
        assert!(want.iter().any(|&w| (w as f32) as f64 != w));
    }

    #[test]
    fn metadata_chunks_are_not_written() {
        let src = source(
            WavBuilder::new(Encoding::Pcm, 16, 2, 44_100)
                .chunk(*b"LIST", b"INFOISFT\x05\x00\x00\x00abcde".to_vec())
                .chunk(*b"bext", vec![0; 602]),
        );
        let out = write(&src, EditList::identity(50).silence(0, 5).unwrap());
        assert_eq!(chunk_ids(&out), [*b"fmt ", *b"data"]);
    }

    #[test]
    fn eight_bit_silence_and_odd_length_padding() {
        let src = source(WavBuilder::new(Encoding::Pcm, 8, 1, 8_000));
        // 100 mono frames; keep an odd number so the data chunk needs a pad byte.
        let out = write(
            &src,
            EditList::identity(100)
                .crop(0, 51)
                .unwrap()
                .silence(0, 3)
                .unwrap(),
        );
        assert_eq!(&data(&out)[..3], [0x80; 3]);
        assert_eq!(out.len() % 2, 0);
        let riff = u32::from_le_bytes(out[4..8].try_into().unwrap()) as usize;
        assert_eq!(riff, out.len() - 8);
        assert_eq!(parse_header(&out).unwrap().frames, 51);
    }

    #[test]
    fn changed_samples_stay_on_the_valid_bit_grid() {
        let mut bytes = WavBuilder::new(Encoding::Pcm, 24, 2, 48_000)
            .extensible(true)
            .build(&samples());
        // Declare 20 valid bits (wValidBitsPerSample) in the 24-bit container.
        let fmt = parse_header(&bytes).unwrap().fmt_range.start as usize;
        bytes[fmt + 18..fmt + 20].copy_from_slice(&20u16.to_le_bytes());
        let src = Arc::new(SampleSource::from_bytes(bytes).unwrap());
        let list = EditList::identity(50)
            .fade_in_range(0, 50, LIN)
            .unwrap()
            .delete(20, 30, 8)
            .unwrap();
        let out = write(&src, list);
        for s in data(&out).as_chunks::<3>().0 {
            assert_eq!(s[0] & 0x0F, 0, "low 4 bits must stay zero");
        }
    }

    #[test]
    fn refuses_results_over_4_gib() {
        assert_eq!(riff_size(16, 1000), Some(4 + 24 + 1008));
        assert!(riff_size(16, u32::MAX as u64 - 37).is_some());
        assert!(riff_size(16, u32::MAX as u64 - 35).is_none());
        assert!(riff_size(40, 5_000_000_000).is_none());
    }
}
