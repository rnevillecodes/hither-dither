//! Memory-mapped sample access, decoding any supported format to `f32` on demand.

use std::fs::File;
use std::path::Path;

use memmap2::Mmap;

use super::{SampleFormat, WavError, WavInfo, parse_header};

enum Backing {
    Mmap(Mmap),
    Owned(Vec<u8>),
}

impl Backing {
    fn bytes(&self) -> &[u8] {
        match self {
            Backing::Mmap(m) => m,
            Backing::Owned(v) => v,
        }
    }
}

/// A parsed WAV file whose samples are decoded lazily from the file bytes.
pub struct SampleSource {
    info: WavInfo,
    backing: Backing,
}

impl SampleSource {
    /// Open and parse a file, mapping it read-only into memory.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, WavError> {
        let file = File::open(path)?;
        if file.metadata()?.len() < 12 {
            return Err(WavError::NotWav);
        }
        // SAFETY: the map is read-only. Modifying the file externally while it is open is
        // unsupported (see design: Risks); playback re-checks the file before starting.
        let map = unsafe { Mmap::map(&file)? };
        let info = parse_header(&map)?;
        Ok(Self {
            info,
            backing: Backing::Mmap(map),
        })
    }

    /// Parse an in-memory file image.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, WavError> {
        let info = parse_header(&bytes)?;
        Ok(Self {
            info,
            backing: Backing::Owned(bytes),
        })
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

    /// The `fmt ` chunk body, for writing a file in the same format.
    pub fn fmt_bytes(&self) -> &[u8] {
        let r = &self.info.fmt_range;
        &self.backing.bytes()[r.start as usize..r.end as usize]
    }

    /// Raw bytes of frames `[start, start + count)`, clamped to the available frames.
    pub fn frame_bytes(&self, start: u64, count: usize) -> &[u8] {
        let start = start.min(self.info.frames);
        let end = start.saturating_add(count as u64).min(self.info.frames);
        let ba = self.info.block_align() as u64;
        let off = self.info.data_offset;
        &self.backing.bytes()[(off + start * ba) as usize..(off + end * ba) as usize]
    }

    /// Decode frames `[start, start + count)` as interleaved `f32` into `out` (replacing its
    /// contents). Requests past the end are clamped, so `out` may be shorter than asked.
    pub fn read(&self, start: u64, count: usize, out: &mut Vec<f32>) {
        self.decode(start, count, out, None);
    }

    /// Like [`read`](Self::read), and also set `full_scale[i]` when sample `i` is at or
    /// beyond full scale in the raw domain (integer min/max, or |x| >= 1.0 for float).
    pub fn read_with_full_scale(
        &self,
        start: u64,
        count: usize,
        out: &mut Vec<f32>,
        full_scale: &mut Vec<bool>,
    ) {
        self.decode(start, count, out, Some(full_scale));
    }

    /// Frames `[start, start + count)` as interleaved values in the file's own units:
    /// integer sample values for PCM (8-bit re-centred on zero), float values for IEEE
    /// float. Used to write edited audio without losing precision.
    pub fn read_native(&self, start: u64, count: usize, out: &mut Vec<f64>) {
        let bytes = self.frame_bytes(start, count);
        out.clear();
        match self.info.format {
            SampleFormat::U8 => out.extend(bytes.iter().map(|&b| b as f64 - 128.0)),
            SampleFormat::I16 => out.extend(
                bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|b| i16::from_le_bytes(*b) as f64),
            ),
            SampleFormat::I24 => out.extend(
                bytes
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .map(|b| (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f64),
            ),
            SampleFormat::I32 => out.extend(
                bytes
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|b| i32::from_le_bytes(*b) as f64),
            ),
            SampleFormat::F32 => out.extend(
                bytes
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|b| f32::from_le_bytes(*b) as f64),
            ),
            SampleFormat::F64 => out.extend(
                bytes
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .map(|b| f64::from_le_bytes(*b)),
            ),
        }
    }

    fn decode(&self, start: u64, count: usize, out: &mut Vec<f32>, fs: Option<&mut Vec<bool>>) {
        let bytes = self.frame_bytes(start, count);
        out.clear();
        let info = &self.info;
        // Integer formats only; float formats ignore these limits.
        let container = (info.format.bytes() as u32 * 8).min(32);
        let valid = (info.valid_bits as u32).min(container);
        let limits = IntLimits::new(valid, container - valid);
        match info.format {
            SampleFormat::U8 => {
                decode_int::<1>(bytes, |b| b[0] as i32 - 128, 1.0 / 128.0, limits, out, fs)
            }
            SampleFormat::I16 => decode_int::<2>(
                bytes,
                |b| i16::from_le_bytes(*b) as i32,
                1.0 / 32_768.0,
                limits,
                out,
                fs,
            ),
            SampleFormat::I24 => decode_int::<3>(
                bytes,
                |b| i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8,
                1.0 / 8_388_608.0,
                limits,
                out,
                fs,
            ),
            SampleFormat::I32 => decode_int::<4>(
                bytes,
                |b| i32::from_le_bytes(*b),
                1.0 / 2_147_483_648.0,
                limits,
                out,
                fs,
            ),
            SampleFormat::F32 => {
                decode_float::<4>(bytes, |b| f32::from_le_bytes(*b), out, fs);
            }
            SampleFormat::F64 => {
                decode_float::<8>(bytes, |b| f64::from_le_bytes(*b) as f32, out, fs);
            }
        }
    }
}

