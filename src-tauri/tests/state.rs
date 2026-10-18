//! Integration tests for loading files and the command-level response shapes.

mod common;

use std::sync::Arc;

use common::test_plugins::{self, TestPlugin};
use hither_dither_lib::commands::{
    PEAKS_HEADER_BYTES, PEAKS_KIND_MINMAX, PEAKS_KIND_SAMPLES, encode_peaks,
};
use hither_dither_lib::fx::chain::{ChainInfo, SlotId};
use hither_dither_lib::plugins::ClassId;
use hither_dither_lib::plugins::cache::PluginCache;
use hither_dither_lib::plugins::scan::Scanner;
use hither_dither_lib::session::SessionFile;
use hither_dither_lib::state::{AppState, ClipSettings, EditOp, FadeSetting, OpenError, Preview};
use hither_dither_lib::stats::Loudness;
use hither_dither_lib::wav::synth::{Encoding, WavBuilder, interleave, sine};
use hither_dither_lib::wav::{SampleSource, write_wav};

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
    let meta = state.open(1, &good, |_, _| {}).unwrap();
    assert_eq!(meta.name, "good.wav");

    let err = state.open(2, &adpcm, |_, _| {}).unwrap_err();
    assert!(matches!(err, OpenError::Wav { .. }));
    let msg = err.to_string();
    assert!(
        msg.contains("adpcm.wav") && msg.contains("IMA ADPCM"),
        "{msg}"
    );

    let err = state.open(3, &junk, |_, _| {}).unwrap_err().to_string();
    assert!(
        err.contains("notes.wav") && err.contains("not a valid WAV"),
        "{err}"
    );

    assert!(
        state
            .open(4, &dir.path().join("missing.wav"), |_, _| {})
            .is_err()
    );
    assert_eq!(state.get(1).unwrap().metadata.name, "good.wav");
    assert!(state.get(2).is_none() && state.get(3).is_none() && state.get(4).is_none());
}

#[test]
fn files_stay_open_side_by_side_until_closed() {
    let dir = tempfile::tempdir().unwrap();
    let a = write(
        &dir,
        "a.wav",
        WavBuilder::new(Encoding::Pcm, 16, 1, 8_000),
        &[0.1; 100],
    );
    let b = write(
        &dir,
        "b.wav",
        WavBuilder::new(Encoding::Pcm, 16, 2, 8_000),
        &[0.2; 400],
    );
    let state = AppState::default();
    state.open(1, &a, |_, _| {}).unwrap();
    state.open(2, &b, |_, _| {}).unwrap();
    assert_eq!(state.get(1).unwrap().metadata.name, "a.wav");
    assert_eq!(state.get(2).unwrap().metadata.name, "b.wav");
    assert_eq!(state.compute_stats(2, None).unwrap().unwrap().end, 200);
    assert_eq!(state.clip_runs(1).unwrap().channels.len(), 1);

    assert_eq!(state.close(1).unwrap().metadata.name, "a.wav");
    assert!(state.get(1).is_none());
    assert!(state.compute_stats(1, None).is_err());
    assert_eq!(state.get(2).unwrap().metadata.name, "b.wav");
    assert!(state.close(1).is_none());
}

#[test]
fn cancel_open_stops_only_that_load() {
    let dir = tempfile::tempdir().unwrap();
    let a = write(
        &dir,
        "a.wav",
        WavBuilder::new(Encoding::Pcm, 16, 1, 8_000),
        &[0.1; 100],
    );
    let big = write(
        &dir,
        "big.wav",
        WavBuilder::new(Encoding::Pcm, 16, 1, 48_000),
        &vec![0.1; 480_000],
    );
    let state = AppState::default();
    state.open(1, &a, |_, _| {}).unwrap();

    let err = state
        .open(2, &big, |_, _| state.cancel_open(2))
        .unwrap_err();
    assert!(
        matches!(err, OpenError::Cancelled(ref n) if n == "big.wav"),
        "{err}"
    );
    assert!(state.get(2).is_none());
    assert_eq!(state.get(1).unwrap().metadata.name, "a.wav");

    // Closing a loading file cancels it the same way.
    let err = state
        .open(3, &big, |_, _| drop(state.close(3)))
        .unwrap_err();
    assert!(matches!(err, OpenError::Cancelled(_)));
    assert!(state.get(3).is_none());

    // A later open under the same id is not affected by the earlier cancel.
    state.open(2, &big, |_, _| {}).unwrap();
    assert_eq!(state.get(2).unwrap().metadata.frames, 480_000);
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
    let m = state.open(1, &p, |d, t| calls.push((d, t))).unwrap();
    assert_eq!(m.encoding, "PCM");
    assert_eq!(
        (m.bit_depth, m.sample_rate, m.channels, m.frames),
        (16, 44_100, 2, 441_000)
    );
    assert_eq!(m.duration_secs, 10.0);
    assert_eq!(m.channel_labels, ["L", "R"]);
    assert_eq!((m.speaker_arrangement, m.layout.as_str()), (0x3, "stereo"));
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
    let m = AppState::default().open(1, &p, |_, _| {}).unwrap();
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
    state.open(1, &p, |_, _| {}).unwrap();
    let file = state.get(1).unwrap();

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
    assert!(state.compute_stats(1, None).is_err());
    state.open(1, &p, |_, _| {}).unwrap();

    let whole = state.compute_stats(1, None).unwrap().unwrap();
    assert_eq!((whole.start, whole.end), (0, 96_000));
    assert_eq!(whole.channels[0].clip_runs, 1);
    assert!(matches!(whole.loudness, Loudness::Value { .. }));

    let sel = state
        .compute_stats(1, Some((48_000, 96_000)))
        .unwrap()
        .unwrap();
    assert_eq!(sel.channels[0].levels.peak_dbfs, None);
    assert_eq!(sel.loudness, Loudness::Silent);

    let json = serde_json::to_value(&sel).unwrap();
    assert!(json["channels"][0].get("peak_dbfs").is_some());
    assert_eq!(json["loudness"]["kind"], "silent");

    let runs = state.clip_runs(1).unwrap();
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
    state.open(1, &p, |_, _| {}).unwrap();
    assert!(!state.get(1).unwrap().changed_on_disk());
    // Appending changes the size (and on most systems the mtime).
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(&p)
        .unwrap()
        .write_all(&[0; 10])
        .unwrap();
    assert!(state.get(1).unwrap().changed_on_disk());
}

/// A 16-bit stereo sine file of `frames` frames at 8 kHz, opened as `id`.
fn open_sine(state: &AppState, dir: &tempfile::TempDir, id: u32, frames: usize) {
    let s = sine(440.0, 0.5, 8_000, frames);
    let path = write(
        dir,
        &format!("sine{id}.wav"),
        WavBuilder::new(Encoding::Pcm, 16, 2, 8_000),
        &interleave(&[s.clone(), s]),
    );
    state.open(id, &path, |_, _| {}).unwrap();
}

fn whole_stats(state: &AppState, id: u32) -> hither_dither_lib::stats::Stats {
    state.compute_stats(id, None).unwrap().unwrap()
}

/// Whole-file stats once the current edit state has been analysed.
fn analysed_stats(state: &AppState, id: u32) -> hither_dither_lib::stats::Stats {
    assert!(state.wait_analysis(id, state.edit_state(id).unwrap().revision));
    whole_stats(state, id)
}

#[test]
fn edit_then_undo_restores_frames_and_stats() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let before = whole_stats(&state, 1);
    assert!(!state.edit_state(1).unwrap().dirty);

    let out = state
        .apply_edit(1, EditOp::Delete, 1_000, 3_000, 0.0)
        .unwrap();
    assert_eq!(out.metadata.frames, 6_000);
    assert_eq!(state.get(1).unwrap().audio.frames(), 6_000);
    assert!(out.edit_state.dirty && out.edit_state.can_undo && !out.edit_state.can_redo);
    assert!(state.wait_analysis(1, out.edit_state.revision));
    assert_eq!(whole_stats(&state, 1).end, 6_000);

    let out = state.undo(1).unwrap();
    assert_eq!(out.metadata.frames, 8_000);
    assert!(!out.edit_state.dirty && !out.edit_state.can_undo && out.edit_state.can_redo);
    // Back to the unedited source: its analysis applies straight away.
    assert_eq!(whole_stats(&state, 1), before);
    assert!(state.undo(1).is_err());

    let out = state.redo(1).unwrap();
    assert_eq!(out.metadata.frames, 6_000);
    assert!(out.edit_state.revision > 1);
}

#[test]
fn new_edit_after_undo_clears_redo() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    state.apply_edit(1, EditOp::Silence, 100, 200, 0.0).unwrap();
    state.undo(1).unwrap();
    let out = state.apply_edit(1, EditOp::FadeIn, 0, 4_000, 0.0).unwrap();
    assert!(!out.edit_state.can_redo && out.edit_state.can_undo);
    assert!(state.redo(1).is_err());
}

#[test]
fn silence_shows_in_selection_stats() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    state
        .apply_edit(1, EditOp::Silence, 2_000, 3_000, 0.0)
        .unwrap();
    let sel = state
        .compute_stats(1, Some((2_000, 3_000)))
        .unwrap()
        .unwrap();
    assert!(
        sel.channels
            .iter()
            .all(|c| c.levels.peak_linear == 0.0 && c.levels.peak_dbfs.is_none())
    );
}

