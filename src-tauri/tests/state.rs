//! Integration tests for loading files and the command-level response shapes.

use hither_dither_lib::commands::{
    PEAKS_HEADER_BYTES, PEAKS_KIND_MINMAX, PEAKS_KIND_SAMPLES, encode_peaks,
};
use hither_dither_lib::state::{AppState, OpenError};
use hither_dither_lib::stats::Loudness;
use hither_dither_lib::wav::synth::{Encoding, WavBuilder, interleave, sine};

fn write(
    dir: &tempfile::TempDir,
    name: &str,
    b: WavBuilder,
    samples: &[f64],
) -> std::path::PathBuf {
    let p = dir.path().join(name);
    b.write(&p, samples).unwrap();
    p
}

fn header(bytes: &[u8]) -> (u32, u32, u32, f64) {
    let u = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
    (
        u(0),
        u(4),
        u(8),
        f64::from_le_bytes(bytes[16..24].try_into().unwrap()),
    )
}

#[test]
fn failed_open_keeps_previous_file() {
    let dir = tempfile::tempdir().unwrap();
    let good = write(
        &dir,
        "good.wav",
        WavBuilder::new(Encoding::Pcm, 16, 2, 44_100),
        &[0.1; 882],
    );
    let adpcm = write(
        &dir,
        "adpcm.wav",
        WavBuilder::new(Encoding::Tag(0x11), 4, 1, 8_000),
        &[0.0; 4],
    );
    let junk = dir.path().join("notes.wav");
    std::fs::write(&junk, b"definitely not audio").unwrap();

    let state = AppState::default();
    let meta = state.open(&good, |_, _| {}).unwrap();
    assert_eq!(meta.name, "good.wav");

    let err = state.open(&adpcm, |_, _| {}).unwrap_err();
    assert!(matches!(err, OpenError::Wav { .. }));
    let msg = err.to_string();
    assert!(
        msg.contains("adpcm.wav") && msg.contains("IMA ADPCM"),
        "{msg}"
    );

    let err = state.open(&junk, |_, _| {}).unwrap_err().to_string();
    assert!(
        err.contains("notes.wav") && err.contains("not a valid WAV"),
        "{err}"
    );

    assert!(
        state
            .open(&dir.path().join("missing.wav"), |_, _| {})
            .is_err()
    );
    assert_eq!(state.current().unwrap().metadata.name, "good.wav");
}

#[test]
fn metadata_for_cd_quality_file() {
    let dir = tempfile::tempdir().unwrap();
    let p = write(
        &dir,
        "ten.wav",
        WavBuilder::new(Encoding::Pcm, 16, 2, 44_100),
        &vec![0.0; 882_000],
    );
    let state = AppState::default();
    let mut calls = Vec::new();
    let m = state.open(&p, |d, t| calls.push((d, t))).unwrap();
    assert_eq!(m.encoding, "PCM");
    assert_eq!(
        (m.bit_depth, m.sample_rate, m.channels, m.frames),
        (16, 44_100, 2, 441_000)
    );
    assert_eq!(m.duration_secs, 10.0);
    assert_eq!(m.channel_labels, ["L", "R"]);
    assert_eq!(m.size_bytes, std::fs::metadata(&p).unwrap().len());
    assert!(!m.truncated);
    assert_eq!(calls.last(), Some(&(441_000, 441_000)));
}

#[test]
fn truncated_file_loads_with_flag() {
    let dir = tempfile::tempdir().unwrap();
    let mut bytes = WavBuilder::new(Encoding::Pcm, 16, 1, 8_000).build(&[0.2; 100]);
    bytes.truncate(bytes.len() - 21);
    let p = dir.path().join("cut.wav");
    std::fs::write(&p, bytes).unwrap();
    let m = AppState::default().open(&p, |_, _| {}).unwrap();
    assert!(m.truncated);
    assert_eq!(m.frames, 89);
}

#[test]
fn peaks_response_shape_and_length() {
    let dir = tempfile::tempdir().unwrap();
    let tone = sine(100.0, 0.5, 8_000, 80_000);
    let p = write(
        &dir,
        "s.wav",
        WavBuilder::new(Encoding::Pcm, 16, 2, 8_000),
        &interleave(&[tone.clone(), tone]),
    );
    let state = AppState::default();
    state.open(&p, |_, _| {}).unwrap();
    let file = state.current().unwrap();

    let b = encode_peaks(&file, 0, 80_000, 800);
    assert_eq!(header(&b), (PEAKS_KIND_MINMAX, 2, 800, 0.0));
    assert_eq!(b.len(), PEAKS_HEADER_BYTES + 2 * 800 * 2 * 4);
    let f = |i: usize| f32::from_le_bytes(b[PEAKS_HEADER_BYTES + i * 4..][..4].try_into().unwrap());
    // Each 100-frame column spans a full 100 Hz cycle: min ≈ -0.5, max ≈ 0.5.
    assert!((f(0) + 0.5).abs() < 0.01 && (f(1) - 0.5).abs() < 0.01);

    let b = encode_peaks(&file, 1_000, 1_100, 800);
    assert_eq!(header(&b), (PEAKS_KIND_SAMPLES, 2, 100, 1_000.0));
    assert_eq!(b.len(), PEAKS_HEADER_BYTES + 2 * 100 * 4);

    // Out-of-range requests clamp instead of failing.
    let b = encode_peaks(&file, 79_990, 1_000_000, 800);
    assert_eq!(header(&b), (PEAKS_KIND_SAMPLES, 2, 10, 79_990.0));
}

#[test]
fn stats_and_clip_runs_commands() {
    let dir = tempfile::tempdir().unwrap();
    let mut x = sine(1000.0, 0.5, 48_000, 96_000);
    x[48_000..96_000].fill(0.0);
    x[10_000..10_004].fill(1.0);
    let p = write(
        &dir,
        "c.wav",
        WavBuilder::new(Encoding::Pcm, 16, 1, 48_000),
        &x,
    );
    let state = AppState::default();
    assert!(state.compute_stats(None).is_err());
    state.open(&p, |_, _| {}).unwrap();

    let whole = state.compute_stats(None).unwrap().unwrap();
    assert_eq!((whole.start, whole.end), (0, 96_000));
    assert_eq!(whole.channels[0].clip_runs, 1);
    assert!(matches!(whole.loudness, Loudness::Value { .. }));

    let sel = state
        .compute_stats(Some((48_000, 96_000)))
        .unwrap()
        .unwrap();
    assert_eq!(sel.channels[0].levels.peak_dbfs, None);
    assert_eq!(sel.loudness, Loudness::Silent);

    let json = serde_json::to_value(&sel).unwrap();
    assert!(json["channels"][0].get("peak_dbfs").is_some());
    assert_eq!(json["loudness"]["kind"], "silent");

    let runs = state.clip_runs().unwrap();
    assert_eq!(runs.channels[0].len(), 1);
    assert_eq!(runs.channels[0][0].start, 10_000);
    assert!(!runs.capped);
}

#[test]
fn detects_file_changed_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let p = write(
        &dir,
        "x.wav",
        WavBuilder::new(Encoding::Pcm, 16, 1, 8_000),
        &[0.1; 100],
    );
    let state = AppState::default();
    state.open(&p, |_, _| {}).unwrap();
    assert!(!state.current().unwrap().changed_on_disk());
    // Appending changes the size (and on most systems the mtime).
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(&p)
        .unwrap()
        .write_all(&[0; 10])
        .unwrap();
    assert!(state.current().unwrap().changed_on_disk());
}