/// Full-scale integer values after removing unused low-order bits.
#[derive(Clone, Copy)]
struct IntLimits {
    shift: u32,
    min: i32,
    max: i32,
}

impl IntLimits {
    fn new(valid_bits: u32, shift: u32) -> Self {
        let max = ((1i64 << (valid_bits - 1)) - 1) as i32;
        Self {
            shift,
            min: -max - 1,
            max,
        }
    }

    #[inline(always)]
    fn is_full_scale(self, v: i32) -> bool {
        let v = v >> self.shift;
        v >= self.max || v <= self.min
    }
}

#[inline(always)]
fn decode_int<const N: usize>(
    bytes: &[u8],
    get: impl Fn(&[u8; N]) -> i32,
    scale: f32,
    limits: IntLimits,
    out: &mut Vec<f32>,
    fs: Option<&mut Vec<bool>>,
) {
    let samples = bytes.as_chunks::<N>().0.iter().map(&get);
    match fs {
        None => out.extend(samples.map(|v| v as f32 * scale)),
        Some(fs) => {
            fs.clear();
            for v in samples {
                out.push(v as f32 * scale);
                fs.push(limits.is_full_scale(v));
            }
        }
    }
}

#[inline(always)]
fn decode_float<const N: usize>(
    bytes: &[u8],
    get: impl Fn(&[u8; N]) -> f32,
    out: &mut Vec<f32>,
    fs: Option<&mut Vec<bool>>,
) {
    let samples = bytes.as_chunks::<N>().0.iter().map(&get);
    match fs {
        None => out.extend(samples),
        Some(fs) => {
            fs.clear();
            for v in samples {
                out.push(v);
                fs.push(v.abs() >= 1.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wav::synth::{Encoding, WavBuilder};

    const INPUT: [f64; 8] = [0.0, 0.5, -0.5, 0.25, -0.75, 0.999, -1.0, 1.0];

    fn source(enc: Encoding, bits: u16, channels: u16, samples: &[f64]) -> SampleSource {
        SampleSource::from_bytes(WavBuilder::new(enc, bits, channels, 48_000).build(samples))
            .unwrap()
    }

    #[test]
    fn decodes_every_format() {
        for (enc, bits, tol) in [
            (Encoding::Pcm, 8, 1.0 / 128.0),
            (Encoding::Pcm, 16, 1.0 / 32_768.0),
            (Encoding::Pcm, 24, 1.0 / 8_388_608.0),
            (Encoding::Pcm, 32, 1e-7),
            (Encoding::Float, 32, 1e-7),
            (Encoding::Float, 64, 1e-7),
        ] {
            let src = source(enc, bits, 2, &INPUT);
            let mut out = Vec::new();
            src.read(0, 4, &mut out);
            assert_eq!(out.len(), 8);
            for (got, want) in out.iter().zip(INPUT) {
                assert!(
                    (*got as f64 - want).abs() <= tol,
                    "{enc:?} {bits}: got {got}, want {want}"
                );
            }
        }
    }

    #[test]
    fn decodes_24_bit_exactly() {
        // Every representable 24-bit step must survive decoding unchanged.
        let raw: Vec<i32> = vec![0, 1, -1, 8_388_607, -8_388_608, 123_456, -654_321];
        let input: Vec<f64> = raw.iter().map(|&v| v as f64 / 8_388_608.0).collect();
        let src = source(Encoding::Pcm, 24, 1, &input);
        let mut out = Vec::new();
        src.read(0, raw.len(), &mut out);
        for (got, &want) in out.iter().zip(&raw) {
            assert_eq!((*got as f64 * 8_388_608.0).round() as i32, want);
        }
    }

    #[test]
    fn reads_arbitrary_ranges_and_clamps() {
        let input: Vec<f64> = (0..20).map(|i| i as f64 / 100.0).collect();
        let src = source(Encoding::Pcm, 16, 2, &input);
        let mut out = Vec::new();
        src.read(3, 2, &mut out);
        assert_eq!(out.len(), 4);
        assert!((out[0] - 0.06).abs() < 1e-4 && (out[3] - 0.09).abs() < 1e-4);
        src.read(8, 100, &mut out);
        assert_eq!(out.len(), 4);
        src.read(50, 10, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn full_scale_is_detected_in_raw_domain() {
        let input = [0.5, 1.0, -1.0, 0.999_999, 1.5, -0.2];
        for (enc, bits, want) in [
            (Encoding::Pcm, 8, [false, true, true, true, true, false]),
            (Encoding::Pcm, 16, [false, true, true, true, true, false]),
            (Encoding::Pcm, 24, [false, true, true, false, true, false]),
            (Encoding::Pcm, 32, [false, true, true, false, true, false]),
            (Encoding::Float, 32, [false, true, true, false, true, false]),
            (Encoding::Float, 64, [false, true, true, false, true, false]),
        ] {
            let src = source(enc, bits, 1, &input);
            let (mut out, mut fs) = (Vec::new(), Vec::new());
            src.read_with_full_scale(0, 6, &mut out, &mut fs);
            assert_eq!(fs, want, "{enc:?} {bits}");
        }
        // A positive full-scale 16-bit sample is below 1.0 once normalised.
        let src = source(Encoding::Pcm, 16, 1, &[1.0]);
        let mut out = Vec::new();
        src.read(0, 1, &mut out);
        assert!(out[0] < 1.0);
    }

    #[test]
    fn opens_from_disk_via_mmap() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        WavBuilder::new(Encoding::Float, 32, 1, 8_000)
            .write(&path, &[0.1, 0.2, 0.3])
            .unwrap();
        let src = SampleSource::open(&path).unwrap();
        let mut out = Vec::new();
        src.read(0, 3, &mut out);
        assert_eq!(out, [0.1, 0.2, 0.3]);

        let junk = dir.path().join("junk.wav");
        std::fs::write(&junk, b"hello").unwrap();
        assert!(matches!(SampleSource::open(&junk), Err(WavError::NotWav)));
    }

    #[test]
    fn truncated_file_reads_whole_frames_only() {
        let mut bytes = WavBuilder::new(Encoding::Pcm, 16, 2, 8_000).build(&[0.5; 20]);
        bytes.truncate(bytes.len() - 3);
        let src = SampleSource::from_bytes(bytes).unwrap();
        assert!(src.info().truncated);
        assert_eq!(src.frames(), 9);
        let mut out = Vec::new();
        src.read(0, 100, &mut out);
        assert_eq!(out.len(), 18);
        assert!(out.iter().all(|&v| (v - 0.5).abs() < 1e-4));
    }
}