#[test]
fn edits_on_one_file_leave_others_alone() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    open_sine(&state, &dir, 2, 8_000);
    state.apply_edit(1, EditOp::Crop, 0, 1_000, 0.0).unwrap();
    assert_eq!(state.get(2).unwrap().metadata.frames, 8_000);
    let other = state.edit_state(2).unwrap();
    assert!(!other.dirty && !other.can_undo);
    assert!(state.undo(2).is_err());
}

#[test]
fn rejected_edit_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let err = state
        .apply_edit(1, EditOp::Delete, 0, 8_000, 0.0)
        .unwrap_err();
    assert!(err.contains("no audio"), "{err}");
    assert!(!state.edit_state(1).unwrap().can_undo);
    // The failed edit didn't leave the file marked busy.
    assert!(state.apply_edit(1, EditOp::Silence, 0, 10, 0.0).is_ok());
}

/// A float file (so it can be rewritten without rounding) of a 440 Hz sine.
fn open_float_sine(state: &AppState, dir: &tempfile::TempDir, id: u32, frames: usize) {
    let s = sine(440.0, 0.9, 8_000, frames);
    let path = write(
        dir,
        &format!("float{id}.wav"),
        WavBuilder::new(Encoding::Float, 32, 2, 8_000),
        &interleave(&[s.clone(), s]),
    );
    state.open(id, &path, |_, _| {}).unwrap();
}

/// Write file `id`'s edited audio to a new file and open that as `as_id`.
fn open_rendered(state: &AppState, dir: &tempfile::TempDir, id: u32, as_id: u32) {
    let path = dir.path().join(format!("rendered{as_id}.wav"));
    let audio = state.get(id).unwrap().audio.clone();
    write_wav(&audio, std::fs::File::create(&path).unwrap()).unwrap();
    state.open(as_id, &path, |_, _| {}).unwrap();
}

#[test]
fn edits_apply_back_to_back_without_analysis() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 300_000);
    let ops = [
        (EditOp::Silence, 1_000, 50_000),
        (EditOp::FadeOut, 100_000, 200_000),
        (EditOp::Delete, 10_000, 20_000),
        (EditOp::FadeIn, 0, 5_000),
        (EditOp::Crop, 0, 250_000),
    ];
    for (op, start, end) in ops {
        state.apply_edit(1, op, start, end, 0.0).unwrap();
        // The whole-file stats are stale until the analysis finishes.
        assert_eq!(state.compute_stats(1, None).unwrap(), None);
        assert!(state.clip_runs(1).unwrap().pending);
    }
    assert_eq!(state.get(1).unwrap().metadata.frames, 250_000);
    assert_eq!(state.undo(1).unwrap().metadata.frames, 290_000);
}

#[test]
fn rapid_changes_are_analysed_once() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 100_000);
    let before = state.completed_analyses();
    let first = state.apply_edit(1, EditOp::Silence, 0, 100, 0.0).unwrap();
    let mut last = first.clone();
    for i in 1..5 {
        last = state
            .apply_edit(1, EditOp::FadeOut, i * 1_000, i * 1_000 + 500, 0.0)
            .unwrap();
    }
    // An older revision is superseded; the last one is analysed, once.
    assert!(!state.wait_analysis(1, first.edit_state.revision));
    assert!(state.wait_analysis(1, last.edit_state.revision));
    assert_eq!(state.completed_analyses() - before, 1);
    assert!(state.compute_stats(1, None).unwrap().is_some());
    assert!(!state.clip_runs(1).unwrap().pending);
}

#[test]
fn analysis_matches_a_file_with_the_same_audio() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_float_sine(&state, &dir, 1, 60_000);
    let out = state
        .apply_edit(1, EditOp::Delete, 10_000, 25_000, 0.0)
        .unwrap();
    open_rendered(&state, &dir, 1, 2);
    // Before the analysis: peaks are drawn through the edits. A delete only changes the
    // short crossfade, which is read exactly, so they match the rendered file's.
    assert!(state.get(1).unwrap().analysis.is_none());
    for columns in [1, 37, 800, 5_000] {
        assert_eq!(
            encode_peaks(&state.get(1).unwrap(), 0, 45_000, columns),
            encode_peaks(&state.get(2).unwrap(), 0, 45_000, columns),
            "{columns} columns"
        );
    }
    assert!(state.wait_analysis(1, out.edit_state.revision));
    assert_eq!(whole_stats(&state, 1), whole_stats(&state, 2));
    assert_eq!(
        encode_peaks(&state.get(1).unwrap(), 0, 45_000, 800),
        encode_peaks(&state.get(2).unwrap(), 0, 45_000, 800)
    );
}

#[test]
fn closing_during_analysis_resolves_waiters() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 300_000);
    let out = state
        .apply_edit(1, EditOp::FadeOut, 0, 300_000, 0.0)
        .unwrap();
    let analysed = std::thread::scope(|s| {
        let waiter = s.spawn(|| state.wait_analysis(1, out.edit_state.revision));
        std::thread::sleep(std::time::Duration::from_millis(50));
        state.close(1);
        waiter.join().unwrap()
    });
    assert!(!analysed);
    assert!(state.get(1).is_none() && state.edit_state(1).is_none());
}

fn frames_on_disk(path: &std::path::Path) -> u64 {
    hither_dither_lib::wav::parse_header(&std::fs::read(path).unwrap())
        .unwrap()
        .frames
}

#[test]
fn export_writes_the_edits_and_leaves_the_tab_alone() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let source = dir.path().join("sine1.wav");
    let original = std::fs::read(&source).unwrap();
    state
        .apply_edit(1, EditOp::Delete, 1_000, 3_000, 0.0)
        .unwrap();
    let edited_stats = analysed_stats(&state, 1);
    let before = state.edit_state(1).unwrap();

    let copy = dir.path().join("trimmed.wav");
    let mut last = (0, 0);
    let out = state
        .export(1, &copy, |done, total| last = (done, total))
        .unwrap();
    assert_eq!(out.clamped_samples, 0);
    // One writing pass, with no analysis first.
    assert_eq!(last, (6_000, 6_000));
    assert_eq!(std::fs::read(&source).unwrap(), original);
    assert_eq!(frames_on_disk(&copy), 6_000);
    // The tab still edits the source, with its history and stats.
    let file = state.get(1).unwrap();
    assert_eq!(
        (file.metadata.path.clone(), file.metadata.frames),
        (source, 6_000)
    );
    drop(file);
    assert_eq!(state.edit_state(1).unwrap(), before);
    assert!(before.dirty && before.can_undo);
    assert_eq!(whole_stats(&state, 1), edited_stats);
}

#[test]
fn export_refuses_the_audio_being_edited() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let path = dir.path().join("sine1.wav");
    let original = std::fs::read(&path).unwrap();
    state
        .apply_edit(1, EditOp::Crop, 2_000, 7_500, 0.0)
        .unwrap();
    let err = state.export(1, &path, |_, _| {}).unwrap_err();
    assert!(err.contains("can't be overwritten"), "{err}");
    assert_eq!(std::fs::read(&path).unwrap(), original);
    // No temporary files are left behind.
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, ["sine1.wav"]);
}

#[test]
fn exporting_again_replaces_an_earlier_export() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let copy = dir.path().join("sine1-export.wav");
    state.apply_edit(1, EditOp::Crop, 0, 6_000, 0.0).unwrap();
    state.export(1, &copy, |_, _| {}).unwrap();
    assert_eq!(frames_on_disk(&copy), 6_000);
    state.apply_edit(1, EditOp::Delete, 0, 1_000, 0.0).unwrap();
    state.export(1, &copy, |_, _| {}).unwrap();
    assert_eq!(frames_on_disk(&copy), 5_000);
    let mut names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["sine1-export.wav", "sine1.wav"]);
}

#[test]
fn a_failed_export_keeps_everything() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let original = std::fs::read(dir.path().join("sine1.wav")).unwrap();
    state.apply_edit(1, EditOp::Crop, 0, 1_000, 0.0).unwrap();
    let target = dir.path().join("missing-dir").join("x.wav");
    let err = state.export(1, &target, |_, _| {}).unwrap_err();
    assert!(err.starts_with("Could not export “x.wav”"), "{err}");
    let st = state.edit_state(1).unwrap();
    assert!(st.dirty && st.can_undo);
    assert_eq!(state.get(1).unwrap().metadata.frames, 1_000);
    assert_eq!(
        std::fs::read(dir.path().join("sine1.wav")).unwrap(),
        original
    );
}

fn clip(trim: (u64, u64), fade_in: (u64, f64), fade_out: (u64, f64)) -> ClipSettings {
    ClipSettings {
        trim_start: trim.0,
        trim_end: trim.1,
        fade_in: FadeSetting {
            len: fade_in.0,
            bend: fade_in.1,
        },
        fade_out: FadeSetting {
            len: fade_out.0,
            bend: fade_out.1,
        },
    }
}

