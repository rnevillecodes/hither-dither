# Acceptance checklist

Manual acceptance pass over every scenario in the four v1 specs (`openspec/changes/add-wav-analyser-app/specs/`), plus the multi-file tabs and app icon change (`openspec/changes/add-multi-file-tabs/specs/`; sections file-tabs, audio-stats: details panel, app-branding, and rows I2b–I13) the dark theme change (section app-theme), and the session restore, tab reordering and window change (`openspec/changes/tab-session-and-window-fit/specs/`; sections app-window, tab-reorder and session-restore), and the audio editing change (`openspec/changes/add-audio-editing/specs/`; sections transport-controls, audio-editing and wav-export), and the clip fades change (`openspec/changes/add-clip-fades-and-crossfades/specs/`; section clip-fades, plus rows E4b, E8b, E8c and X8b), and the live editing change (`openspec/changes/add-live-editing/specs/`; section live-editing, plus rows E9, E10b, F14, F14b, F19, X13b and X13c, and E11 removed).

**Setup:** generate fixtures with `cd src-tauri && cargo run --release --example gen_fixtures -- ../fixtures --large`, then run `npm run tauri dev`. Fixture names below refer to files in `fixtures/`.

**Result key:** ✅ pass · ❌ fail · ⏳ not yet checked. "Auto" means an automated test also covers the scenario (named in the Evidence column).

Environment for this pass: _fill in (OS, webview, audio device)_.

## wav-import

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| I1 | Open via dialog | Ctrl/Cmd+O (or File › Open…), pick `cd-16bit-stereo-10s.wav` | Opens in a new active tab; waveform, metadata and stats appear | | ⏳ |
| I2 | Cancel dialog | With I1 loaded, Ctrl/Cmd+O, then Cancel | Same tabs open, same tab active, nothing changes | | ⏳ |
| I2b | Open several from dialog | Ctrl/Cmd+O, select `u8-mono.wav`, `float64-mono.wav` and `pcm24-stereo-48k.wav` | Each opens in its own tab; tabs already open stay open | | ⏳ |
| I3 | Drop one file | With a file open, drag `pcm24-stereo-48k.wav` onto the window | Opens in a new active tab; the previous file stays open in its tab | | ⏳ |
| I4 | Drop several files | Drag `u8-mono.wav` and `float64-mono.wav` together | Both open, one tab each; no "one file at a time" notice | | ⏳ |
| I5 | 24-bit stereo | Open `pcm24-stereo-48k.wav` | 2 channels, 24-bit, 48,000 Hz | Auto: `wav::source::decodes_24_bit_exactly`, `parser::stereo_24_bit_48k` | ⏳ |
| I6 | Extensible float 6-ch | Open `float32-extensible-5.1.wav` | 6 lanes (L R C LFE Ls Rs), IEEE float | Auto: `parser::extensible_float_six_channels` | ⏳ |
| I7 | Metadata chunks | Open `with-list-bext-chunks.wav` | Loads normally | Auto: `parser::skips_metadata_and_odd_chunks` | ⏳ |
| I8 | Truncated file | Open `truncated.wav` | Loads available audio; "appears truncated" warning | Auto: `tests/state.rs::truncated_file_loads_with_flag` | ⏳ |
| I9 | Non-WAV file | With a file loaded, open `not-a-wav.wav` | Error names the file and "not a valid WAV file"; no tab is left for it; the open tabs stay | Auto: `tests/state.rs::failed_open_keeps_previous_file` | ⏳ |
| I9b | One bad file among several | Drop `not-a-wav.wav` together with `u8-mono.wav` and `float64-mono.wav` | One error naming `not-a-wav.wav`; the two valid files each open in a tab | | ⏳ |
| I10 | Unsupported encoding | Open `ima-adpcm.wav` (also `mu-law.wav`) | Error names "IMA ADPCM" (or "µ-law") | Auto: `parser::rejects_unsupported_encodings_by_name` | ⏳ |
| I11 | Large file load | Open `large-1gb.wav`, then move the window and open a menu while it loads | Progress bar appears (load > 500 ms); window stays interactive | | ⏳ |
| I12 | No toolbar Open button, nothing open | Start the app | No toolbar is shown at all; the empty screen's Open… button opens the dialog | | ⏳ |
| I13 | No toolbar Open button, files open | Open any file | Toolbar still has no Open… button; File › Open… works | | ⏳ |

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

