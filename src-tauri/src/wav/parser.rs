//! RIFF chunk walking and `fmt ` parsing.

use super::{SampleFormat, WavError};

const WAVE_FORMAT_PCM: u16 = 0x0001;
const WAVE_FORMAT_IEEE_FLOAT: u16 = 0x0003;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;
/// Bytes 2..16 shared by every `KSDATAFORMAT_SUBTYPE_*` GUID derived from a format tag.
const SUBFORMAT_GUID_TAIL: [u8; 14] = [
    0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
];
const MAX_CHANNELS: u16 = 32;
const MAX_SAMPLE_RATE: u32 = 768_000;

/// Header facts needed to decode and describe a WAV file.
#[derive(Debug, Clone, PartialEq)]
pub struct WavInfo {
    pub format: SampleFormat,
    /// Significant bits per sample (may be less than the container, e.g. 20 in 24).
    pub valid_bits: u16,
    pub channels: u16,
    pub sample_rate: u32,
    /// `dwChannelMask` from a `WAVE_FORMAT_EXTENSIBLE` header.
    pub channel_mask: Option<u32>,
    /// Byte offset of the first sample.
    pub data_offset: u64,
    /// Whole frames available in the file.
    pub frames: u64,
    /// The `data` chunk declares more bytes than the file contains.
    pub truncated: bool,
}

impl WavInfo {
    pub fn block_align(&self) -> usize {
        self.format.bytes() * self.channels as usize
    }

    pub fn duration_secs(&self) -> f64 {
        self.frames as f64 / self.sample_rate as f64
    }
}

/// Human-readable name for a WAV format tag, used in error messages.
pub fn format_tag_name(tag: u16) -> String {
    let name = match tag {
        0x0001 => "PCM",
        0x0002 => "Microsoft ADPCM",
        0x0003 => "IEEE float",
        0x0006 => "A-law",
        0x0007 => "µ-law",
        0x0011 => "IMA ADPCM",
        0x0022 => "TrueSpeech",
        0x0031 => "GSM 6.10",
        0x0040 => "G.721 ADPCM",
        0x0050 => "MPEG",
        0x0055 => "MPEG Layer 3",
        0x0092 => "Dolby AC-3 SPDIF",
        0x0161..=0x0163 => "Windows Media Audio",
        0x2000 => "Dolby AC-3",
        _ => return format!("unknown (format tag 0x{tag:04X})"),
    };
    format!("{name} (format tag 0x{tag:04X})")
}

struct Fmt {
    format: SampleFormat,
    valid_bits: u16,
    channels: u16,
    sample_rate: u32,
    channel_mask: Option<u32>,
}

/// Parse a complete WAV file image (normally a memory map). `bytes` must be the whole file
/// so that truncation can be detected.
pub fn parse_header(bytes: &[u8]) -> Result<WavInfo, WavError> {
    if bytes.len() < 12 || &bytes[8..12] != b"WAVE" {
        return Err(WavError::NotWav);
    }
    match &bytes[0..4] {
        b"RIFF" => {}
        b"RF64" => return Err(WavError::UnsupportedEncoding("RF64".into())),
        _ => return Err(WavError::NotWav),
    }

    let mut fmt: Option<Fmt> = None;
    let mut data: Option<(u64, u64)> = None;
    let mut pos: u64 = 12;
    let len = bytes.len() as u64;

    while pos + 8 <= len {
        let p = pos as usize;
        let id: [u8; 4] = bytes[p..p + 4].try_into().unwrap();
        let size = u32::from_le_bytes(bytes[p + 4..p + 8].try_into().unwrap()) as u64;
        let body = pos + 8;
        match &id {
            b"fmt " => {
                if body + size > len {
                    return Err(WavError::Malformed("the 'fmt ' chunk is cut short".into()));
                }
                fmt = Some(parse_fmt(&bytes[body as usize..(body + size) as usize])?);
            }
            b"data" => {
                if data.is_none() {
                    data = Some((body, size));
                }
            }
            _ => {}
        }
        // Chunks are padded to an even length.
        pos = body + size + (size & 1);
    }

    let fmt = fmt.ok_or(WavError::MissingChunk("fmt "))?;
    let (data_offset, declared) = data.ok_or(WavError::MissingChunk("data"))?;
    let available = len.saturating_sub(data_offset);
    let truncated = declared > available;
    let block_align = (fmt.format.bytes() * fmt.channels as usize) as u64;
    let frames = declared.min(available) / block_align;

    Ok(WavInfo {
        format: fmt.format,
        valid_bits: fmt.valid_bits,
        channels: fmt.channels,
        sample_rate: fmt.sample_rate,
        channel_mask: fmt.channel_mask,
        data_offset,
        frames,
        truncated,
    })
}