#[test]
fn adjusting_a_fade_replaces_it_and_undo_restores_it() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let first = state
        .set_clip(1, clip((0, 8_000), (400, 0.3), (0, 0.0)))
        .unwrap();
    assert_eq!(first.clip.fade_in.len, 400);
    assert!(first.edit_state.dirty);
    let second = state
        .set_clip(1, clip((0, 8_000), (800, 0.3), (0, 0.0)))
        .unwrap();
    assert_eq!(second.clip.fade_in.len, 800);
    let back = state.undo(1).unwrap();
    assert_eq!(
        back.clip.fade_in,
        FadeSetting {
            len: 400,
            bend: 0.3
        }
    );
    // A drag that ends where it started adds nothing to the history.
    let same = state
        .set_clip(1, clip((0, 8_000), (400, 0.3), (0, 0.0)))
        .unwrap();
    assert!(same.edit_state.can_redo, "the redo history is untouched");
}

#[test]
fn trim_changes_the_length_and_can_be_restored() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let out = state
        .set_clip(1, clip((1_200, 8_000), (0, 0.0), (0, 0.0)))
        .unwrap();
    assert_eq!(out.metadata.frames, 6_800);
    assert_eq!(out.clip.untrimmed_frames, 8_000);
    let out = state
        .set_clip(1, clip((0, 8_000), (0, 0.0), (0, 0.0)))
        .unwrap();
    assert_eq!(out.metadata.frames, 8_000);
}

#[test]
fn delete_creates_a_join_whose_crossfade_is_clamped() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let out = state
        .apply_edit(1, EditOp::Delete, 4_000, 5_000, 0.0)
        .unwrap();
    let join = out.clip.joins[0];
    // 10 ms at 8 kHz.
    assert_eq!((join.position, join.len), (4_000, 80));
    let out = state
        .set_crossfade(
            1,
            join.index,
            FadeSetting {
                len: 1_000_000,
                bend: 0.5,
            },
        )
        .unwrap();
    assert_eq!(out.clip.joins[0].len, join.max_len);
    assert_eq!(out.clip.joins[0].bend, 0.5);
    assert!(state.set_crossfade(1, 0, FadeSetting::default()).is_err());
}

#[test]
fn exporting_writes_the_clip_settings_and_keeps_them() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    state
        .set_clip(1, clip((500, 7_500), (300, 0.0), (300, -0.5)))
        .unwrap();
    let shown = analysed_stats(&state, 1);
    let before = state.clip(1).unwrap();
    let copy = dir.path().join("shaped.wav");
    state.export(1, &copy, |_, _| {}).unwrap();
    assert_eq!(frames_on_disk(&copy), 7_000);
    // The handles stay where they were.
    assert_eq!(state.clip(1).unwrap(), before);
    assert_eq!((before.trim_start, before.trim_end), (500, 7_500));
    // The exported audio is what was shown, to within 16-bit rounding.
    state.open(2, &copy, |_, _| {}).unwrap();
    let exported = whole_stats(&state, 2);
    assert_eq!(exported.end, shown.end);
    for (e, s) in exported.channels.iter().zip(&shown.channels) {
        assert!((e.levels.peak_linear - s.levels.peak_linear).abs() < 1e-4);
        let (e, s) = (e.levels.rms_dbfs.unwrap(), s.levels.rms_dbfs.unwrap());
        assert!((e - s).abs() < 0.01, "{e} vs {s}");
    }
}

#[test]
fn previews_change_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let out = state
        .apply_edit(1, EditOp::Delete, 4_000, 5_000, 0.0)
        .unwrap();
    let before = (state.edit_state(1), state.clip(1));
    let audio = state.get(1).unwrap().audio.clone();
    let previews = [
        Preview::Clip {
            clip: clip((100, 7_000), (500, 0.4), (0, 0.0)),
        },
        Preview::Crossfade {
            join: out.clip.joins[0].index,
            xfade: FadeSetting {
                len: 400,
                bend: 0.0,
            },
        },
        Preview::Fade {
            op: EditOp::FadeOut,
            start: 1_000,
            end: 3_000,
            bend: -0.5,
        },
    ];
    for p in &previews {
        let heard = state.preview(1, p).unwrap();
        assert!(heard.same_source(&audio) && heard.edits() != audio.edits());
    }
    // The clip preview is what releasing the handle would apply.
    let heard = state.preview(1, &previews[0]).unwrap();
    assert_eq!(heard.frames(), 6_900);
    assert_eq!((state.edit_state(1), state.clip(1)), before);
    assert!(std::sync::Arc::ptr_eq(&state.get(1).unwrap().audio, &audio));
    // Only fades can be previewed from the menu.
    let crop = Preview::Fade {
        op: EditOp::Crop,
        start: 0,
        end: 10,
        bend: 0.0,
    };
    assert!(state.preview(1, &crop).is_err());
}

// --- Effect chains -------------------------------------------------------------------------

/// The test plugins, scanned into a plugin cache of their own.
struct Effects {
    _data: tempfile::TempDir,
    _folder: tempfile::TempDir,
    plugins: Arc<PluginCache>,
    /// HD Test Gain: any layout, 64 frames of latency, no bypass parameter.
    gain: ClassId,
    /// HD Test Stereo: stereo only.
    stereo: ClassId,
}

fn effects() -> Effects {
    let data = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    test_plugins::bundle(folder.path(), TestPlugin::Fx);
    let scanner = Scanner::new(env!("CARGO_BIN_EXE_hither-dither").into());
    let plugins = Arc::new(PluginCache::open(
        data.path().to_path_buf(),
        vec![folder.path().to_path_buf()],
        scanner,
    ));
    plugins.rescan(false, |_| {});
    let class = |name: &str| {
        let list = plugins.list().plugins;
        list.iter()
            .find(|p| p.name == name)
            .unwrap()
            .class_id
            .unwrap()
    };
    let (gain, stereo) = (class("HD Test Gain"), class("HD Test Stereo"));
    Effects {
        _data: data,
        _folder: folder,
        plugins,
        gain,
        stereo,
    }
}

fn slot_ids(chain: &ChainInfo) -> Vec<SlotId> {
    chain.effects.iter().map(|e| e.slot_id).collect()
}

const GAIN_PARAM: u32 = 0;

/// HD Test Gain's normalised value for +6.02 dB (it maps 0..1 onto ±24 dB).
fn plus_6db() -> f64 {
    (20.0 * 2f64.log10() + 24.0) / 48.0
}

fn gain_of(state: &AppState, id: u32, slot: SlotId) -> f64 {
    use vst3::Steinberg::Vst::IEditControllerTrait;
    let slot = state.slot(id, slot).unwrap();
    let controller = slot.instance().controller().unwrap();
    unsafe { controller.getParamNormalized(GAIN_PARAM) }
}

#[test]
fn undo_a_removal_brings_the_effect_back_with_its_settings() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let eq = slot_ids(&state.add_effect(1, &fx.plugins, fx.gain).unwrap().chain)[0];
    let both = state.add_effect(1, &fx.plugins, fx.gain).unwrap();
    assert_eq!(both.chain.effects.len(), 2);
    let slot = state.slot(1, eq).unwrap();
    slot.instance().set_param(GAIN_PARAM, plus_6db());
    drop(slot);

    let out = state.remove_effect(1, eq).unwrap();
    assert_eq!(out.chain.effects.len(), 1);
    let out = state.undo(1).unwrap();
    // Back in its place, and not an audio change.
    assert_eq!(slot_ids(&out.chain), slot_ids(&both.chain));
    assert!(!out.audio_changed);
    // The same instance, with the gain it had when removed.
    assert_eq!(gain_of(&state, 1, eq), plus_6db());
}

#[test]
fn edits_and_effects_share_one_history() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let silenced = state.apply_edit(1, EditOp::Silence, 0, 100, 0.0).unwrap();
    assert!(silenced.audio_changed);
    let added = state.add_effect(1, &fx.plugins, fx.gain).unwrap();
    assert_eq!(added.chain.effects[0].name, "HD Test Gain");
    assert_eq!(added.edit_state.revision, silenced.edit_state.revision);

    // The first undo removes the effect and leaves the silence.
    let out = state.undo(1).unwrap();
    assert!(out.chain.effects.is_empty() && !out.audio_changed);
    assert_eq!(out.edit_state.revision, silenced.edit_state.revision);
    assert!(out.edit_state.dirty && out.edit_state.can_undo && out.edit_state.can_redo);
    // The second removes the silence.
    let out = state.undo(1).unwrap();
    assert!(out.audio_changed && out.edit_state.revision > silenced.edit_state.revision);
    assert!(!out.edit_state.dirty && !out.edit_state.can_undo);

    // Redo brings both back in order.
    assert!(state.redo(1).unwrap().audio_changed);
    let out = state.redo(1).unwrap();
    assert_eq!(slot_ids(&out.chain), slot_ids(&added.chain));
}

#[test]
fn an_effect_alone_marks_the_file_unsaved() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let out = state.add_effect(1, &fx.plugins, fx.gain).unwrap();
    assert!(out.edit_state.dirty && out.edit_state.can_undo);
    assert!(!out.audio_changed);
    assert_eq!(out.edit_state.revision, 0);
    assert_eq!(out.metadata.frames, 8_000);
    assert_eq!(out.chain.latency, 64);
    assert_eq!(state.get_chain(1), Some(out.chain));
    assert!(state.edit_state(1).unwrap().dirty);
}