## file-tabs

Below, "A" is `cd-16bit-stereo-10s.wav` and "B" is `beep-every-second-mono-60s.wav`.

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| T1 | Two files open | Open A, then B | Tabs "A", "B" in that order; B active (highlighted) and shown in the panels; hovering a tab shows its full path | | ⏳ |
| T2 | No files open | Start the app (or close every tab) | No tab strip; the empty "Open a WAV file" screen | | ⏳ |
| T3 | Open while another file is shown | With A active, open B | New tab B becomes active; A stays open | | ⏳ |
| T4 | Open the same file twice | With A and B open and B active, open A again (dialog or drop) | A's tab becomes active; still exactly two tabs | Auto: `tabOrder.test.ts` (path matching) | ⏳ |
| T5 | Switch tabs during a long load | With A open, open `large-1gb.wav`, then click A's tab while it loads | Loading tab shows a spinner; A's waveform is usable; the load finishes and its tab becomes active | | ⏳ |
| T6 | Click a tab | With A and B open and B active, click A | A becomes active with its own waveform, metadata and stats | | ⏳ |
| T7 | Cycle with the keyboard | Open three files, activate the last, press Ctrl+Tab (then Ctrl+Shift+Tab) | First tab becomes active (then back to the last) | Auto: `tabOrder.test.ts` | ⏳ |
| T8 | Selection kept across switches | In A select 1.0–2.0 s and zoom in; switch to B, then back to A | Same zoom and scroll, the 1.0–2.0 s selection, stats labelled "Selection" | | ⏳ |
| T9 | Selections are independent | With a selection in A, switch to B (no selection) | B shows no selection and "Whole file" stats | | ⏳ |
| T10 | Close with the keyboard | With A and B open and B active, press Ctrl/Cmd+W once | Only B closes; A remains | | ⏳ |
| T11 | Close a loading tab | Open `large-1gb.wav`, click its tab's × while it loads | The load stops, the tab goes away, no error appears | Auto: `tests/state.rs::cancel_open_stops_only_that_load` | ⏳ |
| T12 | Close the middle tab | Open three files, activate the middle one, close it | The tab to its right becomes active | Auto: `tabOrder.test.ts` | ⏳ |
| T13 | Close the last tab | With one tab open, close it (also try middle-click and File › Close Tab) | Empty screen with its Open button; File › Close Tab is greyed out | | ⏳ |
| T14 | Many tabs | Open about 15 files | The strip scrolls sideways; the active tab stays visible | | ⏳ |
| T15 | Switch tabs during playback | Play B to about 12 s, switch to A, then back to B | Playback stops on switching; B's playhead is at about 12 s | | ⏳ |
| T16 | Play in another tab | Switch to A and press Space | A plays from its own playhead | | ⏳ |
| T17 | Close the playing tab | Play B, then close its tab | Playback stops; no error | | ⏳ |

## audio-stats: details panel

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| D1 | No files open | Start the app (or close every tab) | No details panel and no panel toggle; the empty screen spans the full width | | ⏳ |
| D2 | Hide the panel | Open a file, click the panel toggle at the right of the toolbar | Panel hides; the waveform widens to the full window | | ⏳ |
| D3 | Choice is remembered | Hide the panel, switch tabs, quit, restart and open a file | Panel stays hidden until toggled back on | | ⏳ |
| D4 | Narrow window | Make the window narrower than 820 px, toggle the panel | The bottom panel hides and shows | | ⏳ |

