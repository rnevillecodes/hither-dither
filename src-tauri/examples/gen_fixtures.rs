//! Writes WAV fixtures for manual acceptance testing.
//!
//! Usage: `cargo run --release --example gen_fixtures -- <out-dir> [--large]`
//! `--large` also writes a ~1 GB file and a 1-hour stereo 48 kHz file (~660 MiB).

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use hither_dither_lib::wav::synth::{Encoding, WavBuilder, db_to_amp, interleave, sine};

fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().unwrap_or_else(|| "fixtures".into()));
    let large = args.any(|a| a == "--large");
    std::fs::create_dir_all(&out)?;
    let p = |name: &str| out.join(name);

    // wav-import
    let tone = sine(440.0, 0.5, 44_100, 441_000);
    WavBuilder::new(Encoding::Pcm, 16, 2, 44_100).write(
        p("cd-16bit-stereo-10s.wav"),
        &interleave(&[tone.clone(), sine(660.0, 0.4, 44_100, 441_000)]),
    )?;
    let t48 = sine(1000.0, 0.7, 48_000, 48_000 * 5);
    WavBuilder::new(Encoding::Pcm, 24, 2, 48_000).write(
        p("pcm24-stereo-48k.wav"),
        &interleave(&[t48.clone(), t48.clone()]),
    )?;
    let six: Vec<Vec<f64>> = (0..6)
        .map(|c| sine(220.0 * (c + 1) as f64, 0.3, 48_000, 48_000 * 4))
        .collect();
    WavBuilder::new(Encoding::Float, 32, 6, 48_000)
        .extensible(true)
        .write(p("float32-extensible-5.1.wav"), &interleave(&six))?;
    WavBuilder::new(Encoding::Float, 64, 1, 48_000).write(p("float64-mono.wav"), &t48)?;
    WavBuilder::new(Encoding::Pcm, 8, 1, 22_050)
        .write(p("u8-mono.wav"), &sine(300.0, 0.8, 22_050, 22_050 * 3))?;
    WavBuilder::new(Encoding::Pcm, 16, 2, 44_100)
        .chunk(
            *b"LIST",
            b"INFOINAM\x09\x00\x00\x00Fixture!\x00\x00".to_vec(),
        )
        .chunk(*b"bext", vec![0; 603])
        .write(
            p("with-list-bext-chunks.wav"),
            &interleave(&[tone.clone(), tone.clone()]),
        )?;
    let mut cut = WavBuilder::new(Encoding::Pcm, 16, 2, 44_100)
        .build(&interleave(&[tone.clone(), tone.clone()]));
    cut.truncate(cut.len() / 2 + 3);
    std::fs::write(p("truncated.wav"), cut)?;
    WavBuilder::new(Encoding::Tag(0x0011), 4, 1, 8_000).write(p("ima-adpcm.wav"), &[0.0; 256])?;
    WavBuilder::new(Encoding::Tag(0x0007), 8, 1, 8_000).write(p("mu-law.wav"), &[0.0; 256])?;
    std::fs::write(
        p("not-a-wav.wav"),
        b"This is a text file pretending to be audio.\n",
    )?;

    // waveform-view
    let mut spike = vec![0.0; 48_000 * 600];
    spike[48_000 * 377 + 123] = 1.0;
    WavBuilder::new(Encoding::Pcm, 16, 1, 48_000).write(p("single-spike-10min.wav"), &spike)?;

    // audio-stats
    let full = sine(1000.0, 1.0, 48_000, 48_000 * 10);
    WavBuilder::new(Encoding::Pcm, 16, 2, 48_000).write(
        p("full-scale-sine-and-silence.wav"),
        &interleave(&[full, vec![0.0; 480_000]]),
    )?;
    let reference = sine(1000.0, db_to_amp(-18.0), 48_000, 48_000 * 10);
    WavBuilder::new(Encoding::Pcm, 24, 2, 48_000).write(
        p("reference-minus18dbfs-stereo.wav"),
        &interleave(&[reference.clone(), reference]),
    )?;
    let mut clipped = sine(200.0, 0.6, 44_100, 44_100 * 4);
    clipped[44_100..44_105].fill(1.0);
    clipped[44_100 * 3..44_100 * 3 + 5].fill(1.0);
    clipped[100_000] = 1.0;
    WavBuilder::new(Encoding::Pcm, 16, 1, 44_100).write(p("two-clip-runs.wav"), &clipped)?;
    let mut gap = sine(500.0, 0.9, 48_000, 48_000 * 10);
    gap[48_000 * 4..48_000 * 6].fill(0.0);
    WavBuilder::new(Encoding::Pcm, 16, 2, 48_000).write(
        p("loud-with-silent-gap-4s-6s.wav"),
        &interleave(&[gap.clone(), gap]),
    )?;
    WavBuilder::new(Encoding::Float, 32, 4, 48_000)
        .write(p("quad-loudness-na.wav"), &vec![0.1; 48_000 * 4 * 2])?;

    // audio-playback
    let hi = sine(1000.0, 0.5, 96_000, 96_000 * 10);
    WavBuilder::new(Encoding::Pcm, 24, 2, 96_000)
        .write(p("tone-1khz-96k.wav"), &interleave(&[hi.clone(), hi]))?;
    let beeps: Vec<f64> = (0..48_000 * 60)
        .map(|i| {
            if (i / 4_800) % 10 == 0 {
                0.5 * (std::f64::consts::TAU * 880.0 * i as f64 / 48_000.0).sin()
            } else {
                0.0
            }
        })
        .collect();
    WavBuilder::new(Encoding::Pcm, 16, 1, 48_000)
        .write(p("beep-every-second-mono-60s.wav"), &beeps)?;

    if large {
        stream_sine(&p("large-1gb.wav"), 48_000, 2, 48_000 * 5592)?; // ≈ 1.0 GiB
        stream_sine(&p("one-hour-stereo-48k.wav"), 48_000, 2, 48_000 * 3600)?;
    }
    println!("Fixtures written to {}", out.display());
    Ok(())
}

/// Write a large 16-bit file without holding it in memory.
fn stream_sine(path: &Path, rate: u32, channels: u16, frames: u64) -> std::io::Result<()> {
    let mut header = WavBuilder::new(Encoding::Pcm, 16, channels, rate).build(&[]);
    let data_len = frames * channels as u64 * 2;
    let n = header.len();
    header[n - 4..].copy_from_slice(&(data_len as u32).to_le_bytes());
    header[4..8].copy_from_slice(&((n as u64 - 8 + data_len) as u32).to_le_bytes());
    let mut w = BufWriter::with_capacity(1 << 20, File::create(path)?);
    w.write_all(&header)?;
    for i in 0..frames {
        let t = i as f64 / rate as f64;
        // A slow amplitude sweep makes scrolling visibly change the waveform.
        let amp = 0.2 + 0.7 * (0.5 + 0.5 * (t * 0.05).sin());
        let v = (amp * (std::f64::consts::TAU * 220.0 * t).sin() * 32767.0) as i16;
        for _ in 0..channels {
            w.write_all(&v.to_le_bytes())?;
        }
    }
    w.flush()
}