#[test]
fn bypassing_is_outside_the_history_but_unsaved() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let slot = slot_ids(&state.add_effect(1, &fx.plugins, fx.gain).unwrap().chain)[0];

    let out = state.set_effect_bypass(1, slot, true).unwrap();
    assert!(out.chain.effects[0].bypassed && out.edit_state.dirty);
    // Skipped by the host, so its latency no longer counts.
    assert_eq!(out.chain.latency, 0);
    assert!(state.get(1).unwrap().chain.processed().next().is_none());
    let out = state.set_effect_bypass(1, slot, false).unwrap();
    assert_eq!(out.chain.latency, 64);

    let out = state.set_bypass_all(1, true).unwrap();
    assert!(out.chain.bypass_all);
    assert!(!state.get(1).unwrap().chain.is_active());
    // Bypassing isn't an undo step: undo removes the effect, and Bypass All stays on.
    // Back at the state as opened, the tab is still unsaved: a switch changed meanwhile.
    let out = state.undo(1).unwrap();
    assert!(out.chain.effects.is_empty() && out.chain.bypass_all);
    assert!(out.edit_state.dirty && !out.edit_state.can_undo);
    assert!(state.set_effect_bypass(1, slot, true).is_err());
    let out = state.redo(1).unwrap();
    assert_eq!(slot_ids(&out.chain), [slot]);
}

#[test]
fn a_chain_has_no_length_limit() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let mut added = Vec::new();
    for _ in 0..10 {
        let out = state.add_effect(1, &fx.plugins, fx.gain).unwrap();
        added.push(*slot_ids(&out.chain).last().unwrap());
    }
    let chain = state.get_chain(1).unwrap();
    assert_eq!(slot_ids(&chain), added);
    assert_eq!(chain.latency, 10 * 64);
    // Each effect is its own step.
    assert_eq!(slot_ids(&state.undo(1).unwrap().chain), added[..9]);
}

#[test]
fn stereo_only_plugin_is_refused_for_a_5_1_file() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let s = sine(440.0, 0.5, 8_000, 1_000);
    let path = write(
        &dir,
        "surround.wav",
        WavBuilder::new(Encoding::Pcm, 16, 6, 8_000).extensible(true),
        &interleave(&vec![s; 6]),
    );
    let state = AppState::default();
    state.open(1, &path, |_, _| {}).unwrap();
    let err = state.add_effect(1, &fx.plugins, fx.stereo).unwrap_err();
    assert_eq!(err, "HD Test Stereo doesn't support 5.1 audio");
    assert!(state.get_chain(1).unwrap().effects.is_empty());
    let st = state.edit_state(1).unwrap();
    assert!(!st.can_undo && !st.dirty);
    // An effect that supports any layout is fine.
    assert!(state.add_effect(1, &fx.plugins, fx.gain).is_ok());
}

#[test]
fn chain_changes_do_not_start_an_analysis() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let edited = state.apply_edit(1, EditOp::FadeIn, 0, 1_000, 0.0).unwrap();
    assert!(state.wait_analysis(1, edited.edit_state.revision));
    let analysed = state.completed_analyses();
    let stats = whole_stats(&state, 1);

    let a = slot_ids(&state.add_effect(1, &fx.plugins, fx.gain).unwrap().chain)[0];
    let b = slot_ids(&state.add_effect(1, &fx.plugins, fx.gain).unwrap().chain)[1];
    let outcomes = [
        state.move_effect(1, b, 0).unwrap(),
        state.set_effect_bypass(1, a, true).unwrap(),
        state.set_bypass_all(1, true).unwrap(),
        state.remove_effect(1, a).unwrap(),
        state.undo(1).unwrap(),
    ];
    assert_eq!(slot_ids(&outcomes[0].chain), [b, a]);
    for out in &outcomes {
        assert!(!out.audio_changed);
        assert_eq!(out.edit_state.revision, edited.edit_state.revision);
    }
    // The analysis of the edited audio still applies, and none is scheduled.
    assert!(state.get(1).unwrap().analysis.is_some());
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert_eq!(state.completed_analyses(), analysed);
    assert_eq!(whole_stats(&state, 1), stats);
}

#[test]
fn moving_an_effect_reorders_the_chain() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    for _ in 0..3 {
        state.add_effect(1, &fx.plugins, fx.gain).unwrap();
    }
    let ids = slot_ids(&state.get_chain(1).unwrap());
    let out = state.move_effect(1, ids[2], 0).unwrap();
    assert_eq!(slot_ids(&out.chain), [ids[2], ids[0], ids[1]]);
    // Past the end means last.
    let out = state.move_effect(1, ids[2], 99).unwrap();
    assert_eq!(slot_ids(&out.chain), ids);
    // Where it already is adds no undo step.
    let out = state.move_effect(1, ids[2], 2).unwrap();
    assert!(!out.edit_state.can_redo);
    let out = state.undo(1).unwrap();
    assert_eq!(slot_ids(&out.chain), [ids[2], ids[0], ids[1]]);
    assert!(state.move_effect(1, 999, 0).is_err());
}

#[test]
fn effects_no_longer_in_the_history_are_released() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let a = slot_ids(&state.add_effect(1, &fx.plugins, fx.gain).unwrap().chain)[0];
    let kept = Arc::downgrade(&state.slot(1, a).unwrap());
    state.remove_effect(1, a).unwrap();
    // Still kept for undo.
    assert!(kept.upgrade().is_some());
    state.undo(1).unwrap();
    state.undo(1).unwrap();
    // A new step discards the redo history, and with it the effect.
    state.apply_edit(1, EditOp::Silence, 0, 10, 0.0).unwrap();
    assert!(state.slot(1, a).is_none());
    assert!(kept.upgrade().is_none());

    // Closing the file releases its effects, including those kept for undo.
    let b = slot_ids(&state.add_effect(1, &fx.plugins, fx.gain).unwrap().chain)[0];
    let c = slot_ids(&state.add_effect(1, &fx.plugins, fx.gain).unwrap().chain)[1];
    state.remove_effect(1, c).unwrap();
    let weak = [b, c].map(|s| Arc::downgrade(&state.slot(1, s).unwrap()));
    drop(state.close(1));
    assert!(weak.iter().all(|w| w.upgrade().is_none()));
}

// --- Saving through effects ----------------------------------------------------------------

/// A 10 s stereo 440 Hz tone peaking at `peak`, opened as file `id`.
fn open_tone(
    state: &AppState,
    dir: &tempfile::TempDir,
    id: u32,
    builder: WavBuilder,
    peak: f64,
) -> std::path::PathBuf {
    let s = sine(440.0, peak, 8_000, 80_000);
    let path = write(
        dir,
        &format!("tone{id}.wav"),
        builder,
        &interleave(&[s.clone(), s]),
    );
    state.open(id, &path, |_, _| {}).unwrap();
    path
}

/// Adds HD Test Gain to file `id`, set to +6.02 dB.
fn add_double(state: &AppState, fx: &Effects, id: u32) -> SlotId {
    let slot = slot_ids(&state.add_effect(id, &fx.plugins, fx.gain).unwrap().chain)[0];
    state
        .slot(id, slot)
        .unwrap()
        .instance()
        .set_param(GAIN_PARAM, plus_6db());
    slot
}

/// Every sample of the file at `path`, in the file's own units.
fn native_samples(path: &std::path::Path) -> Vec<f64> {
    let src = SampleSource::open(path).unwrap();
    let mut out = Vec::new();
    src.read_native(0, src.frames() as usize, &mut out);
    out
}

#[test]
fn effects_are_applied_with_latency_removed() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    let source = open_tone(
        &state,
        &dir,
        1,
        WavBuilder::new(Encoding::Pcm, 16, 2, 8_000),
        0.4,
    );
    add_double(&state, &fx, 1);
    assert_eq!(state.get_chain(1).unwrap().latency, 64);

    let copy = dir.path().join("doubled.wav");
    let mut last = (0, 0);
    let out = state
        .export(1, &copy, |done, total| last = (done, total))
        .unwrap();
    // Rendering, writing and analysing are one pass.
    assert_eq!(last, (80_000, 80_000));
    assert_eq!(out.clamped_samples, 0);
    assert_eq!(frames_on_disk(&copy), 80_000);
    let (src, saved) = (native_samples(&source), native_samples(&copy));
    assert_eq!(saved.len(), src.len());
    for (i, (s, d)) in src.iter().zip(&saved).enumerate() {
        assert_eq!(*d, (s * 2.0).clamp(-32_768.0, 32_767.0), "sample {i}");
    }
}

#[test]
fn a_bypassed_chain_exports_bit_exact() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    let source = open_tone(
        &state,
        &dir,
        1,
        WavBuilder::new(Encoding::Pcm, 24, 2, 8_000),
        0.4,
    );
    add_double(&state, &fx, 1);
    state.set_bypass_all(1, true).unwrap();

    let copy = dir.path().join("bypassed.wav");
    let out = state.export(1, &copy, |_, _| {}).unwrap();
    let frames = |p: &std::path::Path| {
        let src = SampleSource::open(p).unwrap();
        src.frame_bytes(0, src.frames() as usize).to_vec()
    };
    assert_eq!(frames(&copy), frames(&source));
    // The chain stays as it was.
    assert_eq!(out.clamped_samples, 0);
    assert_eq!(state.get_chain(1).unwrap().effects.len(), 1);
}

