//! Integration tests for loading files and the command-level response shapes.

use hither_dither_lib::commands::{
    PEAKS_HEADER_BYTES, PEAKS_KIND_MINMAX, PEAKS_KIND_SAMPLES, encode_peaks,
};
use hither_dither_lib::state::{AppState, ClipSettings, EditOp, FadeSetting, OpenError, Preview};
use hither_dither_lib::stats::Loudness;
use hither_dither_lib::wav::synth::{Encoding, WavBuilder, interleave, sine};
use hither_dither_lib::wav::write_wav;

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

#[test]
fn save_before_the_analysis_finishes_is_exact() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_float_sine(&state, &dir, 1, 60_000);
    state
        .apply_edit(1, EditOp::FadeOut, 0, 60_000, -0.5)
        .unwrap();
    open_rendered(&state, &dir, 1, 2);
    let copy = dir.path().join("saved.wav");
    let mut last = (0, 0);
    state
        .save(1, Some(&copy), |done, total| last = (done, total))
        .unwrap();
    // Analysing and writing each take one pass.
    assert_eq!(last, (120_000, 120_000));
    assert_eq!(whole_stats(&state, 1), whole_stats(&state, 2));
    assert!(!state.clip_runs(1).unwrap().pending);
    assert_eq!(
        encode_peaks(&state.get(1).unwrap(), 0, 60_000, 300),
        encode_peaks(&state.get(2).unwrap(), 0, 60_000, 300)
    );
    // Saving an analysed state only writes.
    state.apply_edit(1, EditOp::Silence, 0, 10, 0.0).unwrap();
    let rev = state.edit_state(1).unwrap().revision;
    assert!(state.wait_analysis(1, rev));
    state
        .save(1, Some(&copy), |done, total| last = (done, total))
        .unwrap();
    assert_eq!(last, (60_000, 60_000));
}

fn frames_on_disk(path: &std::path::Path) -> u64 {
    hither_dither_lib::wav::parse_header(&std::fs::read(path).unwrap())
        .unwrap()
        .frames
}

#[test]
fn save_as_leaves_the_source_alone() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let source = dir.path().join("sine1.wav");
    let original = std::fs::read(&source).unwrap();
    state
        .apply_edit(1, EditOp::Delete, 1_000, 3_000, 0.0)
        .unwrap();
    let edited_stats = analysed_stats(&state, 1);

    let copy = dir.path().join("trimmed.wav");
    let out = state.save(1, Some(&copy), |_, _| {}).unwrap();
    assert_eq!(std::fs::read(&source).unwrap(), original);
    assert_eq!(frames_on_disk(&copy), 6_000);
    assert_eq!(out.metadata.name, "trimmed.wav");
    assert_eq!(out.metadata.frames, 6_000);
    assert!(!out.edit_state.dirty && !out.edit_state.can_undo && !out.edit_state.can_redo);
    assert_eq!(state.get(1).unwrap().metadata.path, copy);
    // Same audio, so the same stats, now served from the saved file.
    assert_eq!(whole_stats(&state, 1), edited_stats);
    assert!(state.compute_stats(1, Some((0, 100))).unwrap().is_some());
}

#[test]
fn save_replaces_the_original() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let path = dir.path().join("sine1.wav");
    state
        .apply_edit(1, EditOp::Crop, 2_000, 7_500, 0.0)
        .unwrap();
    state
        .apply_edit(1, EditOp::FadeOut, 5_000, 5_500, 0.0)
        .unwrap();
    let out = state.save(1, None, |_, _| {}).unwrap();
    assert_eq!(out.metadata.frames, 5_500);
    assert_eq!(frames_on_disk(&path), 5_500);
    assert!(!out.edit_state.dirty);
    // The tab keeps working from the replaced file, and further edits can be saved too.
    let file = state.get(1).unwrap();
    assert!(!file.changed_on_disk());
    let tile = encode_peaks(&file, 0, 5_500, 100);
    assert_eq!(header(&tile).2, 100);
    drop(file);
    state.apply_edit(1, EditOp::Delete, 0, 500, 0.0).unwrap();
    state.save(1, None, |_, _| {}).unwrap();
    assert_eq!(frames_on_disk(&path), 5_000);
    // No temporary files are left behind.
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, ["sine1.wav"]);
}

#[test]
fn save_refuses_when_the_original_changed() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    state.apply_edit(1, EditOp::Silence, 0, 100, 0.0).unwrap();
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(dir.path().join("sine1.wav"))
        .unwrap()
        .write_all(&[0; 10])
        .unwrap();
    let err = state.save(1, None, |_, _| {}).unwrap_err();
    assert!(
        err.contains("changed on disk") && err.contains("Save As"),
        "{err}"
    );
    assert!(state.edit_state(1).unwrap().dirty);
}

#[test]
fn failed_save_keeps_the_edits() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    let original = std::fs::read(dir.path().join("sine1.wav")).unwrap();
    state.apply_edit(1, EditOp::Crop, 0, 1_000, 0.0).unwrap();
    let err = state
        .save(
            1,
            Some(&dir.path().join("missing-dir").join("x.wav")),
            |_, _| {},
        )
        .unwrap_err();
    assert!(err.starts_with("Could not save “x.wav”"), "{err}");
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
fn saving_bakes_the_clip_settings() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::default();
    open_sine(&state, &dir, 1, 8_000);
    state
        .set_clip(1, clip((500, 7_500), (300, 0.0), (300, -0.5)))
        .unwrap();
    let shown = analysed_stats(&state, 1);
    let copy = dir.path().join("shaped.wav");
    let out = state.save(1, Some(&copy), |_, _| {}).unwrap();
    assert_eq!(frames_on_disk(&copy), 7_000);
    assert_eq!(out.clip.untrimmed_frames, 7_000);
    assert_eq!((out.clip.trim_start, out.clip.trim_end), (0, 7_000));
    assert_eq!(out.clip.fade_in.len + out.clip.fade_out.len, 0);
    assert!(out.clip.joins.is_empty());
    assert_eq!(whole_stats(&state, 1), shown);
    assert_eq!(state.clip(1).unwrap(), out.clip);
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
