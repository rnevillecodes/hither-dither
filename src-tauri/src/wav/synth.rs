//! Synthetic WAV writer used by tests and the fixture generator.
//!
//! Samples are given as interleaved normalised `f64` values and encoded into the requested
//! format. Integer encodings round and clamp, so `1.0` becomes the maximum integer value.

use std::f64::consts::TAU;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Pcm,
    Float,
    /// Any other format tag (e.g. 0x0011 IMA ADPCM). Sample data is written as zeros.
    Tag(u16),
}

#[derive(Debug, Clone)]
pub struct WavBuilder {
    encoding: Encoding,
    bits: u16,
    channels: u16,
    sample_rate: u32,
    extensible: bool,
    chunks: Vec<([u8; 4], Vec<u8>)>,
}

impl WavBuilder {
    pub fn new(encoding: Encoding, bits: u16, channels: u16, sample_rate: u32) -> Self {
        Self {
            encoding,
            bits,
            channels,
            sample_rate,
            extensible: false,
            chunks: Vec::new(),
        }
    }

    /// Write a `WAVE_FORMAT_EXTENSIBLE` header with the encoding as its subformat.
    pub fn extensible(mut self, on: bool) -> Self {
        self.extensible = on;
        self
    }

    /// Add a chunk between `fmt ` and `data`. Odd-sized bodies get a pad byte.
    pub fn chunk(mut self, id: [u8; 4], body: Vec<u8>) -> Self {
        self.chunks.push((id, body));
        self
    }

    fn bytes_per_sample(&self) -> usize {
        (self.bits as usize).div_ceil(8)
    }

    fn tag(&self) -> u16 {
        match self.encoding {
            Encoding::Pcm => 1,
            Encoding::Float => 3,
            Encoding::Tag(t) => t,
        }
    }

    /// Encode interleaved samples into a complete file image.
    pub fn build(&self, samples: &[f64]) -> Vec<u8> {
        let block_align = (self.bytes_per_sample() * self.channels as usize) as u16;
        let mut fmt = Vec::new();
        let tag = if self.extensible { 0xFFFE } else { self.tag() };
        fmt.extend_from_slice(&tag.to_le_bytes());
        fmt.extend_from_slice(&self.channels.to_le_bytes());
        fmt.extend_from_slice(&self.sample_rate.to_le_bytes());
        fmt.extend_from_slice(&(self.sample_rate * block_align as u32).to_le_bytes());
        fmt.extend_from_slice(&block_align.to_le_bytes());
        fmt.extend_from_slice(&self.bits.to_le_bytes());
        if self.extensible {
            let mask: u32 = match self.channels {
                1 => 0x4,
                2 => 0x3,
                6 => 0x3F,
                n if n < 32 => (1u32 << n) - 1,
                _ => 0,
            };
            fmt.extend_from_slice(&22u16.to_le_bytes());
            fmt.extend_from_slice(&self.bits.to_le_bytes());
            fmt.extend_from_slice(&mask.to_le_bytes());
            fmt.extend_from_slice(&self.tag().to_le_bytes());
            fmt.extend_from_slice(&[
                0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
            ]);
        }

        let mut data = Vec::with_capacity(samples.len() * self.bytes_per_sample());
        for &x in samples {
            self.encode(x, &mut data);
        }

        let mut out = Vec::with_capacity(data.len() + 128);
        out.extend_from_slice(b"RIFF\0\0\0\0WAVE");
        push_chunk(&mut out, *b"fmt ", &fmt);
        for (id, body) in &self.chunks {
            push_chunk(&mut out, *id, body);
        }
        push_chunk(&mut out, *b"data", &data);
        let riff_size = (out.len() - 8) as u32;
        out[4..8].copy_from_slice(&riff_size.to_le_bytes());
        out
    }

    pub fn write(&self, path: impl AsRef<Path>, samples: &[f64]) -> std::io::Result<()> {
        std::fs::write(path, self.build(samples))
    }

    fn encode(&self, x: f64, out: &mut Vec<u8>) {
        let int = |scale: f64, min: f64, max: f64| (x * scale).round().clamp(min, max) as i64;
        match (self.encoding, self.bits) {
            (Encoding::Pcm, 8) => out.push(int(128.0, -128.0, 127.0).wrapping_add(128) as u8),
            (Encoding::Pcm, 16) => {
                out.extend_from_slice(&(int(32768.0, -32768.0, 32767.0) as i16).to_le_bytes())
            }
            (Encoding::Pcm, 24) => {
                let v = int(8_388_608.0, -8_388_608.0, 8_388_607.0) as i32;
                out.extend_from_slice(&v.to_le_bytes()[..3]);
            }
            (Encoding::Pcm, 32) => {
                let v = int(2_147_483_648.0, -2_147_483_648.0, 2_147_483_647.0) as i32;
                out.extend_from_slice(&v.to_le_bytes());
            }
            (Encoding::Float, 32) => out.extend_from_slice(&(x as f32).to_le_bytes()),
            (Encoding::Float, 64) => out.extend_from_slice(&x.to_le_bytes()),
            _ => out.extend(std::iter::repeat_n(0, self.bytes_per_sample())),
        }
    }
}

fn push_chunk(out: &mut Vec<u8>, id: [u8; 4], body: &[u8]) {
    out.extend_from_slice(&id);
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
    if body.len() % 2 == 1 {
        out.push(0);
    }
}

/// `frames` samples of a sine wave with peak amplitude `amp`.
pub fn sine(freq: f64, amp: f64, sample_rate: u32, frames: usize) -> Vec<f64> {
    (0..frames)
        .map(|i| amp * (TAU * freq * i as f64 / sample_rate as f64).sin())
        .collect()
}

/// Interleave equal-length channel buffers into frames.
pub fn interleave(channels: &[Vec<f64>]) -> Vec<f64> {
    let frames = channels.iter().map(Vec::len).min().unwrap_or(0);
    let mut out = Vec::with_capacity(frames * channels.len());
    for i in 0..frames {
        out.extend(channels.iter().map(|c| c[i]));
    }
    out
}

/// Convert a dBFS level to linear amplitude.
pub fn db_to_amp(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wav::parse_header;

    #[test]
    fn round_trips_simple_16_bit_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("simple.wav");
        let samples = [0.0, 0.5, -0.5, 1.0, -1.0, 0.25];
        WavBuilder::new(Encoding::Pcm, 16, 2, 44_100)
            .write(&path, &samples)
            .unwrap();

        let bytes = std::fs::read(&path).unwrap();
        let info = parse_header(&bytes).unwrap();
        assert_eq!(
            (info.channels, info.sample_rate, info.frames),
            (2, 44_100, 3)
        );
        let off = info.data_offset as usize;
        let raw: Vec<i16> = bytes[off..]
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect();
        assert_eq!(raw, [0, 16384, -16384, 32767, -32768, 8192]);
    }
}