#[test]
fn too_much_gain_counts_the_clamped_samples() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    // -3 dBFS.
    let peak = 10f64.powf(-3.0 / 20.0);
    open_tone(
        &state,
        &dir,
        1,
        WavBuilder::new(Encoding::Pcm, 16, 2, 8_000),
        peak,
    );
    add_double(&state, &fx, 1);
    let copy = dir.path().join("clipped.wav");
    let out = state.export(1, &copy, |_, _| {}).unwrap();
    assert!(out.clamped_samples > 0);
    let saved = native_samples(&copy);
    let at_limit = saved
        .iter()
        .filter(|&&v| v == 32_767.0 || v == -32_768.0)
        .count() as u64;
    assert_eq!(at_limit, out.clamped_samples);
}

#[test]
fn a_float_file_keeps_its_overs() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    let peak = 10f64.powf(-3.0 / 20.0);
    open_tone(
        &state,
        &dir,
        1,
        WavBuilder::new(Encoding::Float, 32, 2, 8_000),
        peak,
    );
    add_double(&state, &fx, 1);
    let copy = dir.path().join("overs.wav");
    let out = state.export(1, &copy, |_, _| {}).unwrap();
    assert_eq!(out.clamped_samples, 0);
    let max = native_samples(&copy)
        .iter()
        .fold(0f64, |m, v| m.max(v.abs()));
    assert!(max > 1.4, "peak {max}");
}

#[test]
fn exporting_keeps_the_effects_and_their_settings() {
    use hither_dither_lib::fx::chain::LIVE_BLOCK;
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_tone(
        &state,
        &dir,
        1,
        WavBuilder::new(Encoding::Pcm, 16, 2, 8_000),
        0.4,
    );
    let slot = add_double(&state, &fx, 1);
    let dry = whole_stats(&state, 1);
    let (chain, st) = (state.get_chain(1).unwrap(), state.edit_state(1).unwrap());

    let copy = dir.path().join("processed.wav");
    state.export(1, &copy, |_, _| {}).unwrap();
    // The same effect, with its setting, back to playing in real time.
    assert_eq!(state.get_chain(1).unwrap(), chain);
    assert_eq!(gain_of(&state, 1, slot), plus_6db());
    let config = state.slot(1, slot).unwrap().instance().config().unwrap();
    assert!(!config.offline && config.max_block == LIVE_BLOCK);
    assert_eq!(state.edit_state(1).unwrap(), st);
    // The tab's statistics still describe the audio before effects; the export has them.
    assert_eq!(whole_stats(&state, 1), dry);
    state.open(2, &copy, |_, _| {}).unwrap();
    let peak = |s: &hither_dither_lib::stats::Stats| s.channels[0].levels.peak_dbfs.unwrap();
    assert!((peak(&whole_stats(&state, 2)) - peak(&dry) - 6.02).abs() < 0.01);
}

#[test]
fn a_failed_export_with_effects_keeps_the_chain() {
    use hither_dither_lib::fx::chain::LIVE_BLOCK;
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_tone(
        &state,
        &dir,
        1,
        WavBuilder::new(Encoding::Pcm, 16, 2, 8_000),
        0.4,
    );
    state.apply_edit(1, EditOp::FadeIn, 0, 1_000, 0.0).unwrap();
    let slot = add_double(&state, &fx, 1);
    let before = state.get_chain(1).unwrap();

    // The folder can't be written to.
    let target = dir.path().join("missing-dir").join("x.wav");
    let err = state.export(1, &target, |_, _| {}).unwrap_err();
    assert!(err.starts_with("Could not export “x.wav”"), "{err}");
    // The chain, the effect's settings and the edits are kept.
    assert_eq!(state.get_chain(1).unwrap(), before);
    assert_eq!(gain_of(&state, 1, slot), plus_6db());
    let st = state.edit_state(1).unwrap();
    assert!(st.dirty && st.can_undo);
    // The effect is back to processing for playback.
    let config = state.slot(1, slot).unwrap().instance().config().unwrap();
    assert!(!config.offline && config.max_block == LIVE_BLOCK);
    // And a later export still applies it.
    let copy = dir.path().join("retry.wav");
    state.export(1, &copy, |_, _| {}).unwrap();
    assert!(native_samples(&copy).iter().any(|v| v.abs() > 16_384.0));
}

/// Runs `frames` frames of 0.25 in every channel through `slot`, as playback would, and
/// returns the first channel of the output.
fn process_quarter(state: &AppState, id: u32, slot: SlotId, frames: usize) -> Vec<f32> {
    use hither_dither_lib::plugins::instance::Transport;
    let slot = state.slot(id, slot).unwrap();
    let instance = slot.instance();
    let channels = instance.config().unwrap().channels as usize;
    let input = vec![vec![0.25f32; frames]; channels];
    let mut output = vec![vec![0.0f32; frames]; channels];
    let ins: Vec<&[f32]> = input.iter().map(|c| c.as_slice()).collect();
    let mut outs: Vec<&mut [f32]> = output.iter_mut().map(|c| c.as_mut_slice()).collect();
    assert!(instance.process(&ins, &mut outs, frames, Transport::default()));
    output.swap_remove(0)
}

#[test]
fn generic_controls_list_the_gain_parameter() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let slot = slot_ids(&state.add_effect(1, &fx.plugins, fx.gain).unwrap().chain)[0];

    let params = state.fx_params(1, slot).unwrap();
    assert_eq!(params.len(), 1);
    let gain = &params[0];
    assert_eq!(
        (gain.id, gain.title.as_str(), gain.units.as_str()),
        (GAIN_PARAM, "Gain", "dB")
    );
    assert_eq!((gain.step_count, gain.default_value), (0, 0.5));
    assert_eq!((gain.value, gain.display.as_str()), (0.5, "0.0"));

    // Only effects in the chain have controls.
    state.remove_effect(1, slot).unwrap();
    assert_eq!(
        state.fx_params(1, slot).unwrap_err(),
        "That effect isn't in the chain"
    );
}

#[test]
fn a_generic_control_is_heard_from_the_next_block() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let slot = slot_ids(&state.add_effect(1, &fx.plugins, fx.gain).unwrap().chain)[0];

    // At its default the gain passes audio through, after its 64 frames of latency.
    let first = process_quarter(&state, 1, slot, 256);
    assert!(first[64..].iter().all(|&v| v == 0.25));

    let shown = state.set_fx_param(1, slot, GAIN_PARAM, plus_6db()).unwrap();
    assert_eq!(shown, "6.0");
    assert_eq!(gain_of(&state, 1, slot), plus_6db());
    // The whole next block is doubled.
    let next = process_quarter(&state, 1, slot, 256);
    assert!(next.iter().all(|&v| (v - 0.5).abs() < 1e-6), "{next:?}");
}

#[test]
fn generic_control_changes_are_not_undo_steps() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let slot = slot_ids(&state.add_effect(1, &fx.plugins, fx.gain).unwrap().chain)[0];
    let before = state.edit_state(1).unwrap();

    state.set_fx_param(1, slot, GAIN_PARAM, plus_6db()).unwrap();
    assert_eq!(state.edit_state(1).unwrap(), before);
    // Undo takes the effect away, not the parameter change, and redo brings it back as set.
    let undone = state.undo(1).unwrap();
    assert!(undone.chain.effects.is_empty());
    state.redo(1).unwrap();
    assert_eq!(gain_of(&state, 1, slot), plus_6db());
}

#[test]
fn changes_in_a_plugin_editor_reach_the_generic_controls() {
    use std::time::{Duration, Instant};
    use vst3::Steinberg::Vst::IComponentHandlerTrait;
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let slot = slot_ids(&state.add_effect(1, &fx.plugins, fx.gain).unwrap().chain)[0];
    let (tx, rx) = std::sync::mpsc::channel();
    state.params.on_changed(move |changed| {
        let _ = tx.send(changed);
    });

    // As the plugin's own editor reports a knob being turned.
    let instance = state.slot(1, slot).unwrap();
    let handler = instance.instance().handler();
    unsafe {
        handler.performEdit(GAIN_PARAM, 0.25);
        handler.performEdit(GAIN_PARAM, plus_6db());
    }

    // The latest value arrives with its display string, possibly after the first.
    let deadline = Instant::now() + Duration::from_secs(5);
    let last = loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let changed = rx.recv_timeout(left).expect("no fx-param-changed");
        assert_eq!((changed.file_id, changed.slot_id), (1, slot));
        let change = changed.changes.last().unwrap().clone();
        if change.value == plus_6db() {
            break change;
        }
    };
    assert_eq!((last.id, last.display.as_str()), (GAIN_PARAM, "6.0"));
    // And the processor hears it (after the plugin's latency, which starts out silent).
    let out = process_quarter(&state, 1, slot, 256);
    assert!(out[64..].iter().all(|&v| (v - 0.5).abs() < 1e-6));
}

// --- Session files -------------------------------------------------------------------------

/// Saves file `id`'s session as `name` in `dir` and reads it back.
fn save_session(state: &AppState, dir: &tempfile::TempDir, id: u32, name: &str) -> SessionFile {
    let path = dir.path().join(name);
    state.save_session(id, &path, true).unwrap();
    SessionFile::read(&path).unwrap()
}