fn parse_fmt(b: &[u8]) -> Result<Fmt, WavError> {
    if b.len() < 16 {
        return Err(WavError::Malformed("the 'fmt ' chunk is too short".into()));
    }
    let u16_at = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap());

    let mut tag = u16_at(0);
    let channels = u16_at(2);
    let sample_rate = u32_at(4);
    let block_align = u16_at(12);
    let bits = u16_at(14);
    let mut valid_bits = bits;
    let mut channel_mask = None;

    if tag == WAVE_FORMAT_EXTENSIBLE {
        if b.len() < 40 || u16_at(16) < 22 {
            return Err(WavError::Malformed(
                "the extensible 'fmt ' chunk is too short".into(),
            ));
        }
        let declared_valid = u16_at(18);
        if declared_valid != 0 {
            valid_bits = declared_valid;
        }
        channel_mask = Some(u32_at(20));
        if b[26..40] != SUBFORMAT_GUID_TAIL {
            return Err(WavError::UnsupportedEncoding(
                "unknown extensible subformat".into(),
            ));
        }
        tag = u16_at(24);
    }

    let format = match (tag, bits) {
        (WAVE_FORMAT_PCM, 8) => SampleFormat::U8,
        (WAVE_FORMAT_PCM, 16) => SampleFormat::I16,
        (WAVE_FORMAT_PCM, 24) => SampleFormat::I24,
        (WAVE_FORMAT_PCM, 32) => SampleFormat::I32,
        (WAVE_FORMAT_IEEE_FLOAT, 32) => SampleFormat::F32,
        (WAVE_FORMAT_IEEE_FLOAT, 64) => SampleFormat::F64,
        (WAVE_FORMAT_PCM, _) => {
            return Err(WavError::UnsupportedBitDepth {
                encoding: "PCM",
                bits,
            });
        }
        (WAVE_FORMAT_IEEE_FLOAT, _) => {
            return Err(WavError::UnsupportedBitDepth {
                encoding: "IEEE float",
                bits,
            });
        }
        _ => return Err(WavError::UnsupportedEncoding(format_tag_name(tag))),
    };

    if channels == 0 || channels > MAX_CHANNELS {
        return Err(WavError::ChannelCount(channels));
    }
    if sample_rate == 0 || sample_rate > MAX_SAMPLE_RATE {
        return Err(WavError::SampleRate(sample_rate));
    }
    if block_align as usize != format.bytes() * channels as usize {
        return Err(WavError::Malformed(format!(
            "block align {block_align} does not match {channels} channels of {bits}-bit samples"
        )));
    }
    if valid_bits == 0 || valid_bits > bits {
        return Err(WavError::Malformed(format!(
            "{valid_bits} valid bits in a {bits}-bit container"
        )));
    }

    Ok(Fmt {
        format,
        valid_bits,
        channels,
        sample_rate,
        channel_mask,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wav::synth::{Encoding, WavBuilder};

    fn info(builder: &WavBuilder, samples: &[f64]) -> Result<WavInfo, WavError> {
        parse_header(&builder.build(samples))
    }

    #[test]
    fn accepts_every_plain_format() {
        for (enc, bits, fmt) in [
            (Encoding::Pcm, 8, SampleFormat::U8),
            (Encoding::Pcm, 16, SampleFormat::I16),
            (Encoding::Pcm, 24, SampleFormat::I24),
            (Encoding::Pcm, 32, SampleFormat::I32),
            (Encoding::Float, 32, SampleFormat::F32),
            (Encoding::Float, 64, SampleFormat::F64),
        ] {
            for extensible in [false, true] {
                let b = WavBuilder::new(enc, bits, 2, 44_100).extensible(extensible);
                let i = info(&b, &[0.0; 20]).unwrap();
                assert_eq!(i.format, fmt, "{enc:?} {bits} ext={extensible}");
                assert_eq!(i.channels, 2);
                assert_eq!(i.frames, 10);
                assert!(!i.truncated);
                assert_eq!(i.channel_mask.is_some(), extensible);
            }
        }
    }

    #[test]
    fn stereo_24_bit_48k() {
        let i = info(&WavBuilder::new(Encoding::Pcm, 24, 2, 48_000), &[0.0; 96_000]).unwrap();
        assert_eq!((i.channels, i.sample_rate, i.frames), (2, 48_000, 48_000));
        assert_eq!(i.format, SampleFormat::I24);
    }

    #[test]
    fn extensible_float_six_channels() {
        let b = WavBuilder::new(Encoding::Float, 32, 6, 48_000).extensible(true);
        let i = info(&b, &[0.0; 60]).unwrap();
        assert_eq!(i.channels, 6);
        assert_eq!(i.format, SampleFormat::F32);
        assert_eq!(i.channel_mask, Some(0x3F));
    }

    #[test]
    fn channel_and_rate_limits() {
        assert!(info(&WavBuilder::new(Encoding::Pcm, 16, 1, 1), &[0.0]).is_ok());
        assert!(info(&WavBuilder::new(Encoding::Pcm, 16, 32, 768_000), &[0.0; 32]).is_ok());
        assert!(matches!(
            info(&WavBuilder::new(Encoding::Pcm, 16, 33, 48_000), &[0.0; 33]),
            Err(WavError::ChannelCount(33))
        ));
        assert!(matches!(
            info(&WavBuilder::new(Encoding::Pcm, 16, 1, 768_001), &[0.0]),
            Err(WavError::SampleRate(768_001))
        ));
        assert!(matches!(
            info(&WavBuilder::new(Encoding::Pcm, 16, 1, 0), &[0.0]),
            Err(WavError::SampleRate(0))
        ));
    }

    #[test]
    fn skips_metadata_and_odd_chunks() {
        let b = WavBuilder::new(Encoding::Pcm, 16, 2, 44_100)
            .chunk(*b"LIST", b"INFOISFT\x05\x00\x00\x00abcde".to_vec())
            .chunk(*b"bext", vec![0; 602])
            .chunk(*b"odd ", vec![1, 2, 3]);
        let i = info(&b, &[0.5; 8]).unwrap();
        assert_eq!(i.frames, 4);
        assert!(!i.truncated);
    }

    #[test]
    fn truncated_data_keeps_whole_frames() {
        let b = WavBuilder::new(Encoding::Pcm, 16, 2, 44_100);
        let mut bytes = b.build(&[0.25; 200]);
        // Drop the last 1.5 frames (6 bytes): 100 frames declared, 98 whole frames remain.
        bytes.truncate(bytes.len() - 6);
        let i = parse_header(&bytes).unwrap();
        assert!(i.truncated);
        assert_eq!(i.frames, 98);
    }

    #[test]
    fn rejects_non_riff() {
        assert!(matches!(
            parse_header(b"ID3\x04\x00\x00\x00\x00\x00\x00 mp3 data"),
            Err(WavError::NotWav)
        ));
        assert!(matches!(parse_header(b"RI"), Err(WavError::NotWav)));
        assert!(matches!(
            parse_header(b"RIFF\x04\x00\x00\x00AVI "),
            Err(WavError::NotWav)
        ));
    }

    #[test]
    fn rejects_unsupported_encodings_by_name() {
        for (tag, name) in [
            (0x0011, "IMA ADPCM"),
            (0x0002, "Microsoft ADPCM"),
            (0x0007, "µ-law"),
            (0x0006, "A-law"),
        ] {
            let err = info(&WavBuilder::new(Encoding::Tag(tag), 4, 1, 8_000), &[0.0]).unwrap_err();
            assert!(err.to_string().contains(name), "{err}");
            assert!(matches!(err, WavError::UnsupportedEncoding(_)));
        }
        let mut rf64 = WavBuilder::new(Encoding::Pcm, 16, 1, 8_000).build(&[0.0]);
        rf64[0..4].copy_from_slice(b"RF64");
        assert!(parse_header(&rf64).unwrap_err().to_string().contains("RF64"));
    }

    #[test]
    fn rejects_unsupported_bit_depths() {
        let err = info(&WavBuilder::new(Encoding::Pcm, 12, 1, 8_000), &[0.0]).unwrap_err();
        assert!(matches!(err, WavError::UnsupportedBitDepth { bits: 12, .. }));
        let err = info(&WavBuilder::new(Encoding::Float, 16, 1, 8_000), &[0.0]).unwrap_err();
        assert!(matches!(err, WavError::UnsupportedBitDepth { bits: 16, .. }));
    }

    #[test]
    fn rejects_missing_chunks() {
        let full = WavBuilder::new(Encoding::Pcm, 16, 1, 8_000).build(&[0.0; 4]);
        // Header + fmt chunk only.
        let no_data = &full[..12 + 8 + 16];
        assert!(matches!(
            parse_header(no_data),
            Err(WavError::MissingChunk("data"))
        ));
        let mut no_fmt = full[..12].to_vec();
        no_fmt.extend_from_slice(&full[12 + 8 + 16..]);
        assert!(matches!(
            parse_header(&no_fmt),
            Err(WavError::MissingChunk("fmt "))
        ));
    }
}
