# Acceptance checklist

Manual acceptance pass over every scenario in the four v1 specs (`openspec/changes/add-wav-analyser-app/specs/`).

**Setup:** generate fixtures with `cd src-tauri && cargo run --release --example gen_fixtures -- ../fixtures --large`, then run `npm run tauri dev`. Fixture names below refer to files in `fixtures/`.

**Result key:** ✅ pass · ❌ fail · ⏳ not yet checked. "Auto" means an automated test also covers the scenario (named in the Evidence column).

Environment for this pass: _fill in (OS, webview, audio device)_.

## wav-import

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| I1 | Open via dialog | Ctrl/Cmd+O (or File › Open…), pick `cd-16bit-stereo-10s.wav` | Waveform, metadata and stats appear | | ⏳ |
| I2 | Cancel dialog | With I1 loaded, Ctrl/Cmd+O, then Cancel | Same file still loaded, nothing changes | | ⏳ |
| I3 | Drop one file | Drag `pcm24-stereo-48k.wav` onto the window | Replaces the loaded file | | ⏳ |
| I4 | Drop several files | Drag `u8-mono.wav` and `float64-mono.wav` together | First file loads; notice "Only one file can be opened at a time" | | ⏳ |
| I5 | 24-bit stereo | Open `pcm24-stereo-48k.wav` | 2 channels, 24-bit, 48,000 Hz | Auto: `wav::source::decodes_24_bit_exactly`, `parser::stereo_24_bit_48k` | ⏳ |
| I6 | Extensible float 6-ch | Open `float32-extensible-5.1.wav` | 6 lanes (L R C LFE Ls Rs), IEEE float | Auto: `parser::extensible_float_six_channels` | ⏳ |
| I7 | Metadata chunks | Open `with-list-bext-chunks.wav` | Loads normally | Auto: `parser::skips_metadata_and_odd_chunks` | ⏳ |
| I8 | Truncated file | Open `truncated.wav` | Loads available audio; "appears truncated" warning | Auto: `tests/state.rs::truncated_file_loads_with_flag` | ⏳ |
| I9 | Non-WAV file | With a file loaded, open `not-a-wav.wav` | Error names the file and "not a valid WAV file"; previous file stays | Auto: `tests/state.rs::failed_open_keeps_previous_file` | ⏳ |
| I10 | Unsupported encoding | Open `ima-adpcm.wav` (also `mu-law.wav`) | Error names "IMA ADPCM" (or "µ-law") | Auto: `parser::rejects_unsupported_encodings_by_name` | ⏳ |
| I11 | Large file load | Open `large-1gb.wav`, then move the window and open a menu while it loads | Progress bar appears (load > 500 ms); window stays interactive | | ⏳ |