#[test]
fn a_session_captures_the_edits_and_effects() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let wav = dir.path().join("sine1.wav");
    let before = std::fs::read(&wav).unwrap();
    state
        .apply_edit(1, EditOp::Delete, 1_000, 2_000, 0.0)
        .unwrap();
    state
        .apply_edit(1, EditOp::FadeOut, 5_000, 7_000, 0.4)
        .unwrap();
    let first = add_double(&state, &fx, 1);
    let second = slot_ids(&state.add_effect(1, &fx.plugins, fx.gain).unwrap().chain)[1];
    state.set_effect_bypass(1, second, true).unwrap();

    let session = save_session(&state, &dir, 1, "mix.hdsession.json");
    // The edits as they are, including the crossfade at the join.
    let edits = state.get(1).unwrap().audio.edits().clone();
    assert_eq!(session.edit_list(), *edits);
    assert!(session.edit_list().joins()[0].xfade.len > 0);
    // Both effects in order, the second bypassed, with the first one's gain in its state.
    let names: Vec<_> = session
        .chain
        .effects
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(names, ["HD Test Gain", "HD Test Gain"]);
    let bypassed: Vec<_> = session.chain.effects.iter().map(|e| e.bypassed).collect();
    assert_eq!(bypassed, [false, true]);
    assert!(session.chain.effects.iter().all(|e| e.class_id == fx.gain));
    assert!(!session.chain.effects[0].state.component.is_empty());
    assert_ne!(first, second);
    // The WAV it points to, described as loaded, and left exactly as it was.
    assert_eq!(session.audio.path, wav);
    assert_eq!(session.audio.relative_path.as_deref(), Some("sine1.wav"));
    assert_eq!(
        (session.audio.sample_rate, session.audio.channels),
        (8_000, 2)
    );
    assert_eq!((session.audio.frames, session.audio.bit_depth), (8_000, 16));
    assert_eq!(std::fs::read(&wav).unwrap(), before);
}

#[test]
fn a_clean_tab_saves_an_empty_session() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let session = save_session(&state, &dir, 1, "clean.hdsession.json");
    assert!(session.edit_list().is_identity());
    assert!(session.chain.effects.is_empty() && !session.chain.bypass_all);
}

#[test]
fn a_session_includes_changes_made_while_stopped() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let slot = slot_ids(&state.add_effect(1, &fx.plugins, fx.gain).unwrap().chain)[0];
    // Set in the generic controls without playing: the processor hasn't had it yet.
    state.set_fx_param(1, slot, GAIN_PARAM, plus_6db()).unwrap();
    let session = save_session(&state, &dir, 1, "mix.hdsession.json");
    assert_eq!(
        session.chain.effects[0].pending_params,
        [(GAIN_PARAM, plus_6db())]
    );
    // Saving leaves it pending for playback (after the latency, which starts out silent).
    let next = process_quarter(&state, 1, slot, 256);
    assert!(next[64..].iter().all(|&v| (v - 0.5).abs() < 1e-6));
}

#[test]
fn saving_a_session_replaces_the_file_only_when_complete() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let path = dir.path().join("mix.hdsession.json");
    std::fs::write(&path, "old").unwrap();
    // Without permission to replace it, an existing file is left alone.
    assert!(!state.save_session(1, &path, false).unwrap().written);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "old");
    assert!(state.save_session(1, &path, true).unwrap().written);
    assert!(SessionFile::read(&path).is_ok());

    let missing = dir.path().join("missing").join("mix.hdsession.json");
    let err = state.save_session(1, &missing, true).unwrap_err();
    assert!(
        err.starts_with("Could not save “mix.hdsession.json”"),
        "{err}"
    );
    // No temporary files are left behind, and the existing session is intact.
    let mut names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["mix.hdsession.json", "sine1.wav"]);
    assert!(SessionFile::read(&path).is_ok());
}

/// Rewrites the session file at `path` through its JSON.
fn edit_session(path: &std::path::Path, edit: impl FnOnce(&mut serde_json::Value)) {
    let mut json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    edit(&mut json);
    std::fs::write(path, json.to_string()).unwrap();
}

fn effect_names(chain: &ChainInfo) -> Vec<&str> {
    chain.effects.iter().map(|e| e.name.as_str()).collect()
}

#[test]
fn a_restored_session_sounds_and_saves_the_same() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    let source = open_tone(
        &state,
        &dir,
        1,
        WavBuilder::new(Encoding::Pcm, 16, 2, 8_000),
        0.4,
    );
    state
        .apply_edit(1, EditOp::Delete, 10_000, 20_000, 0.0)
        .unwrap();
    state
        .apply_edit(1, EditOp::FadeOut, 50_000, 70_000, 0.3)
        .unwrap();
    add_double(&state, &fx, 1);
    let session = dir.path().join("tone.hdsession.json");
    state.save_session(1, &session, true).unwrap();
    let (clip, chain) = (state.clip(1).unwrap(), state.get_chain(1).unwrap());
    let first = dir.path().join("first.wav");
    state.export(1, &first, |_, _| {}).unwrap();

    // Reopened from the session in another tab.
    state.open(2, &source, |_, _| {}).unwrap();
    let out = state
        .restore_session(2, &session, false, &fx.plugins)
        .unwrap();
    assert!(out.warnings.is_empty() && !out.wav_changed);
    // A fresh history at the restored state, which matches its session file.
    assert!(!out.edit_state.dirty && !out.edit_state.can_undo && !out.edit_state.can_redo);
    assert_eq!(state.clip(2).unwrap(), clip);
    let restored = state.get_chain(2).unwrap();
    assert_eq!(effect_names(&restored), effect_names(&chain));
    assert_eq!(restored.latency, chain.latency);
    assert_eq!(gain_of(&state, 2, slot_ids(&restored)[0]), plus_6db());
    // The next change can be undone back to the restored state, not past it.
    let silenced = state.apply_edit(2, EditOp::Silence, 0, 100, 0.0).unwrap();
    assert!(silenced.edit_state.can_undo);
    assert!(silenced.edit_state.dirty);
    let undone = state.undo(2).unwrap();
    assert!(!undone.edit_state.can_undo && !undone.edit_state.dirty);
    assert_eq!(state.clip(2).unwrap(), clip);

    // Saved through the restored chain, the audio is identical.
    let second = dir.path().join("second.wav");
    state.export(2, &second, |_, _| {}).unwrap();
    assert_eq!(native_samples(&second), native_samples(&first));
}

#[test]
fn a_long_chain_is_restored_in_full() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    for i in 0..12 {
        let out = state.add_effect(1, &fx.plugins, fx.gain).unwrap();
        // Every third one bypassed, so the order can be told apart.
        if i % 3 == 0 {
            let slot = *slot_ids(&out.chain).last().unwrap();
            state.set_effect_bypass(1, slot, true).unwrap();
        }
    }
    let session = dir.path().join("long.hdsession.json");
    state.save_session(1, &session, true).unwrap();

    state
        .open(2, &dir.path().join("sine1.wav"), |_, _| {})
        .unwrap();
    let out = state
        .restore_session(2, &session, false, &fx.plugins)
        .unwrap();
    assert!(out.warnings.is_empty(), "{:?}", out.warnings);
    let bypassed = |c: &ChainInfo| c.effects.iter().map(|e| e.bypassed).collect::<Vec<_>>();
    let restored = state.get_chain(2).unwrap();
    assert_eq!(restored.effects.len(), 12);
    assert_eq!(bypassed(&restored), bypassed(&state.get_chain(1).unwrap()));
}

#[test]
fn a_restored_session_with_only_bypassed_effects_is_not_unsaved() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let slot = slot_ids(&state.add_effect(1, &fx.plugins, fx.gain).unwrap().chain)[0];
    state.set_effect_bypass(1, slot, true).unwrap();
    let session = dir.path().join("mix.hdsession.json");
    state.save_session(1, &session, true).unwrap();
    state.close(1);

    state
        .open(2, &dir.path().join("sine1.wav"), |_, _| {})
        .unwrap();
    let out = state
        .restore_session(2, &session, false, &fx.plugins)
        .unwrap();
    assert!(!out.edit_state.dirty && !out.edit_state.can_undo);
    let chain = state.get_chain(2).unwrap();
    assert!(chain.effects[0].bypassed && !chain.bypass_all);
    // Skipped by the host, as when bypassed by hand.
    assert_eq!(chain.latency, 0);
}

#[test]
fn a_session_is_not_applied_to_a_different_recording() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    state.apply_edit(1, EditOp::Silence, 0, 100, 0.0).unwrap();
    let session = dir.path().join("mix.hdsession.json");
    state.save_session(1, &session, true).unwrap();
    state.close(1);

    // Re-rendered shorter after the session was saved.
    let s = sine(440.0, 0.5, 8_000, 7_000);
    let wav = write(
        &dir,
        "sine1.wav",
        WavBuilder::new(Encoding::Pcm, 16, 2, 8_000),
        &interleave(&[s.clone(), s]),
    );
    state.open(2, &wav, |_, _| {}).unwrap();
    let out = state
        .restore_session(2, &session, false, &fx.plugins)
        .unwrap();
    // Not applied, so the frontend can offer to choose another WAV.
    assert!(!out.applied);
    let st = state.edit_state(2).unwrap();
    assert!(!st.dirty && !st.can_undo && !st.edited);
    assert_eq!(state.get(2).unwrap().metadata.frames, 7_000);
}