## app-branding

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| B1 | Running app on Windows | `npm run tauri dev` | Title bar and taskbar show the HitherDither icon (waveform tile), not the Tauri logo | | ⏳ |
| B2 | Installed bundle | `npm run tauri build`, install the result | The installed shortcut/launcher shows the HitherDither icon | | ⏳ |
| B3 | Small size | View `src-tauri/icons/32x32.png` at 100% | The waveform bars are recognisable | Signed off from the preview sheet on 2026-10-10 | ✅ |
| B4 | Dark and light backgrounds | View the icon on white and on black | The tile is clearly separated from both | Signed off from the preview sheet on 2026-10-10 | ✅ |
| B5 | Empty-screen watermark | Start the app with no files open (the OS theme doesn't matter; the app is always dark) | A large faint whitish imprint of the icon's waveform (bars and dots, no tile) sits above "Open a WAV file"; the Open… button still works | | ⏳ |
| B6 | Watermark hidden with files | Open any file | The imprint is gone | | ⏳ |

## app-theme

From `openspec/changes/teal-slate-dark-theme/specs/app-theme/`. Run with the OS in light mode, then repeat C1–C3 in dark mode.

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| C1 | OS set to light mode | Set the OS to light mode and start the app | Slate-grey surfaces with whitish text | | ⏳ |
| C2 | OS theme changes while running | Open a file, then switch the OS between dark and light | Nothing in the app changes colour, including the waveform | | ⏳ |
| C3 | Panels stand out | Open a file with the details panel shown | Panel, toolbar and active tab are lighter than the window background; borders visible | | ⏳ |
| C4 | Reading the stats panel | Open `cd-16bit-stereo-10s.wav` | Values in whitish text, labels dimmed; both easy to read | Auto: `palette.test.ts` | ⏳ |
| C5 | Active tab and toggles | Open two files with the details panel shown | Active tab marker and pressed panel toggle are teal; no blue anywhere (check the volume slider and progress bar too) | | ⏳ |
| C6 | Selection | Drag a selection across the waveform | Translucent teal band with teal edges; the waveform stays visible under it | | ⏳ |
| C7 | Playhead over a selection | Open `beep-every-second-mono-60s.wav`, select 1–5 s, press Space | Amber playhead clearly visible over the waveform and the teal band | Auto: `palette.test.ts` | ⏳ |
| C8 | Clipped audio | Open `two-clip-runs.wav` | Clip markers in red, distinct from teal and amber | Auto: `palette.test.ts` | ⏳ |
| C9 | Tabbing through the toolbar | Press Tab repeatedly, then click a button with the mouse | A teal ring moves across toolbar buttons, tabs and the volume slider; the mouse click shows no ring | | ⏳ |
| C10 | Windows title bar | On Windows with the OS in light mode, run `npm run tauri dev` | The title bar is dark | | ⏳ |
| C11 | Shadows | Trigger a notice (open `not-a-wav.wav`) and a long load (`large-1gb.wav`) | The notice and the progress box are visibly lifted off the background | | ⏳ |

## app-window

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| WN1 | Laptop screen | Start the app on a 1366×768 (or similar small) display | The window opens maximized above the taskbar; title bar and menu visible; nothing cut off at the bottom | | ⏳ |
| WN2 | Restore down | Click the window's restore button | A normal, resizable window centred on screen, no smaller than 640×420 | | ⏳ |
| WN3 | App starts with nothing open | Start the app with no remembered session | No toolbar strip above the empty screen; the empty screen fills the window below the menu | | ⏳ |
| WN4 | Toolbar returns with a file | From the empty screen, open A | Toolbar and tab strip appear; once A loads the toolbar shows transport, zoom and selection controls | | ⏳ |
| WN5 | Last tab closed | With only A open, close its tab | Toolbar and tab strip disappear; the empty screen fills the window | | ⏳ |

## tab-reorder

Open three files first; below they are "a", "b" and "c" in that order.

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| R1 | Move the last tab to the front | Drag tab c to the left of a and release | Order is c, a, b | Auto: `tabOrder.test.ts` (move) | ⏳ |
| R2 | Drop indicator while dragging | Drag a over the gap between b and c, don't release | A teal bar marks the gap between b and c; the dragged tab is dimmed | | ⏳ |
| R3 | Reorder during playback | Play a, then drag a's tab to a new position | Playback continues uninterrupted; a stays active | | ⏳ |
| R4 | Reorder an inactive tab | With a selection and zoom in a, activate b, drag a past b | Order changes; b stays active; switching to a shows the same selection and zoom | | ⏳ |
| R5 | Small jitter is a click | Press on an inactive tab, move 1–2 px, release | That tab becomes active; order unchanged | | ⏳ |
| R5b | Close still works | Click a tab's ×, and middle-click another tab | Both close; neither starts a drag | | ⏳ |
| R6 | Escape cancels | Start dragging a tab, press Escape, then release | Order unchanged; the waveform selection (if any) is kept | | ⏳ |
| R6b | Release away from the strip | Drag a tab well down into the waveform and release | Drop bar disappears while away; order unchanged | | ⏳ |
| R7 | Cycle after reordering | Drag c to the front (c, a, b), activate c, press Ctrl+Tab | a becomes active | Auto: `tabOrder.test.ts` | ⏳ |
| R8 | Close after reordering | With c, a, b and a active, close a | b becomes active | Auto: `tabOrder.test.ts` | ⏳ |
| R9 | Many tabs | With about 15 tabs open, drag a tab towards either end of the strip | The strip scrolls so off-screen positions can be reached | | ⏳ |

## session-restore

Below, "A" is `cd-16bit-stereo-10s.wav` and "B" is `beep-every-second-mono-60s.wav`. To start from no session, clear the app's webview data (or close every tab before quitting).

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| S1 | Session updated on open | Open A then B, quit, relaunch | Tabs A, B restored in that order; B active | Auto: `session.test.ts` (round-trip) | ⏳ |
| S2 | Session updated on close | Open A and B, close B, end the process from Task Manager, relaunch | Only A is restored | | ⏳ |
| S3 | Failed load not remembered | Open `not-a-wav.wav` (error shown) together with A, quit, relaunch | Only A is restored; no error on launch | | ⏳ |
| S4 | Restore two files | Open A and B, activate A, quit, relaunch | A and B restored in order; after loading A is active | | ⏳ |
| S5 | Nothing to restore | Close every tab, quit, relaunch | Empty "Open a WAV file" screen | Auto: `session.test.ts` (missing/malformed) | ⏳ |
| S6 | One file deleted between runs | Open copies of A and B, quit, rename the A copy, relaunch; then quit and relaunch again | One "Could not open file" notice for A; B opens and is active; the second relaunch restores only B with no notice | | ⏳ |
| S7 | Close a restoring tab | Open `large-1gb.wav`, quit, relaunch and close its tab while it loads; quit and relaunch again | Load stops with no error; the second relaunch doesn't restore it | | ⏳ |
| S8 | Selection not restored | Select 1.0–2.0 s in A and zoom in, quit, relaunch | A reopens zoomed to fit, no selection, "Whole file" stats | | ⏳ |
| S9 | Reordered tabs | Drag B before A, quit, relaunch | Restored in the order B, A | | ⏳ |

## transport-controls

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| G1 | While stopped | Open `beep-every-second-mono-60s.wav`, click at 12 s on the ruler, click Go to start | Playhead at 0; time shows 0:00:00.000 | | ⏳ |
| G2 | While playing | Play from 30 s, click Go to start | Playback continues from 0 without stopping | | ⏳ |
| G3 | Zoomed in elsewhere | Open `single-spike-10min.wav`, zoom in around 5:00, click Go to start (also try Home) | View scrolls to the start at the same zoom; playhead visible at 0 | | ⏳ |
| G4 | Accessible name and tooltip | Hover and Tab to the button | Tooltip "Go to start (Home)"; announced as "Go to start"; still enabled with no audio device | | ⏳ |

## audio-editing

Below, "A" is `cd-16bit-stereo-10s.wav`. Shortcuts: Delete, Ctrl/Cmd+T (Crop), Ctrl/Cmd+L (Silence), Ctrl/Cmd+Z (Undo), Ctrl/Cmd+Shift+Z or Ctrl+Y (Redo); Fade In and Fade Out are in the Edit menu only.

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| E1 | No selection | Open A without selecting | Crop, Delete, Silence, Fade In and Fade Out are greyed out; their shortcuts do nothing | Auto: `editRules.test.ts` (canApply) | ⏳ |
| E2 | Nothing open | Close every tab, open the Edit menu | Every item is greyed out | | ⏳ |
| E3 | Trim leading and trailing silence | Select 2.0–7.5 s in A, Edit › Crop to Selection | 5.5 s long, starts with the audio that was at 2.0 s, no selection, zoomed to fit, playhead at 0 | Auto: `edit::tests::crop_keeps_the_range`, `editRules.test.ts` | ⏳ |
| E4 | Remove a section | Select 4.0–5.0 s, press Delete | 9.0 s long; audio from 5.0 s now at 4.0 s; playhead at 4.0 s | Auto: `edit::tests::delete_joins_either_side` | ⏳ |
| E4b | Join is crossfaded | After E4, look at 4.0 s zoomed in | A crossfade marker at the join; the audio within 5 ms either side blends instead of switching abruptly | Auto: `edit::audio::tests::crossfade_blends_both_sides_at_the_join`, `tests/state.rs::delete_creates_a_join_whose_crossfade_is_clamped` | ⏳ |
| E5 | Whole file selected | Drag across the whole file, open the Edit menu, press Delete | Delete and Crop greyed out; nothing changes | Auto: `editRules.test.ts` | ⏳ |
| E6 | Silence a click | Select a few ms, Edit › Silence | Flat waveform there, same length, selection stats show −∞ peak | Auto: `tests/state.rs::silence_shows_in_selection_stats` | ⏳ |
| E7 | Fade out the ending | Select the last 2 s, Edit › Fade Out…, Apply without moving the point | Waveform tapers linearly to silence; audio before is unchanged | Auto: `edit::tests::linear_fades` | ⏳ |
| E8 | Fade twice | Apply the same linear Fade Out to a range twice | Visibly steeper (squared) taper | Auto: `edit::tests::fading_twice_squares_the_gain` | ⏳ |
| E8b | Curved fade in | Edit › Fade In…, drag the point fully up, Apply | Gain rises quickly then levels off, matching the dialog's curve | Auto: `edit::tests::curved_fades_follow_the_curve`, `curve.test.ts` | ⏳ |
| E8c | Cancel the dialog | Edit › Fade Out…, then Cancel (also Escape) | Audio and undo history unchanged | | ⏳ |
| E9 | Clipping removed | Open `two-clip-runs.wav`, select a clipped run, Silence | Its clip markers disappear at once; once the stats stop updating, the whole-file clip count has dropped | Auto: `edit::audio::tests::full_scale_survives_only_unchanged`, `stats::clipping::tests::runs_in_changed_audio_vanish` | ⏳ |
| E10 | Playback of an edited file | Delete 4.0–5.0 s, play from 3.5 s | Goes straight from the 4.0 s audio to the 5.0 s audio, no gap | Auto: `edit::audio::tests::reads_every_kind_of_edit` | ⏳ |
| E10b | No wait after an edit | Open `large-1gb.wav`, crop it, switch to another tab and back | Cropped waveform at once, no progress box; Edit menu available straight away; whole-file stats marked "Updating…" until done; other tabs usable | Auto: `tests/state.rs::edits_apply_back_to_back_without_analysis` | ⏳ |
| E12 | Undo a delete | Delete 4.0–5.0 s, Ctrl/Cmd+Z | 10.0 s again with the original audio; Redo enabled | Auto: `tests/state.rs::edit_then_undo_restores_frames_and_stats` | ⏳ |
| E13 | Several undos | Crop, Silence, Fade In, then Undo ×3 | Identical to the file as opened; Undo greyed out | Auto: `tests/state.rs::edit_then_undo_restores_frames_and_stats` | ⏳ |
| E14 | New edit clears redo | Undo an edit, then apply a different one | Redo greyed out | Auto: `tests/state.rs::new_edit_after_undo_clears_redo` | ⏳ |
| E15 | History is per tab | Edit A, switch to B, Ctrl/Cmd+Z | Nothing changes in either file | Auto: `tests/state.rs::edits_on_one_file_leave_others_alone` | ⏳ |
| E16 | Undo a fade | Fade In 1.0–2.0 s, Undo | Fade gone; 1.0–2.0 s still selected | Auto: `editRules.test.ts` | ⏳ |
| E17 | Close without saving | Crop A, close the tab, choose Don't Save, reopen A | Original length and content | | ⏳ |

## wav-export

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| X1 | Edit marks the tab | Silence a selection in A | A dot replaces the tab's × (× on hover); tooltip says "Unsaved changes" | | ⏳ |
| X2 | Undo back to the original | One edit, then Undo | The dot disappears | Auto: `tests/state.rs::edit_then_undo_restores_frames_and_stats` | ⏳ |
| X3 | First save asks | Crop a copy of A, Ctrl/Cmd+S; try Cancel, then Overwrite | Cancel leaves file and edits; Overwrite saves | | ⏳ |
| X4 | Later saves don't ask | After X3, edit again and Ctrl/Cmd+S | Saved without a dialog | | ⏳ |
| X5 | Save a trimmed copy | Crop `take1.wav`, Save As `take1-trim.wav` | New file has the cropped audio; original unchanged; tab now "take1-trim.wav" with no dot; relaunch restores the new path | Auto: `tests/state.rs::save_as_leaves_the_source_alone` | ⏳ |
| X6 | Cancel the dialog | Save As, then Cancel | Nothing written; edits and dot kept | | ⏳ |
| X7 | Name of another open file | With B open in another tab, Save As over B's path | Nothing written; notice says it's open in another tab | | ⏳ |
| X8 | Untouched audio is bit-exact | Delete 1.0–2.0 s from `pcm24-stereo-48k.wav`, Save As | Samples outside the join's crossfade identical to the source | Auto: `wav::writer::tests::untouched_samples_are_bit_exact_outside_the_crossfade` | ⏳ |
| X8b | Hard join is bit-exact | As X8 but drag the join's crossfade to zero before saving | Every sample identical to the source | Auto: `wav::writer::tests::hard_join_is_bit_exact` | ⏳ |
| X9 | Metadata is not carried over | Edit `with-list-bext-chunks.wav`, Save As, reopen | Only `fmt ` and `data` chunks; same format details | Auto: `wav::writer::tests::metadata_chunks_are_not_written`, `edited_file_keeps_the_format` | ⏳ |
| X10 | Result too large for WAV | (Not practical by hand) | Save fails with "too large" and writes nothing | Auto: `wav::writer::tests::refuses_results_over_4_gib` | ⏳ |
| X11 | Read-only target | Save As into a read-only folder | Error notice; original intact; edits kept | Auto: `tests/state.rs::failed_save_keeps_the_edits` | ⏳ |
| X12 | File changed by another program | Open a copy of A, edit it in another program and save there, then edit and Save in HitherDither | Nothing written; notice says it changed on disk and suggests Save As | Auto: `tests/state.rs::save_refuses_when_the_original_changed` | ⏳ |
| X13 | Keep working after save | Zoom in, select 1.0–2.0 s, Fade In, Save | Zoom and selection unchanged; Undo greyed out | Auto: `tests/state.rs::save_replaces_the_original` | ⏳ |
| X13b | Save before the analysis finishes | Open `large-1gb.wav`, apply Fade Out to most of it, Save As straight away while the stats say "Updating…" | Progress box after a moment; afterwards the stats are current and not marked as updating | Auto: `tests/state.rs::save_before_the_analysis_finishes_is_exact` | ⏳ |
| X13c | Progress while rendering | Edit `large-1gb.wav`, Save As | The delayed progress box shows how far the save has got; other tabs stay usable | Auto: `tests/state.rs::save_before_the_analysis_finishes_is_exact` (progress reaches the total) | ⏳ |
| X14 | Cancel closing | Edit A, Ctrl/Cmd+W, Cancel (also try ×, middle-click, File › Close Tab) | Tab stays open with its edits | | ⏳ |
| X15 | Save on close | Edit A, close its tab, choose Save | A written with the edits (no overwrite prompt); tab closes | | ⏳ |
| X16 | Two edited files | Edit A and B, close the window (also File › Quit) | Asked about A, then B; quits after both answered | | ⏳ |
| X17 | Cancel quitting | Quit with edits, Cancel at the first prompt | App keeps running with every tab and edit; quitting with no edits doesn't prompt | | ⏳ |

## clip-fades

Below, "A" is `cd-16bit-stereo-10s.wav`. Handles: trim brackets at the audio's edges, small squares at the top for fade and crossfade lengths, dots on fade lines for curves.

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| F1 | Linear by default | Drag A's start fade square to 1 s without touching its dot | Straight fade line; gain rises linearly | Auto: `curve.test.ts`, `edit::curve::tests` | ⏳ |
| F2 | Bend upward | Drag a fade-out's dot to the top | Line bows upward; audio half way through is louder than linear | Auto: `clipHandles.test.ts` (bend) | ⏳ |
| F3 | Reset the bend | Double-click a bent fade's dot | Curve returns to linear (one undo step) | Auto: `clipHandles.test.ts` (resetBend) | ⏳ |
| F4 | Trim the start | Drag A's start bracket to 1.5 s and release | 8.5 s long; starts with the audio that was at 1.5 s | Auto: `tests/state.rs::trim_changes_the_length_and_can_be_restored` | ⏳ |
| F5 | Restore trimmed audio | Trim 1.5 s, delete a selection, drag the start bracket back to the left | Trimmed 1.5 s returns; the deletion is unaffected | Auto: `edit::tests::trimming_and_restoring` | ⏳ |
| F6 | Can't trim to nothing | Drag the start bracket past the end | Stops one frame before the end | Auto: `clipHandles.test.ts`, `edit::tests::trim_keeps_at_least_one_frame` | ⏳ |
| F7 | Fade in the start | Drag the start fade square to 0.5 s | First 0.5 s rises from silence; rest unchanged | Auto: `edit::audio::tests::reads_trims_clip_fades_and_crossfades` | ⏳ |
| F8 | Fades can't overlap | On a 2 s crop with a 1.5 s fade-out, drag the fade-in towards the end | Fade-in stops at 0.5 s | Auto: `clipHandles.test.ts`, `edit::tests::clip_fades_fit_and_follow_trims` | ⏳ |
| F9 | Fade follows a trim | Add a 0.5 s fade-in, then trim 1 s from the start | Fade-in still covers the first 0.5 s | Auto: `edit::tests::clip_fades_fit_and_follow_trims` | ⏳ |
| F10 | Smooth cut | Delete 4.0–5.0 s of A | 9.0 s; 3.995–4.005 s blends out of the cut and into the audio from just before 5.0 s | Auto: `edit::audio::tests::crossfade_blends_both_sides_at_the_join` | ⏳ |
| F11 | Longer crossfade | Drag the join's crossfade square out to 200 ms | Blend covers 100 ms each side; length unchanged | Auto: `clipHandles.test.ts` (crossfades) | ⏳ |
| F12 | Hard join | Drag the crossfade to zero | Direct switch at the join; samples unchanged | Auto: `edit::audio::tests::hard_join_is_a_plain_concatenation` | ⏳ |
| F13 | Limited by available audio | Make a join 20 ms from the end, drag its crossfade to 200 ms | Stops at the longest length the audio allows | Auto: `edit::tests::crossfades_are_clamped_by_the_audio` | ⏳ |
| F14 | Preview a fade | Drag the end fade square slowly | Fade line and tapered waveform follow the pointer; label shows the fade length; while playing, the fade is heard as you drag | | ⏳ |
| F14b | Hear a trim while playing | Play A to the end from 5.0 s; drag the end bracket from 10.0 s to 7.0 s without releasing | Playback ends at the audio that is at 7.0 s | Auto: `playback::engine::tests::removed_position_with_nothing_after_it_ends`, `tests/state.rs::previews_change_nothing` | ⏳ |
| F15 | Cancel a drag | Drag a trim bracket, press Escape, release | Audio, handles and undo history unchanged; selection kept | | ⏳ |
| F16 | Select near an edge | Press 20 px in from the start and drag right | A selection is made; no trim or fade | Auto: `clipHandles.test.ts` (hitTest) | ⏳ |
| F17 | Adjust twice, undo once | Set a 0.5 s fade-in, drag it to 1.0 s, Undo | Fade-in is 0.5 s again with the same curve | Auto: `tests/state.rs::adjusting_a_fade_replaces_it_and_undo_restores_it` | ⏳ |
| F18 | Save a shaped take | Trim both ends, add fades, Save As, reopen | File contains exactly what was shown; handles back at the edges, no fades | Auto: `tests/state.rs::saving_bakes_the_clip_settings` | ⏳ |
| F19 | Release during playback | Play A, drag the fade-out square and release | Playback continues; handles and waveform show the new fade; no progress box | Auto: `playback::engine::tests::fade_while_listening` | ⏳ |

## live-editing

Below, "A" is `cd-16bit-stereo-10s.wav` and "L" is `large-1gb.wav`. "Playing" means playing to the end unless a selection is looping.

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| L1 | Delete on a long file | Open L, select 10:00–20:00, Delete | Waveform and duration show the shorter result straight away, no progress box | Auto: `tests/state.rs::edits_apply_back_to_back_without_analysis` | ⏳ |
| L2 | Edit again straight away | Silence a selection in A, then at once Fade Out another selection | Both applied; Undo reverts the fade first | Auto: `tests/state.rs::edits_apply_back_to_back_without_analysis` | ⏳ |
| L3 | Undo is instant | Make several edits on L, hold Ctrl/Cmd+Z | Each step shows its result straight away, no progress box | Auto: `tests/state.rs::edits_apply_back_to_back_without_analysis` | ⏳ |
| L4 | No peak hidden | Open `single-spike-10min.wav`, Fade Out from 1:00 to the end, zoom to fit at once | The spike is visible at no less than its faded height | Auto: `peaks::tests::transient_in_a_long_fade_is_not_hidden`, `peaks::tests::edited_peaks_are_exact_or_never_hide_a_peak` | ⏳ |
| L5 | Exact when zoomed in | After L4, zoom in to single samples around the spike before the stats finish | The samples drawn are the faded samples | Auto: `peaks::tests::transient_in_a_long_fade_is_not_hidden`, `tests/state.rs::analysis_matches_a_file_with_the_same_audio` | ⏳ |
| L6 | Old view kept until redrawn | Open `beep-every-second-mono-60s.wav`, zoom in, Silence a selection | No blank or flicker; zoom and scroll stay as the edit rules say | | ⏳ |
| L7 | Rapid changes | Make five edits on L in quick succession | Whole-file stats update once, for the final state | Auto: `tests/state.rs::rapid_changes_are_analysed_once` | ⏳ |
| L8 | Selection stats stay current | On L, select a range, silence part of it straight away | Selection stats reflect the silence while whole-file stats still say "Updating…" | Auto: `tests/state.rs::silence_shows_in_selection_stats` | ⏳ |
| L9 | Stats updating | Edit L, look at the whole-file stats | "Updating…" with dimmed figures, then the new figures | Auto: `tests/state.rs::edits_apply_back_to_back_without_analysis`, `rapid_changes_are_analysed_once` | ⏳ |
| L10 | Clip markers removed at once | Open `two-clip-runs.wav`, silence a clipped run | Its markers disappear straight away | Auto: `stats::clipping::tests::runs_in_changed_audio_vanish` | ⏳ |
| L11 | Fade while listening | Play A from 3.0 s, select 5.0–8.0 s, Fade Out | Playback continues; the fade is heard from 5.0 s | Auto: `playback::engine::tests::fade_while_listening` | ⏳ |
| L12 | Delete before the playhead | Play L from 0:29, at 0:30 delete 0:10–0:20 | Same audio keeps playing without a jump; playhead moves to 0:20 | Auto: `playback::engine::tests::delete_before_the_playhead_keeps_playing_the_same_audio`, `edit::tests::delete_before_the_playhead_keeps_the_audio` | ⏳ |
| L13 | Delete the audio being played | Play L, at 0:15 delete 0:10–0:20 | Playback continues from the join at 0:10 | Auto: `playback::engine::tests::deleting_the_audio_being_played_continues_from_the_join`, `edit::tests::deleted_audio_continues_from_the_join` | ⏳ |
| L14 | Looping selection | Loop 2.0–3.0 s of A, silence part of the selection | Loop keeps playing 2.0–3.0 s with that part silent | Auto: `playback::engine::tests::looping_selection_keeps_its_range_through_silence` | ⏳ |
| L15 | Selection cleared by the edit | Play a selection of A, press Delete | Playback continues to the end of the file | | ⏳ |
| L16 | Shape a fade by ear | Loop a selection near A's end, drag the end fade square inward | Fade heard while dragging; one undo step on release | Auto: `tests/state.rs::previews_change_nothing` | ⏳ |
| L17 | Cancel a heard drag | Delete a range of A, play across the join, drag its crossfade square, press Escape | Sounds as before the drag; Undo unchanged | Auto: `tests/state.rs::previews_change_nothing` | ⏳ |
| L18 | Audition a curve | Loop a selection of A, Edit › Fade Out…, drag the point | Waveform under the dialog and the playing audio follow the bend | | ⏳ |
| L19 | Cancel the preview | Edit › Fade In…, drag the point, Cancel | Waveform and playback as before; Undo unchanged | Auto: `tests/state.rs::previews_change_nothing` | ⏳ |
| L20 | Many edits, no files | Make many edits to a copy of A without saving; watch its folder | No new files; the file is unchanged on disk | Auto: `tests/state.rs::edits_apply_back_to_back_without_analysis` | ⏳ |
