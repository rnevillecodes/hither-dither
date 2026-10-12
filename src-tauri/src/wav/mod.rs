//! WAV file parsing and on-demand sample decoding.

mod parser;
mod source;
pub mod synth;
mod writer;

pub use parser::{WavInfo, parse_header};
pub use source::SampleSource;
pub use writer::{WriteError, check_size, write_wav, write_wav_with_progress};

use serde::Serialize;

/// On-disk sample encoding of a supported WAV file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SampleFormat {
    U8,
    I16,
    I24,
    I32,
    F32,
    F64,
}

impl SampleFormat {
    pub fn bytes(self) -> usize {
        match self {
            SampleFormat::U8 => 1,
            SampleFormat::I16 => 2,
            SampleFormat::I24 => 3,
            SampleFormat::I32 | SampleFormat::F32 => 4,
            SampleFormat::F64 => 8,
        }
    }

    pub fn is_float(self) -> bool {
        matches!(self, SampleFormat::F32 | SampleFormat::F64)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WavError {
    #[error("not a valid WAV file (missing RIFF/WAVE header)")]
    NotWav,
    #[error("the {0} encoding is not supported")]
    UnsupportedEncoding(String),
    #[error("{bits}-bit {encoding} samples are not supported")]
    UnsupportedBitDepth { encoding: &'static str, bits: u16 },
    #[error("the file has no '{0}' chunk")]
    MissingChunk(&'static str),
    #[error("{0} channels is outside the supported range of 1 to 32")]
    ChannelCount(u16),
    #[error("a sample rate of {0} Hz is outside the supported range of 1 Hz to 768 kHz")]
    SampleRate(u32),
    #[error("malformed WAV file: {0}")]
    Malformed(String),
    #[error("could not read the file: {0}")]
    Io(#[from] std::io::Error),
}