#[test]
fn a_touched_wav_is_restored_with_a_warning() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    state.apply_edit(1, EditOp::Silence, 0, 100, 0.0).unwrap();
    let session = dir.path().join("mix.hdsession.json");
    state.save_session(1, &session, true).unwrap();
    state.close(1);

    // Same audio and format, written again later.
    let wav = dir.path().join("sine1.wav");
    std::thread::sleep(std::time::Duration::from_millis(50));
    std::fs::write(&wav, std::fs::read(&wav).unwrap()).unwrap();
    state.open(2, &wav, |_, _| {}).unwrap();
    let out = state
        .restore_session(2, &session, false, &fx.plugins)
        .unwrap();
    assert!(out.wav_changed);
    assert!(!out.edit_state.dirty);
}

#[test]
fn effects_that_cant_be_found_are_left_out() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    let wav = open_tone(
        &state,
        &dir,
        1,
        WavBuilder::new(Encoding::Pcm, 16, 2, 8_000),
        0.4,
    );
    add_double(&state, &fx, 1);
    state.add_effect(1, &fx.plugins, fx.stereo).unwrap();
    let session = dir.path().join("mix.hdsession.json");
    state.save_session(1, &session, true).unwrap();
    // An effect that has since been uninstalled, between the two.
    edit_session(&session, |json| {
        let effects = json["chain"]["effects"].as_array_mut().unwrap();
        let mut gone = effects[0].clone();
        gone["classId"] = "00000000000000000000000000000000".into();
        gone["name"] = "Gone FX".into();
        effects.insert(1, gone);
    });

    state.open(2, &wav, |_, _| {}).unwrap();
    let out = state
        .restore_session(2, &session, false, &fx.plugins)
        .unwrap();
    let chain = state.get_chain(2).unwrap();
    assert_eq!(effect_names(&chain), ["HD Test Gain", "HD Test Stereo"]);
    assert_eq!(gain_of(&state, 2, slot_ids(&chain)[0]), plus_6db());
    assert_eq!(out.warnings.len(), 1);
    assert_eq!(out.warnings[0].effect, "Gone FX");
    assert!(out.warnings[0].reason.contains("isn't available"));
}

#[test]
fn an_effect_that_rejects_its_settings_keeps_its_defaults() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    add_double(&state, &fx, 1);
    let session = dir.path().join("mix.hdsession.json");
    state.save_session(1, &session, true).unwrap();
    edit_session(&session, |json| {
        let effect = &mut json["chain"]["effects"][0];
        // Three bytes: not a gain value.
        effect["state"]["component"] = "AQID".into();
        effect["pendingParams"] = serde_json::json!([]);
    });

    state
        .open(2, &dir.path().join("sine1.wav"), |_, _| {})
        .unwrap();
    let out = state
        .restore_session(2, &session, false, &fx.plugins)
        .unwrap();
    let chain = state.get_chain(2).unwrap();
    assert_eq!(effect_names(&chain), ["HD Test Gain"]);
    assert_eq!(gain_of(&state, 2, slot_ids(&chain)[0]), 0.5);
    assert_eq!(out.warnings.len(), 1);
    assert!(out.warnings[0].reason.contains("could not be restored"));
}

#[test]
fn a_stereo_only_effect_is_left_out_of_a_5_1_session() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let s = sine(440.0, 0.5, 8_000, 1_000);
    let path = write(
        &dir,
        "surround.wav",
        WavBuilder::new(Encoding::Pcm, 16, 6, 8_000).extensible(true),
        &interleave(&vec![s; 6]),
    );
    let state = AppState::default();
    state.open(1, &path, |_, _| {}).unwrap();
    state.add_effect(1, &fx.plugins, fx.gain).unwrap();
    let session = dir.path().join("surround.hdsession.json");
    state.save_session(1, &session, true).unwrap();
    edit_session(&session, |json| {
        json["chain"]["effects"][0]["classId"] = fx.stereo.to_string().into();
        json["chain"]["effects"][0]["name"] = "HD Test Stereo".into();
    });

    state.open(2, &path, |_, _| {}).unwrap();
    let out = state
        .restore_session(2, &session, false, &fx.plugins)
        .unwrap();
    assert!(state.get_chain(2).unwrap().effects.is_empty());
    assert_eq!(out.warnings[0].effect, "HD Test Stereo");
    assert_eq!(
        out.warnings[0].reason,
        "HD Test Stereo doesn't support 5.1 audio"
    );
}

#[test]
fn restored_edits_are_analysed() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    state.apply_edit(1, EditOp::Silence, 0, 4_000, 0.0).unwrap();
    let expected = analysed_stats(&state, 1);
    let session = dir.path().join("mix.hdsession.json");
    state.save_session(1, &session, true).unwrap();

    state
        .open(2, &dir.path().join("sine1.wav"), |_, _| {})
        .unwrap();
    let out = state
        .restore_session(2, &session, false, &fx.plugins)
        .unwrap();
    assert!(out.edit_state.revision > 0);
    assert_eq!(analysed_stats(&state, 2), expected);
}

// --- Unsaved changes, measured against the saved session ---------------------------------

#[test]
fn an_edit_marks_the_tab_and_undoing_it_clears_the_mark() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    assert!(!state.edit_state(1).unwrap().dirty, "fresh from the WAV");
    let out = state.apply_edit(1, EditOp::Silence, 0, 100, 0.0).unwrap();
    assert!(out.edit_state.dirty);
    assert!(!state.undo(1).unwrap().edit_state.dirty);
    assert!(state.redo(1).unwrap().edit_state.dirty);
}

#[test]
fn a_plugin_setting_marks_the_tab_until_saved() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let slot = slot_ids(&state.add_effect(1, &fx.plugins, fx.gain).unwrap().chain)[0];
    state.undo(1).unwrap();
    state.redo(1).unwrap();
    // Back where it was after adding: the effect alone is the unsaved change.
    assert!(state.edit_state(1).unwrap().dirty);

    // From the generic controls.
    state.set_fx_param(1, slot, GAIN_PARAM, plus_6db()).unwrap();
    let st = state.edit_state(1).unwrap();
    assert!(st.dirty && st.can_undo);
    // Undoing to the state as opened doesn't take back the setting, so it stays unsaved.
    assert!(state.undo(1).unwrap().edit_state.dirty);
}

#[test]
fn a_change_in_a_plugin_editor_marks_the_tab() {
    use vst3::Steinberg::Vst::IComponentHandlerTrait;
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let slot = slot_ids(&state.add_effect(1, &fx.plugins, fx.gain).unwrap().chain)[0];
    state.set_effect_bypass(1, slot, true).unwrap();
    let session = dir.path().join("mix.hdsession.json");
    state.save_session(1, &session, true).unwrap();
    // Reopened from the session: clean until a knob turns in the editor.
    state
        .open(2, &dir.path().join("sine1.wav"), |_, _| {})
        .unwrap();
    state
        .restore_session(2, &session, false, &fx.plugins)
        .unwrap();
    assert!(!state.edit_state(2).unwrap().dirty);
    let restored = slot_ids(&state.get_chain(2).unwrap())[0];
    let instance = state.slot(2, restored).unwrap();
    unsafe { instance.instance().handler().performEdit(GAIN_PARAM, 0.25) };
    assert!(state.edit_state(2).unwrap().dirty);
}

#[test]
fn a_change_after_restoring_marks_the_tab() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    state.add_effect(1, &fx.plugins, fx.gain).unwrap();
    let session = dir.path().join("mix.hdsession.json");
    state.save_session(1, &session, true).unwrap();
    state.close(1);

    state
        .open(2, &dir.path().join("sine1.wav"), |_, _| {})
        .unwrap();
    let out = state
        .restore_session(2, &session, false, &fx.plugins)
        .unwrap();
    assert!(!out.edit_state.dirty && !out.edit_state.can_undo);
    let slot = slot_ids(&state.get_chain(2).unwrap())[0];
    assert!(
        state
            .set_effect_bypass(2, slot, true)
            .unwrap()
            .edit_state
            .dirty
    );
}

#[test]
fn saving_the_session_clears_the_unsaved_mark() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let wav = dir.path().join("sine1.wav");
    let before = std::fs::read(&wav).unwrap();
    state.apply_edit(1, EditOp::Silence, 0, 100, 0.0).unwrap();
    let path = dir.path().join("mix.hdsession.json");
    let saved = state.save_session(1, &path, true).unwrap();
    assert!(saved.written && !saved.edit_state.dirty);
    // The history stays, and the WAV is as it was.
    assert!(saved.edit_state.can_undo);
    assert_eq!(std::fs::read(&wav).unwrap(), before);

    // Undo leaves the saved state; redo comes back to it.
    assert!(state.undo(1).unwrap().edit_state.dirty);
    assert!(!state.redo(1).unwrap().edit_state.dirty);
}

#[test]
fn a_session_not_written_leaves_the_tab_unsaved() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    state.apply_edit(1, EditOp::Silence, 0, 100, 0.0).unwrap();
    let path = dir.path().join("mix.hdsession.json");
    std::fs::write(&path, "old").unwrap();
    let saved = state.save_session(1, &path, false).unwrap();
    assert!(!saved.written && saved.edit_state.dirty);
    // A failed write doesn't count as saved either.
    let missing = dir.path().join("missing").join("mix.hdsession.json");
    assert!(state.save_session(1, &missing, true).is_err());
    assert!(state.edit_state(1).unwrap().dirty);
}