## waveform-view

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| W1 | Stereo display | Open `cd-16bit-stereo-10s.wav` | Two lanes labelled L and R, ±1.0 scale | | ⏳ |
| W2 | Transient visible zoomed out | Open `single-spike-10min.wav`, press F | A full-height spike at ~6:17 | Auto: `peaks::spike_in_silence_visible_at_every_zoom` | ⏳ |
| W3 | Ruler adapts to zoom | Zoom in until samples show as dots | Ruler shows sample indices | Auto: `ruler.test.ts` | ⏳ |
| W4 | Zoom around pointer | Ctrl/Cmd+wheel up over a feature | Zooms in; the feature stays under the pointer | Auto: `viewport.test.ts` | ⏳ |
| W5 | Zoom to fit | Zoom in, then press F (or the Fit button, or View › Zoom to Fit) | Whole file visible | Auto: `viewport.test.ts` | ⏳ |
| W5b | Other zoom/scroll inputs | Try +/− keys, wheel, Shift+wheel, scrollbar drag, overview drag | Each zooms or scrolls as labelled | | ⏳ |
| W6 | Create selection | On a 10 s file at fit zoom, drag from 1.0 s to 2.5 s | Highlight on all lanes; toolbar shows Start 1.000 s, End 2.500 s, Duration 1.500 s (± a pixel's worth) | Auto: `selection.test.ts` | ⏳ |
| W7 | Clear selection | Press Escape (also try a plain click) | Selection removed | | ⏳ |
| W8 | Scrolling a long file | Open `one-hour-stereo-48k.wav`, scroll continuously, then press Ctrl/Cmd+Shift+B | Smooth scrolling; benchmark reports ≥ 30 fps (record in `docs/performance.md`) | | ⏳ |

## audio-stats

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| S1 | Metadata after load | Open `cd-16bit-stereo-10s.wav` | PCM, 16-bit, 44,100 Hz, 2 channels, 441,000 frames, 0:00:10.000 | Auto: `tests/state.rs::metadata_for_cd_quality_file`, `format.test.ts` | ⏳ |
| S2 | Full-scale sine | Open `full-scale-sine-and-silence.wav` | L: peak 0.0 dBFS, RMS −3.0 dBFS, DC 0.00%, crest 3.0 dB | Auto: `stats::full_scale_sine_and_silent_channel` | ⏳ |
| S3 | Silent channel | Same file | R: peak and RMS "-inf dBFS" | Auto: same test | ⏳ |
| S4 | Reference tone | Open `reference-minus18dbfs-stereo.wav` | Integrated loudness −18.0 LUFS | Auto: `stats::reference_tone_loudness` | ⏳ |
| S5 | Too short to measure | On S4's file, select about 200 ms | Loudness "n/a" with tooltip | Auto: `stats::loudness_unavailable_for_short_range_and_odd_layouts` | ⏳ |
| S5b | Unsupported layout | Open `quad-loudness-na.wav` | Loudness "n/a", tooltip explains layouts | Auto: same test | ⏳ |
| S6 | Clipped audio | Open `two-clip-runs.wav` | 2 clipped runs; two red markers (at 1 s and 3 s) | Auto: `stats::counts_clip_runs_not_isolated_samples` | ⏳ |
| S7 | Isolated full-scale sample | Same file (sample at ~2.27 s) | Not counted, not marked | Auto: same test | ⏳ |
| S8 | Stats follow selection | Open `loud-with-silent-gap-4s-6s.wav`, select inside 4–6 s | Label "Selection"; peaks "-inf dBFS" | Auto: `tests/state.rs::stats_and_clip_runs_commands` | ⏳ |
| S9 | Clear restores whole file | Press Escape | Label "Whole file"; whole-file values return | | ⏳ |

## audio-playback

These need an audio output device. On WSL, install `libasound2-plugins` so ALSA reaches the WSLg PulseAudio server, or test on the host OS.

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| P1 | Play and pause | Open `beep-every-second-mono-60s.wav`, Space, wait, Space | Plays from the playhead; pauses and the playhead stays | | ⏳ |
| P2 | Stop | Play, then click Stop | Playback halts; playhead returns to where it began | | ⏳ |
| P3 | Seek during playback | While playing, click at 30 s | Continues from 30.0 s | | ⏳ |
| P4 | Play selection | Select 1.0–2.0 s, Space | Plays 1.0 s to 2.0 s, then stops | Auto: `playback::engine::selection_playback_stops_at_range_end` | ⏳ |
| P5 | Loop selection | Loop on, select, Space | Repeats until paused or stopped | Auto: `playback::engine::loop_wraps_until_stopped` | ⏳ |
| P6 | Playhead follows playback | Zoom in, play past the right edge | View scrolls to keep the playhead visible; beeps line up with the playhead (≤ 50 ms) | | ⏳ |
| P7 | Sample-rate mismatch | Play `tone-1khz-96k.wav` on a 48 kHz device | Correct pitch (1 kHz) and speed (10 s) | Auto: `playback::engine::resampler_keeps_length_and_pitch` | ⏳ |
| P8 | No audio device | Run where no output device exists (e.g. WSL without the ALSA PulseAudio plugin) | Playback controls disabled with a message; waveform and stats work | | ⏳ |
| P9 | Lower volume | Set volume to 50% while playing | Quieter; stats panel unchanged | Auto: `playback::engine::volume_scales_output_only` | ⏳ |