// --- Choosing another WAV for a session ----------------------------------------------------

/// A session of `sine1.wav` with a silence edit and HD Test Gain at +6 dB, with the tab closed.
fn edited_session(state: &AppState, fx: &Effects, dir: &tempfile::TempDir) -> std::path::PathBuf {
    open_sine(state, dir, 1, 8_000);
    state.apply_edit(1, EditOp::Silence, 0, 100, 0.0).unwrap();
    add_double(state, fx, 1);
    let session = dir.path().join("mix.hdsession.json");
    state.save_session(1, &session, true).unwrap();
    state.close(1);
    session
}

#[test]
fn a_renamed_wav_keeps_the_session_edits() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    let session = edited_session(&state, &fx, &dir);
    let renamed = dir.path().join("sine1-final.wav");
    std::fs::rename(dir.path().join("sine1.wav"), &renamed).unwrap();

    state.open(2, &renamed, |_, _| {}).unwrap();
    let out = state
        .restore_session(2, &session, true, &fx.plugins)
        .unwrap();
    assert!(out.applied && !out.edits_dropped && out.warnings.is_empty());
    // The edits, the effect with its setting, and unsaved: the session points elsewhere now.
    assert!(out.edit_state.edited && out.edit_state.dirty && !out.edit_state.can_undo);
    let chain = state.get_chain(2).unwrap();
    assert_eq!(gain_of(&state, 2, slot_ids(&chain)[0]), plus_6db());
}

#[test]
fn a_different_recording_keeps_only_the_effects() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    let session = edited_session(&state, &fx, &dir);
    let s = sine(440.0, 0.5, 8_000, 5_000);
    let other = write(
        &dir,
        "other.wav",
        WavBuilder::new(Encoding::Pcm, 16, 2, 8_000),
        &interleave(&[s.clone(), s]),
    );

    state.open(2, &other, |_, _| {}).unwrap();
    let out = state
        .restore_session(2, &session, true, &fx.plugins)
        .unwrap();
    assert!(out.applied && out.edits_dropped);
    assert!(!out.edit_state.edited && out.edit_state.dirty);
    assert_eq!(state.get(2).unwrap().metadata.frames, 5_000);
    let chain = state.get_chain(2).unwrap();
    assert_eq!(effect_names(&chain), ["HD Test Gain"]);
    assert_eq!(gain_of(&state, 2, slot_ids(&chain)[0]), plus_6db());
}

#[test]
fn edited_follows_the_current_edits() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    let session = edited_session(&state, &fx, &dir);
    state
        .open(2, &dir.path().join("sine1.wav"), |_, _| {})
        .unwrap();
    let out = state
        .restore_session(2, &session, false, &fx.plugins)
        .unwrap();
    // Restored edits with nothing to undo.
    assert!(out.edit_state.edited && !out.edit_state.can_undo);

    open_sine(&state, &dir, 3, 8_000);
    assert!(!state.edit_state(3).unwrap().edited);
    assert!(
        state
            .apply_edit(3, EditOp::Silence, 0, 10, 0.0)
            .unwrap()
            .edit_state
            .edited
    );
    assert!(!state.undo(3).unwrap().edit_state.edited);
}

#[test]
fn a_missing_wav_is_reported_by_name() {
    use hither_dither_lib::session::read_info;
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    let session = edited_session(&state, &fx, &dir);
    let found = read_info(&session).unwrap();
    assert_eq!(found.audio_path, Some(dir.path().join("sine1.wav")));
    std::fs::remove_file(dir.path().join("sine1.wav")).unwrap();
    let missing = read_info(&session).unwrap();
    assert_eq!(missing.audio_path, None);
    assert_eq!(missing.audio_name, "sine1.wav");
}

// --- Replacing a tab's audio ---------------------------------------------------------------

/// A WAV of a 16-bit sine at `peak`, `frames` long, with `channels` channels.
fn tone_file(
    dir: &tempfile::TempDir,
    name: &str,
    channels: u16,
    frames: usize,
    peak: f64,
) -> std::path::PathBuf {
    let s = sine(330.0, peak, 8_000, frames);
    write(
        dir,
        name,
        WavBuilder::new(Encoding::Pcm, 16, channels, 8_000),
        &interleave(&vec![s; channels as usize]),
    )
}

#[test]
fn replacing_the_audio_keeps_the_effects() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_tone(
        &state,
        &dir,
        1,
        WavBuilder::new(Encoding::Pcm, 16, 2, 8_000),
        0.4,
    );
    add_double(&state, &fx, 1);
    let second = slot_ids(&state.add_effect(1, &fx.plugins, fx.stereo).unwrap().chain)[1];
    state.set_effect_bypass(1, second, true).unwrap();
    state.set_bypass_all(1, false).unwrap();
    state.apply_edit(1, EditOp::FadeIn, 0, 1_000, 0.0).unwrap();
    let before = state.get_chain(1).unwrap();

    let take2 = tone_file(&dir, "take2.wav", 2, 20_000, 0.3);
    let out = state
        .replace_audio(1, &take2, &fx.plugins, |_, _| {})
        .unwrap();
    assert!(out.warnings.is_empty());
    assert_eq!(
        (out.metadata.name.as_str(), out.metadata.frames),
        ("take2.wav", 20_000)
    );
    // The same effects in order, with their settings and bypass switches; new instances.
    let chain = state.get_chain(1).unwrap();
    assert_eq!(effect_names(&chain), effect_names(&before));
    let bypassed: Vec<_> = chain.effects.iter().map(|e| e.bypassed).collect();
    assert_eq!(bypassed, [false, true]);
    assert_ne!(slot_ids(&chain), slot_ids(&before));
    assert_eq!(gain_of(&state, 1, slot_ids(&chain)[0]), plus_6db());
    // The edits are gone and the history starts again, unsaved.
    let st = out.edit_state;
    assert!(!st.edited && !st.can_undo && !st.can_redo && st.dirty);
    assert_eq!(state.get(1).unwrap().metadata.path, take2);

    // Exported through the kept gain.
    let copy = dir.path().join("take2-export.wav");
    state.export(1, &copy, |_, _| {}).unwrap();
    let (src, done) = (native_samples(&take2), native_samples(&copy));
    for (s, d) in src.iter().zip(&done) {
        assert_eq!(*d, (s * 2.0).clamp(-32_768.0, 32_767.0));
    }
}

#[test]
fn effects_that_dont_fit_the_new_audio_are_left_out() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    add_double(&state, &fx, 1);
    state.add_effect(1, &fx.plugins, fx.stereo).unwrap();
    let mono = tone_file(&dir, "mono.wav", 1, 8_000, 0.3);
    let out = state
        .replace_audio(1, &mono, &fx.plugins, |_, _| {})
        .unwrap();
    assert_eq!(effect_names(&state.get_chain(1).unwrap()), ["HD Test Gain"]);
    assert_eq!(out.warnings.len(), 1);
    assert_eq!(out.warnings[0].effect, "HD Test Stereo");
    assert_eq!(
        out.warnings[0].reason,
        "HD Test Stereo doesn't support mono audio"
    );
}

#[test]
fn replacing_marks_a_saved_tab_unsaved() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    state.add_effect(1, &fx.plugins, fx.gain).unwrap();
    let session = dir.path().join("mix.hdsession.json");
    assert!(
        !state
            .save_session(1, &session, true)
            .unwrap()
            .edit_state
            .dirty
    );
    let take2 = tone_file(&dir, "take2.wav", 2, 8_000, 0.3);
    let out = state
        .replace_audio(1, &take2, &fx.plugins, |_, _| {})
        .unwrap();
    assert!(out.edit_state.dirty);
    // Saving now points the session at the new WAV.
    state.save_session(1, &session, true).unwrap();
    assert_eq!(SessionFile::read(&session).unwrap().audio.path, take2);
}

#[test]
fn a_failed_replacement_changes_nothing() {
    let fx = effects();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let slot = add_double(&state, &fx, 1);
    state.apply_edit(1, EditOp::Silence, 0, 100, 0.0).unwrap();
    let (chain, st) = (state.get_chain(1).unwrap(), state.edit_state(1).unwrap());

    let junk = dir.path().join("notes.wav");
    std::fs::write(&junk, b"not audio").unwrap();
    let err = state
        .replace_audio(1, &junk, &fx.plugins, |_, _| {})
        .unwrap_err();
    assert!(err.contains("notes.wav"), "{err}");
    // Same WAV, edits, history and effect instance.
    assert_eq!(state.get(1).unwrap().metadata.name, "sine1.wav");
    assert_eq!(state.get_chain(1).unwrap(), chain);
    assert_eq!(state.edit_state(1).unwrap(), st);
    assert_eq!(gain_of(&state, 1, slot), plus_6db());

    // The tab's own WAV is refused too.
    let own = dir.path().join("sine1.wav");
    let err = state
        .replace_audio(1, &own, &fx.plugins, |_, _| {})
        .unwrap_err();
    assert!(err.contains("already this tab's audio"), "{err}");
    assert_eq!(state.edit_state(1).unwrap(), st);
}
