# Acceptance checklist

Manual acceptance pass over every scenario in the four v1 specs (`openspec/changes/add-wav-analyser-app/specs/`), plus the multi-file tabs and app icon change (`openspec/changes/add-multi-file-tabs/specs/`; sections file-tabs, app-branding, and rows I2b–I13) the dark theme change (section app-theme), and the session restore, tab reordering and window change (`openspec/changes/tab-session-and-window-fit/specs/`; sections app-window, tab-reorder and session-restore), and the audio editing change (`openspec/changes/add-audio-editing/specs/`; sections transport-controls, audio-editing and wav-export), and the clip fades change (`openspec/changes/add-clip-fades-and-crossfades/specs/`; section clip-fades, plus rows E4b, E8b, E8c and X8b), and the live editing change (`openspec/changes/add-live-editing/specs/`; section live-editing, plus rows E9, E10b, F14, F14b, F19, X13b and X13c, and E11 removed), and the VST3 effect chain change (`openspec/changes/add-vst3-effect-chain/specs/`; sections plugin-editors, effect-chain and vst3-plugins), and the session files change (`openspec/changes/add-session-files/specs/`; section session-files), and the save/export change (`openspec/changes/save-sessions-export-audio/specs/`; sections wav-export and session-files, rows X2b–X17 and SF22–SF26), and the replace-audio change (`openspec/changes/replace-session-audio/specs/`; section replace-audio, rows SF13, SF14 and SF27–SF30), and the tabbed sidebar change (`openspec/changes/tabbed-details-sidebar/specs/`; section details-sidebar, which replaces the audio-stats details panel rows, plus rows C3, C5 and FX1), and the level meter change (`openspec/changes/playback-level-meter/specs/`; section level-meter, plus row SB8), and the effect chain cap removal (`openspec/changes/archive/2026-10-18-remove-effect-chain-cap/`; row FX4), and the effect groups change (`openspec/changes/add-effect-groups/specs/`; section effect-groups), and the group presets change (`openspec/changes/add-group-presets/specs/`; section effect-presets).

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

## details-sidebar

From the tabbed sidebar change (`openspec/changes/tabbed-details-sidebar/specs/details-sidebar/`); replaces the old details panel rows D1–D4. "A" is `cd-16bit-stereo-10s.wav` and "B" is `pcm24-stereo-48k.wav`. The activity bar is the icon strip on the window's right edge: a bar chart (Metrics) above a plug (Effects).

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| SB1 | No files open | Start the app (or close every tab) | No activity bar and no sidebar; the empty screen spans the full width | | ⏳ |
| SB2 | Buttons identify their views | Open A, hover the plug icon, then the bar-chart icon | Tooltips name Effects and Metrics | | ⏳ |
| SB3 | Active view is highlighted | With the Metrics view shown, look at the activity bar | The bar chart is teal; the plug is not | | ⏳ |
| SB4 | Metrics view contents | Open A, show Metrics | Heading "Metrics", then the File and Levels sections; no Effects section | | ⏳ |
| SB5 | Effects view contents | Show Effects; add, bypass, open controls, reorder and remove an effect | Heading "Effects" and the chain; every control works as before; no File or Levels section | | ⏳ |
| SB6 | Playback continues while switching | Play A, then switch Metrics → Effects → Metrics | Playback carries on uninterrupted | | ⏳ |
| SB7 | Switch view | With Metrics shown, click the plug | The sidebar shows Effects | Auto: `sidebar.test.ts` | ⏳ |
| SB8 | Collapse by clicking the active button | With Effects shown, click the plug | The sidebar hides, the waveform widens up to the level meter (which stays beside it), no icon is highlighted | Auto: `sidebar.test.ts` | ⏳ |
| SB9 | Reopen from collapsed | With the sidebar collapsed, click the bar chart | The sidebar opens on Metrics | Auto: `sidebar.test.ts` | ⏳ |
| SB10 | Keyboard | Tab to the activity bar, use Up/Down to move between icons, press Enter and Space | Focus moves between the icons; Enter and Space act as a click | | ⏳ |
| SB11 | Badge counts effects | Add 3 effects to A | The plug shows a "3" badge, with the sidebar open or collapsed | | ⏳ |
| SB12 | Badge follows the active tab | With 2 effects on A and none on B, switch to B | No badge; switching back to A shows "2" | | ⏳ |
| SB13 | Add Effect while collapsed | Collapse the sidebar, Effects › Add Effect…, choose an effect | The sidebar opens on Effects showing the new effect | | ⏳ |
| SB14 | Add Effect while Metrics is shown | With Metrics shown, Effects › Add Effect…, choose an effect; then open Add Effect… again and cancel | The sidebar switches to Effects with the new effect; cancelling leaves the view as it was | | ⏳ |
| SB15 | Remembered across restart | Show Effects, quit, restart and open a file | The sidebar is open on Effects | Auto: `sidebar.test.ts` | ⏳ |
| SB16 | Collapsed state remembered | Collapse the sidebar, switch tabs, quit, restart and open a file | The sidebar stays collapsed until an icon is clicked | Auto: `sidebar.test.ts` | ⏳ |
| SB17 | First launch | Clear the app's web storage (or use a fresh profile), start and open a file | The sidebar is open on Metrics | Auto: `sidebar.test.ts` | ⏳ |
| SB18 | Upgrade with the panel hidden | In the previous build hide the details panel, then start this build and open a file | The sidebar starts collapsed | Auto: `sidebar.test.ts` | ⏳ |
| SB19 | Narrow window | Make the window narrower than 820 px with Metrics open; use Left/Right on the icons | The icons sit in a row above the Metrics view, which is below the waveform; the shown view's icon is teal; Left/Right move focus | | ⏳ |

## level-meter

From the level meter change (`openspec/changes/playback-level-meter/specs/level-meter/`). "A" is `cd-16bit-stereo-10s.wav`, "S" is `float32-extensible-5.1.wav`, "F" is `full-scale-sine-and-silence.wav` (left: 0 dBFS 1 kHz sine, right: silence), "R" is `reference-minus18dbfs-stereo.wav` and "B" is `beep-every-second-mono-60s.wav`. "+6 dB" means Pro-Q 4 (or any EQ) with its output gain at +6 dB.

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| LM1 | Beside the sidebar | Open A with the sidebar open | The meter runs the height of the waveform area, between the waveform and the sidebar | | ⏳ |
| LM2 | Sidebar collapsed | Collapse the sidebar | The meter stays, between the waveform and the activity bar | | ⏳ |
| LM3 | No files open | Close every tab | No meter | | ⏳ |
| LM4 | Stereo file | Open A | Two bars labelled L and R | | ⏳ |
| LM5 | 5.1 file | Open S | Six bars with the file's six channel labels | | ⏳ |
| LM6 | Effects are measured | Play R with +6 dB on the chain | Peaks at about −12 dBFS (−18 + 6) | Auto: `fx_tests::levels_are_measured_after_the_effects` | ⏳ |
| LM7 | Bypass All | During LM6, turn on Bypass All | Within 0.25 s the peaks return to about −18 dBFS | | ⏳ |
| LM8 | Volume does not move the meter | Play R, set the volume to 50% | Quieter, but the meter doesn't change | Auto: `engine::tests::levels_are_per_file_channel_unaffected_by_volume_and_follow_what_is_heard` | ⏳ |
| LM9 | In step with the sound | Play B | Each beep's rise on the meter coincides with hearing it and with the playhead crossing it | Auto (alignment): `engine::tests::levels_are_per_file_channel_unaffected_by_volume_and_follow_what_is_heard` | ⏳ |
| LM10 | Full-scale sine | Play F | L's peak bar reaches the top and its RMS bar settles at −3 dBFS within 0.5 s | Auto: `levels::tests::full_scale_sine_reads_0_db_peak_and_minus_3_db_rms` | ⏳ |
| LM11 | Silence | Play F | R's bars stay empty | | ⏳ |
| LM12 | Loud passage | Play F | The top of L's peak bar, above −6 dBFS, is amber; below is teal | | ⏳ |
| LM13 | Peak falls back | Play A, pause on a loud passage | The peak bars fall to empty in about 3 s | Auto: `meter.test.ts` | ⏳ |
| LM14 | Peak hold | Play A and watch a peak | The hold tick stays at the highest peak for 1.5 s, then falls | Auto: `meter.test.ts` | ⏳ |
| LM15 | Clipping after effects | Play F with +6 dB on the chain | L's clip light turns red and stays on after playback stops; R's doesn't | Auto: `meter.test.ts` | ⏳ |
| LM16 | No clipping | Play R with no effects | No clip light | Auto: `meter.test.ts` | ⏳ |
| LM17 | Readout after playing | Play A for a few seconds, pause | Each readout shows that channel's highest peak to one decimal place | | ⏳ |
| LM18 | Clear by clicking | After LM15, click the meter | The clip lights go out and the readouts show "–∞" | Auto: `meter.test.ts` | ⏳ |
| LM19 | New playback clears | After LM17, press Space twice (pause, then play); then click in the waveform while playing | The readouts start afresh on play; seeking while playing keeps them | | ⏳ |
| LM20 | Pause | Pause during a loud passage | The bars fall to empty; the readouts keep their values | | ⏳ |
| LM21 | Switch tabs | Play A, then switch to B (mono) | One empty bar with a "–∞" readout | | ⏳ |
| LM22 | Narrow window | Make the window narrower than 820 px and play A | Two horizontal bars below the waveform and above the activity bar, moving with the audio, scale along the bottom | | ⏳ |
| LM23 | Keyboard clear | Tab to the meter, press Enter (and Space) | The readouts and clip lights clear; Space doesn't start playback | | ⏳ |

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
| C3 | Panels stand out | Open a file with the sidebar shown | Sidebar, activity bar, toolbar and active tab are lighter than the window background; borders visible | | ⏳ |
| C4 | Reading the stats panel | Open `cd-16bit-stereo-10s.wav` | Values in whitish text, labels dimmed; both easy to read | Auto: `palette.test.ts` | ⏳ |
| C5 | Active tab and toggles | Open two files with the sidebar shown | Active tab marker and the shown view's activity bar icon are teal; no blue anywhere (check the volume slider and progress bar too) | | ⏳ |
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

Since the save/export change (`openspec/changes/save-sessions-export-audio/`), Save writes the session and Export writes audio; the WAV a tab edits is never written.

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| X1 | Edit marks the tab | Silence a selection in A | A dot replaces the tab's × (× on hover); tooltip says "Unsaved changes" | Auto: `tests/state.rs::an_edit_marks_the_tab_and_undoing_it_clears_the_mark` | ⏳ |
| X2 | Undo back to the original | One edit, then Undo | The dot disappears | Auto: `tests/state.rs::an_edit_marks_the_tab_and_undoing_it_clears_the_mark` | ⏳ |
| X2b | Plugin setting changed | Save A's session, turn a knob in an effect's editor, then Undo | The dot appears and stays after Undo | Auto: `tests/state.rs::a_change_in_a_plugin_editor_marks_the_tab`, `a_plugin_setting_marks_the_tab_until_saved` | ⏳ |
| X2c | Saving the session clears it | Edit A, Ctrl/Cmd+S and save the session | The dot goes; A is unchanged on disk; Undo is still available | Auto: `tests/state.rs::saving_the_session_clears_the_unsaved_mark` | ⏳ |
| X5 | Export a trimmed copy | Crop `take1.wav`, File › Export… as `take1-trim.wav` | New file has the cropped audio; original unchanged; the tab is still "take1.wav", still cropped, with its dot | Auto: `tests/state.rs::export_writes_the_edits_and_leaves_the_tab_alone` | ⏳ |
| X5b | Default name | File › Export… on `take1.wav` | The dialog proposes `take1-export.wav` next to it | Auto: `sessionFiles.test.ts` | ⏳ |
| X6 | Cancel the dialog | Export, then Cancel | Nothing written; the tab is unchanged | | ⏳ |
| X6b | Export over the source | Export `take1.wav` choosing `take1.wav` itself | Nothing written; notice says the audio being edited can't be overwritten | Auto: `tests/state.rs::export_refuses_the_audio_being_edited` | ⏳ |
| X7 | Name of another open file | With B open in another tab, Export over B's path | Nothing written; notice says it's open in another tab | | ⏳ |
| X8 | Untouched audio is bit-exact | Delete 1.0–2.0 s from `pcm24-stereo-48k.wav`, Export | Samples outside the join's crossfade identical to the source | Auto: `wav::writer::tests::untouched_samples_are_bit_exact_outside_the_crossfade` | ⏳ |
| X8b | Hard join is bit-exact | As X8 but drag the join's crossfade to zero before exporting | Every sample identical to the source | Auto: `wav::writer::tests::hard_join_is_bit_exact` | ⏳ |
| X9 | Metadata is not carried over | Edit `with-list-bext-chunks.wav`, Export, reopen the export | Only `fmt ` and `data` chunks; same format details | Auto: `wav::writer::tests::metadata_chunks_are_not_written`, `edited_file_keeps_the_format` | ⏳ |
| X10 | Result too large for WAV | (Not practical by hand) | Export fails with "too large" and writes nothing | Auto: `wav::writer::tests::refuses_results_over_4_gib` | ⏳ |
| X11 | Read-only target | Export into a read-only folder | Error notice; the tab keeps its edits, effects and dot | Auto: `tests/state.rs::a_failed_export_keeps_everything` | ⏳ |
| X11b | Replacing an earlier export | Export twice to the same name, confirming the replace | The file holds the second export; no temporary files are left | Auto: `tests/state.rs::exporting_again_replaces_an_earlier_export` | ⏳ |
| X13 | Keep working after export | Add an effect, zoom in, select 1.0–2.0 s, Fade In, Export | The effect, its settings, zoom and selection are unchanged; Undo removes the fade | Auto: `tests/state.rs::exporting_keeps_the_effects_and_their_settings` | ⏳ |
| X13c | Progress while rendering | Edit `large-1gb.wav`, Export | The delayed progress box ("Exporting") shows how far it has got; other tabs stay usable | | ⏳ |
| X14 | Cancel closing | Edit A, Ctrl/Cmd+W, Cancel (also try ×, middle-click, File › Close Tab) | Tab stays open with its edits | | ⏳ |
| X15 | Save on close | Open A's session, edit, close its tab, choose Save | The session file is written; A is unchanged; the tab closes | | ⏳ |
| X15b | Save on close without a session | Edit A (no session), close, choose Save; once Cancel the dialog | A dialog proposes `a.hdsession.json`; saving closes the tab; cancelling keeps it open | | ⏳ |
| X16 | Two edited files | Edit A and B, close the window (also File › Quit) | Asked about A, then B; quits after both answered | | ⏳ |
| X17 | Cancel quitting | Quit with edits, Cancel at the first prompt (and once by cancelling a Save dialog) | App keeps running with every tab and edit; quitting with no changes doesn't prompt | | ⏳ |

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

## plugin-editors

From the VST3 effect chain change (`openspec/changes/add-vst3-effect-chain/specs/plugin-editors/`). The steps open editors with an effect's controls button in the Effects panel. "A" is `cd-16bit-stereo-10s.wav` and "B" is `pcm24-stereo-48k.wav`.

**Plugins:** Valhalla Supermassive (free, fixed-size editor) and a plugin with a resizable editor: Voxengo SPAN (free) or FabFilter Pro-Q 4. On Linux, use a native Linux VST3 build, such as Surge XT (free, resizable). On macOS, also have an Intel-only VST3 bundle at hand for P15.

**Before the in-app pass (Windows, 2026-10-13):** the editor manager was driven directly against the installed plugins, without the Effects panel. With Supermassive and Pro-Q 4: the window was owned by the main window and titled "<effect> — a.wav"; Pro-Q 4's window was resizable and Supermassive's wasn't; enlarging Pro-Q 4's window resized its editor; a drag to 60×60 was fitted to Pro-Q 4's 776×374 minimum; hiding for another file and showing again kept the window's place; opening again kept one window; closing the window, removing the effect, and destroying the main window each closed the editor and sent `fx-editor-closed`.

### Windows

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| P1 | Open an editor | Open A, add Supermassive, click its controls button | A window titled "ValhallaSupermassive — a.wav" shows the editor at its size; the main window stays usable | Direct check above | ⏳ |
| P2 | Open twice | With P1's editor behind the main window, click the controls button again | The same window comes to the front; no second window | Direct check above | ⏳ |
| P3 | Stays above | Click the main window, then minimise and restore it | The editor stays above the main window, and hides and comes back with it | Direct check above (owner) | ⏳ |
| P4 | Plugin resizes its editor | Add Pro-Q 4, open its editor, resize the editor from the plugin's own size control | The window grows and shrinks to fit the editor | | ⏳ |
| P5 | Resize the window | Drag Pro-Q 4's window corner larger, then very small | The editor follows; the window stops at the plugin's minimum size. Supermassive's window can't be resized | Direct check above | ⏳ |
| P6 | Display scaling | With two displays at different scales (e.g. 100 % and 150 %), drag Pro-Q 4's editor from one to the other | The editor redraws sharp at the new scale and keeps its proportions | | ⏳ |
| P7 | Switch tabs | With A's editor open, switch to B, move nothing, switch back to A | The editor hides on B and reappears in the same place on A | Direct check above | ⏳ |
| P8 | Remove the effect | With its editor open, remove the effect | The editor closes; Undo brings the effect back with its editor closed | Direct check above | ⏳ |
| P9 | Close tab, save, quit | Open an editor, then in turn close its tab; save a file with an open editor; quit with an editor open | The editor closes each time; quitting doesn't hang or crash | Direct check above (owner destroyed) | ⏳ |
| P10 | Close the window | Close the editor window with its close button | It closes; the effect's controls button shows it closed | | ⏳ |
| P11 | Turn a knob, then save | Play A, turn Supermassive's Mix knob in its editor, close the editor, save | The change is heard within 0.25 s; the saved file has it; Undo is greyed out | | ⏳ |

### macOS

Check on Apple Silicon.

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| P12 | Open, twice, follow tabs | Repeat P1, P2, P7 and P8 | As on Windows; the editor is a child window that moves with the main window when the main window is dragged | | ⏳ |
| P13 | Resizing | Repeat P4 and P5 | As on Windows | | ⏳ |
| P14 | Close, save, quit | Repeat P9 and P10, and quit with Cmd+Q while an editor is open | As on Windows | | ⏳ |
| P15 | Intel-only plugin | Put an Intel-only `.vst3` in `~/Library/Audio/Plug-Ins/VST3`, rescan | Manage Plugins lists it as Incompatible, "No version for this Mac's processor"; it can't be added | Auto: `plugins::scan` architecture check | ⏳ |

### Linux

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| P16 | X11 session | In an X11 session, repeat P1, P2, P4, P5, P7, P8, P9 and P10 with Surge XT | As on Windows; the editor stays above the main window; its animations and meters run (run-loop timers) and it responds to the mouse (run-loop file descriptors) | | ⏳ |
| P17 | Wayland with XWayland | In a Wayland session with XWayland, open Surge XT's editor | The editor shows in its own window and works; it may not stay above the main window | | ⏳ |
| P18 | Wayland without XWayland | Run with `DISPLAY` unset in a Wayland session (`env -u DISPLAY npm run tauri dev`), open the controls of any effect | Generic controls show with the note that plugin editors need X11 or XWayland | | ⏳ |

### Generic controls

Check on any platform. Use a plugin without an editor of its own, or Show generic controls on one that has one.

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| P19 | Plugin without an editor | Open the controls of an effect that has no editor of its own | The generic controls panel lists its parameters with name, value and units (e.g. "Threshold −18.0 dB") and a slider each; the main window stays usable | Auto: `tests/state.rs::generic_controls_list_the_gain_parameter` | ⏳ |
| P20 | Change and reset a parameter | Play A, drag a slider, then double-click it | The change is heard within 0.25 s; double-clicking returns it to the plugin's default; Undo is unchanged | Auto: `tests/state.rs::a_generic_control_is_heard_from_the_next_block`, `generic_control_changes_are_not_undo_steps` | ⏳ |
| P21 | Both views agree | Open Pro-Q 4's editor and Show generic controls on it; turn a band's gain in the editor, then move that slider | The slider follows the knob; the editor follows the slider | Auto: `tests/state.rs::changes_in_a_plugin_editor_reach_the_generic_controls` | ⏳ |
| P22 | Follows its tab | With generic controls open, switch to another tab and back; then remove the effect | The panel hides on the other tab and comes back; it closes when the effect is removed | | ⏳ |

## effect-chain

From the VST3 effect chain change (`openspec/changes/add-vst3-effect-chain/specs/effect-chain/`). "A" is `cd-16bit-stereo-10s.wav`, "B" is `pcm24-stereo-48k.wav` and "S" is `float32-extensible-5.1.wav`. Plugins: Pro-Q 4 (EQ), Pro-C 3 (compressor), Pro-L 2 (limiter, with lookahead latency), Supermassive (reverb), and a stereo-only effect for FX5.

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| FX1 | Empty chain | Open A, click the plug icon | The Effects view has an "+ Add" button at the top, no effects and a short hint | | ⏳ |
| FX2 | Chains are per tab | Add Pro-C 3 to A, switch to B, play B | B's Effects section is empty and B plays unprocessed | | ⏳ |
| FX3 | Add an EQ | Add Effect…, type "eq", choose Pro-Q 4 | Pro-Q 4 is last in the chain and its editor opens | | ⏳ |
| FX4 | Long chain | Add 8 effects to A, then a ninth from "+ Add" and another from Effects › Add Effect… | Both stay enabled, and the new effects are ninth and tenth in the chain | Auto: `tests/state.rs::a_chain_has_no_length_limit`, `a_long_chain_is_restored_in_full` | ⏳ |
| FX5 | Stereo-only plugin on a 5.1 file | Open S, Add Effect… | The stereo-only effect is greyed out with "Doesn't support 5.1 audio"; Pro-Q 4 isn't | Auto: `effectBrowser.test.ts`, `tests/state.rs::stereo_only_plugin_is_refused_for_a_5_1_file` | ⏳ |
| FX6 | Scan progress in the browser | Rescan All in Manage Plugins, then open Add Effect at once | "Scanning plugins… n of m" with the effects found so far, updating | | ⏳ |
| FX7 | Reorder | With Pro-Q 4 then Pro-C 3, drag Pro-C 3 above Pro-Q 4 | The chain is Pro-C 3 then Pro-Q 4; Undo puts it back | Auto: `chainOrder.test.ts`, `tests/state.rs::moving_an_effect_reorders_the_chain` | ⏳ |
| FX8 | Keyboard reorder | Tab to the second effect, press Alt+Up | It becomes first and keeps the focus | Auto: `chainOrder.test.ts` | ⏳ |
| FX9 | A/B compare | Loop a selection of A with Pro-Q 4 cutting the lows hard; toggle Bypass All (panel button and Effects menu) | Playback switches between processed and dry within 0.25 s each time, without stopping; the menu item's tick follows | Auto: `playback::engine::fx_tests::bypass_all_is_heard_dry_after_one_flush` | ⏳ |
| FX10 | Save with a bypassed effect | Bypass Supermassive, keep Pro-Q 4 active, Save As | The saved file has the EQ and no reverb | Auto: `tests/state.rs::effects_are_applied_with_latency_removed`, `a_bypassed_chain_saves_bit_exact` | ⏳ |
| FX11 | Undo a removal | Set a Pro-Q 4 band to +6 dB, remove Pro-Q 4, Undo | Pro-Q 4 is back in its place with the +6 dB band | Auto: `tests/state.rs::undo_a_removal_brings_the_effect_back_with_its_settings` | ⏳ |
| FX12 | Edits and effects in one history | Silence a selection of A, add Pro-L 2, Undo twice | The first Undo removes Pro-L 2, the second the silence | Auto: `tests/state.rs::edits_and_effects_share_one_history` | ⏳ |
| FX13 | Effect alone marks the tab | Add Pro-Q 4 to an unedited A, close the tab | The tab shows the unsaved dot, and closing asks whether to save | Auto: `tests/state.rs::an_effect_alone_marks_the_file_unsaved` | ⏳ |
| FX14 | Everything bypassed | Bypass A's only effect | No unsaved dot; Save is greyed out | Auto: `tests/state.rs::everything_bypassed_is_not_unsaved` | ⏳ |
| FX15 | Adjust while listening | Play A through Pro-C 3, raise its ratio in the editor | Heard within 0.25 s; playback doesn't stop | Auto: `playback::engine::fx_tests::parameter_changes_are_heard_within_a_quarter_second` | ⏳ |
| FX16 | Effect with latency | Add Pro-L 2 with lookahead, play A from 2.0 s | The first audio heard is at 2.0 s and the playhead matches what is heard | Auto: `playback::engine::fx_tests::effects_are_heard_in_time_and_end_with_the_range` | ⏳ |
| FX17 | Effects not drawn | Add +12 dB of gain (Pro-Q 4 output gain) to A | The waveform and stats are unchanged | Auto: `tests/state.rs::chain_changes_do_not_start_an_analysis` | ⏳ |
| FX18 | After saving | Save FX17's file | The Effects section is empty, the editors close, and the waveform and stats show the louder audio; a notice says how many samples were clipped | Auto: `tests/state.rs::effects_are_baked_in`, `too_much_gain_counts_the_clamped_samples` | ⏳ |
| FX19 | No output device | Disable every output device, start the app, add Pro-Q 4 to a copy of A and save | Effects can be added and edited; the saved file has the EQ | | ⏳ |
| FX20 | Overloaded chain | Add 8 heavy instances (e.g. Supermassive at maximum), play A | One notice that the effects are too demanding to play smoothly; saving still renders them | Auto: `playback::engine::fx_tests::a_chain_too_slow_for_real_time_reports_one_overload` | ⏳ |
| FX21 | Close with editors open | Open two effect editors on A, close A with Don't Save | Both editor windows close | | ⏳ |
| FX22 | Effects menu without a file | Close every tab, open the Effects menu | Add Effect… and Bypass All Effects are greyed out; Manage Plugins… still works | | ⏳ |

## effect-groups

From the effect groups change (`openspec/changes/add-effect-groups/specs/effect-groups/`). "A" is `cd-16bit-stereo-10s.wav`. Unless a row says otherwise, start with A's chain as Pro-Q 4 (EQ), Pro-DS (de-esser), Pro-C 3 (compressor), Supermassive (reverb), no groups.

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| EG1 | Range selection | Click Pro-Q 4, Shift+click Supermassive's row | All four effects are highlighted; then Ctrl/Cmd+click Pro-DS: it is no longer highlighted and the others stay | Auto: `chainOrder.test.ts` | ⏳ |
| EG1b | Escape clears the selection | Select Pro-Q 4 and Pro-DS, and a region of the waveform; press Escape, then Escape again | The first Escape unhighlights both effects and leaves the waveform selection; the second clears the waveform selection | | ⏳ |
| EG2 | Group neighbours | Select Pro-DS and Pro-C 3, press Ctrl/Cmd+G | "Group 1" holds them, indented under its header; playback sounds the same as before | Auto: `tests/state.rs::grouping_neighbours_keeps_the_order_and_can_be_undone` | ⏳ |
| EG3 | Nothing selected | Click nowhere (or Ctrl/Cmd+click the selected effects off), open the Effects menu | Group Effects and Ungroup are greyed out, and so is the panel's Group button | | ⏳ |
| EG4 | Ungroup in place | Ungroup "Group 1" from its header, and again (after Undo) with Ctrl/Cmd+Shift+G after selecting Pro-DS | The chain is back to four effects, no group, same order | Auto: `tests/state.rs::ungrouping_leaves_the_effects_in_place` | ⏳ |
| EG5 | Collapse | Collapse "Group 1" | Its effects are hidden, the header says "2 effects", playback is unchanged, and no unsaved dot appears | | ⏳ |
| EG6 | Rename | Double-click "Group 1", type "Vocal cleanup", Enter; then F2 on the header, clear the name, Enter; then F2, type something, Escape | The header shows "Vocal cleanup" each time | Auto: `tests/state.rs::renaming_trims_and_refuses_empty_names` | ⏳ |
| EG6b | Rename the highlighted group | Click the "Group 1" header, press Ctrl/Cmd+R, type "Drums", Enter; then press Ctrl/Cmd+R with nothing highlighted; then check Effects › Rename Group with and without a group highlighted | The header shows "Drums" and the app doesn't reload; with nothing highlighted nothing happens, and the menu item is greyed out | | ⏳ |
| EG7 | Move a group | Drag the "Group 1" header above Pro-Q 4 | The group and both its effects are first; playback processes Pro-DS, Pro-C 3, Pro-Q 4 | Auto: `chainOrder.test.ts`, `tests/state.rs::moving_a_group_moves_its_effects_together` | ⏳ |
| EG8 | Drag into a group | With Pro-DS and Pro-C 3 grouped, drag Supermassive between them; then drag it just below Pro-C 3, first with the pointer at the indent and then at the left edge | It joins the group between them; below Pro-C 3 it stays in the group when indented (the drop line is indented) and leaves it at the left edge | Auto: `chainOrder.test.ts`, `tests/state.rs::effects_move_into_and_out_of_groups` | ⏳ |
| EG9 | Keyboard out of a group | Focus Pro-C 3, the last effect in "Group 1", press Alt+Down | Pro-C 3 is just after the group, keeps the focus, and is no longer indented | Auto: `chainOrder.test.ts` | ⏳ |
| EG10 | Remove a group | Open Pro-DS's editor, remove "Group 1" from its header, then Undo | Pro-DS, Pro-C 3 and the editor go; Undo brings back the group, named as before, with both effects and their settings | Auto: `tests/state.rs::removing_a_group_can_be_undone_with_its_settings` | ⏳ |
| EG11 | Bypass a group, then restore | Bypass Pro-C 3 on its own, then loop a selection and toggle the group's bypass on and off | With it on, neither Pro-DS nor Pro-C 3 is heard and both rows dim; with it off, Pro-DS is heard again and Pro-C 3 is still bypassed | Auto: `tests/state.rs::group_bypass_holds_its_effects_and_keeps_their_own_switches` | ⏳ |
| EG12 | Export with a bypassed group | Group Supermassive alone, bypass the group, keep Pro-Q 4 active, export | The exported file has the EQ and no reverb | Auto: `tests/state.rs::a_bypassed_group_is_left_out_of_an_export` | ⏳ |
| EG13 | Round trip with a group | With "Vocal cleanup" bypassed, save the session, close A, open the session | The chain shows Pro-Q 4, then "Vocal cleanup" (bypassed, expanded) with Pro-DS and Pro-C 3 and their settings, then Supermassive | Auto: `tests/state.rs::groups_are_saved_and_restored` | ⏳ |
| EG14 | Older build opens a grouped session | Open EG13's session in a release from before groups (0.2.1) | No tab opens and the notice says it was made by a newer version | Auto: `session::tests::groups_round_trip_as_version_2` | ⏳ |

## effect-presets

From the group presets change (`openspec/changes/add-group-presets/specs/effect-presets/`). "A" is `cd-16bit-stereo-10s.wav` and "B" is `cd-16bit-stereo-10s.wav` copied to `b.wav`. Unless a row says otherwise, start with no presets, and A's chain as "Vocal cleanup" holding Pro-Q 4 (EQ) with a +6 dB band and a bypassed Pro-C 3 (compressor). The library is `effect-presets.json` in the app data folder (on Windows `%APPDATA%\io.github.rnevillecodes.hitherdither`).

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| EP1 | Save a group | Save A's session, then press the save button on "Vocal cleanup"'s header and press Save, keeping the name | Effects › Insert Preset lists "Vocal cleanup"; A shows no unsaved dot and Undo is unchanged | Auto: `tests/state.rs::saving_a_group_as_a_preset_keeps_settings_and_leaves_the_tab_alone` | ⏳ |
| EP1b | Save from the menu | Open the Effects menu with nothing highlighted, then click the "Vocal cleanup" header and choose Effects › Save Group as Preset… | Save Group as Preset… is greyed out with nothing highlighted; with the group highlighted it asks for a name, defaulting to "Vocal cleanup" | | ⏳ |
| EP1c | Invalid name | In the name prompt, clear the name, then type 65 characters | Save is greyed out and the prompt says a name must be 1 to 64 characters | Auto: `presetName.test.ts`, `presets::tests::rejects_invalid_names` | ⏳ |
| EP2 | Effect can't save its settings | Save a group holding a plugin that fails to report its state (skip if none is at hand) | No preset is saved and a notice names the effect whose settings could not be saved | | ⏳ |
| EP3 | Name taken | Save "Master bus", then save another group as "master bus"; press Cancel, then Save again and press Replace | The app asks whether to replace "Master bus"; Cancel keeps the name prompt open and the preset unchanged; Replace overwrites it | Auto: `presets::tests::a_taken_name_is_replaced_only_when_asked`, `tests/state.rs::saving_a_group_as_a_preset_keeps_settings_and_leaves_the_tab_alone` | ⏳ |
| EP4 | Available after restarting | Save "Vocal cleanup", quit, start the app again | Effects › Insert Preset lists "Vocal cleanup" | Auto: `presets::tests::saves_and_reopens_from_disk` | ⏳ |
| EP5 | No presets | With no presets, open Effects › Insert Preset | It shows a greyed-out "No presets" item | | ⏳ |
| EP5b | Sorted | Save "vocal", "Master bus" and "drums" | Insert Preset lists drums, Master bus, vocal | Auto: `presets::tests::lists_by_name_ignoring_case` | ⏳ |
| EP6 | Insert into another tab | Save "Vocal cleanup", open B (no effects), choose Effects › Insert Preset › Vocal cleanup, then Undo | B's chain is the group "Vocal cleanup" (not bypassed) holding Pro-Q 4 with its +6 dB band and the bypassed Pro-C 3, and B shows the unsaved dot; one Undo removes it | Auto: `tests/state.rs::inserting_a_preset_adds_a_group_in_one_undo_step` | ⏳ |
| EP6b | Nothing to insert into | Close every tab, open Effects › Insert Preset | Its presets are greyed out | | ⏳ |
| EP7 | Not linked | Insert "Vocal cleanup" into B, change the inserted EQ's band, insert "Vocal cleanup" again | The second group's EQ has the +6 dB band from the preset | Auto: `tests/state.rs::an_inserted_group_is_not_linked_to_its_preset` | ⏳ |
| EP8 | Stereo-only effect on a 5.1 file | Save a group of HD Test Gain and HD Test Stereo as a preset, insert it into `float32-extensible-5.1.wav` | The new group holds only HD Test Gain, and one notice names HD Test Stereo and says it doesn't support 5.1 audio | Auto: `tests/state.rs::a_stereo_only_preset_effect_is_left_out_of_a_5_1_tab` | ⏳ |
| EP8b | Nothing can be inserted | Save a group of HD Test Stereo alone, insert it into `float32-extensible-5.1.wav` | The chain is unchanged and an error notice says the preset could not be inserted, naming HD Test Stereo | Auto: `tests/state.rs::a_preset_with_nothing_insertable_leaves_the_chain_alone` | ⏳ |
| EP8c | Disabled plugin | Untick Pro-C 3 in Manage Plugins, insert "Vocal cleanup" into B | The group holds only Pro-Q 4, and the notice says Pro-C 3 is disabled | | ⏳ |
| EP9 | Rename | In Effects › Manage Presets…, rename "Vocal cleanup" to "Vocals"; then rename "Vocals" to the name of another preset and press Replace | The dialog and Insert Preset show "Vocals" at once; replacing removes the other preset; no tab changes | Auto: `presets::tests::rename_keeps_the_id` | ⏳ |
| EP10 | Delete | Insert "Master bus" into A, then delete "Master bus" in Manage Presets and confirm | "Master bus" is gone from the dialog and from Insert Preset, and the group inserted in A is unchanged | Auto: `presets::tests::deletes` | ⏳ |
| EP11 | Unreadable library | Quit, replace `effect-presets.json` with `{ not json`, start the app; then save a preset | A notice says the presets could not be read, and Insert Preset shows "No presets"; the file is unchanged until the save replaces it | Auto: `presets::tests::an_unreadable_file_opens_empty_and_is_kept_until_the_next_save` | ⏳ |

## vst3-plugins

From the VST3 effect chain change (`openspec/changes/add-vst3-effect-chain/specs/vst3-plugins/`). Steps use Effects › Manage Plugins…. Plugins: those installed on the test machine, plus Vital (free synth) for V5.

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| V1 | Plugins in the standard folders | Start the app | Effects from the standard folders appear in Add Effect once scanned, and Manage Plugins lists the standard folders, which can't be removed | | ⏳ |
| V2 | Standard folder missing | On Linux without `/usr/local/lib/vst3`, start the app | Scanning finishes with no error or notice | Auto: `tests/plugins.rs::missing_standard_folder_is_skipped_silently` | ⏳ |
| V3 | Add a folder | Copy a plugin to `D:\Plugins\VST3`, Add Folder… there, then restart | Its effects appear once the rescan finishes, and the folder is still listed after restarting | Auto: `tests/plugins.rs::extra_folders_and_disabled_plugins_are_remembered` | ⏳ |
| V4 | Remove a folder | With an effect from V3's folder in a chain, Remove the folder | Its plugins leave Manage Plugins and Add Effect; the effect in the chain keeps working until removed | | ⏳ |
| V5 | Instrument plugin | Find Vital in Manage Plugins | It's listed as "No usable effects" and isn't offered in Add Effect | Auto: `tests/plugins.rs::scan_offers_effects_only_with_their_layouts` | ⏳ |
| V6 | Bundle with two effects | Find a bundle with several effect classes | Each is offered in Add Effect as its own entry | Auto: `tests/plugins.rs::scan_offers_effects_only_with_their_layouts` | ⏳ |
| V7 | Crashing or hanging plugin | Put `hd-test-crash.vst3` and `hd-test-hang.vst3` (from the Rust test helper) in an extra folder, Rescan All | The app keeps running; after 20 s they show as Failed with "Crashed while being scanned" and "Timed out while being scanned"; the others scan meanwhile | Auto: `tests/plugins.rs::crashing_plugin_is_reported_and_isolated`, `hanging_plugin_times_out_while_others_scan` | ⏳ |
| V8 | Open files while scanning | Rescan All, then at once open, play and edit A | All work as normal | | ⏳ |
| V9 | Second launch is quick | Restart the app with the same plugins | Every plugin is available straight away, with no scan progress shown | Auto: `tests/plugins.rs::second_scan_of_unchanged_bundles_launches_nothing` | ⏳ |
| V10 | Updated plugin | Update one plugin to a new version, restart | Only it is scanned again; Manage Plugins shows the new version | Auto: `tests/plugins.rs::touching_one_bundle_rescans_only_it` | ⏳ |
| V11 | Statuses | Look through Manage Plugins | Each plugin shows name, vendor, version, location and status (Ready, Failed, Incompatible, Disabled, No usable effects), with the reason when failed or incompatible | | ⏳ |
| V12 | Disable a plugin | Untick Pro-C 3, restart | It's listed as Disabled and isn't offered in Add Effect; a Pro-C 3 already in a chain kept working before the restart | Auto: `tests/plugins.rs::extra_folders_and_disabled_plugins_are_remembered` | ⏳ |
| V13 | Crash while adding an effect | End the app's process (Task Manager) while a large plugin is being added, then start it again | A notice says the plugin was disabled because the app closed unexpectedly while loading it; it's listed as Disabled | Auto: `tests/plugins.rs::leftover_marker_disables_the_class_with_one_notice` | ⏳ |
| V14 | Re-enable after a crash | Tick V13's plugin again | It's offered in Add Effect again | Auto: `tests/plugins.rs::leftover_marker_disables_the_class_with_one_notice` | ⏳ |

## session-files

From the session files change (`openspec/changes/add-session-files/specs/session-files/`). "A" is a copy of `cd-16bit-stereo-10s.wav` in a folder of its own (here `takes/a.wav`). Plugins: a free plugin with a native editor and a bypass parameter, such as Valhalla Supermassive, and HD Test Gain or Pro-Q 4 for a second effect.

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| SF1 | First save asks | Open A, press Ctrl/Cmd+S | A save dialog proposes `a.hdsession.json` in `takes/` | | ⏳ |
| SF2 | Edits and effects captured | Delete a selection, fade out the end, add Supermassive (change Mix and Decay in its editor) and Pro-Q 4, bypass Pro-Q 4, Save, then open the file in a text editor | The JSON lists the edits and both effects in order, Pro-Q 4 marked bypassed; `a.wav` is unchanged (same size and date) | Auto: `tests/state.rs::a_session_captures_the_edits_and_effects` | ⏳ |
| SF3 | Round trip | Quit the app (Don't Save), start it, close `a.wav`'s restored tab, open `a.hdsession.json` | A tab for `a.wav` with the same waveform, clip handles and chain; Pro-Q 4 bypassed; Supermassive's editor shows the same Mix and Decay; Undo is greyed out and the tab has no unsaved dot; playback sounds the same | Auto: `tests/state.rs::a_restored_session_sounds_and_saves_the_same` | ⏳ |
| SF4 | Exported output matches | Before SF3's quit, Export `first.wav`; after SF3, Export `second.wav`; compare the two files | Identical | Auto: `tests/state.rs::a_restored_session_sounds_and_saves_the_same` | ⏳ |
| SF5 | Later saves overwrite | In SF3's tab, change Decay, press Ctrl/Cmd+S | No dialog; reopening the session shows the new Decay | | ⏳ |
| SF6 | Name typed without the extension | File › Save As…, type `mix`, then again with `mix.json` | Saved as `mix.hdsession.json` both times; the second asks before replacing it | Auto: `sessionFiles.test.ts` | ⏳ |
| SF7 | Cancel the dialog | File › Save As…, Cancel | Nothing is written; Save still writes the tab's previous session file | | ⏳ |
| SF8 | Not available while loading | Open a large fixture and open the File menu while it loads; again while exporting | Save and Save As… are greyed out | | ⏳ |
| SF9 | Change made while stopped | Stopped, set HD Test Gain to +6 dB in its generic controls, Save, close the tab, reopen the session | Gain shows +6.0 dB and is heard on play | Auto: `tests/state.rs::a_session_includes_changes_made_while_stopped` | ⏳ |
| SF10 | Effects all bypassed | Session of an unedited file whose only effect is bypassed; reopen it | No unsaved dot | Auto: `tests/state.rs::a_restored_session_with_only_bypassed_effects_is_not_unsaved` | ⏳ |
| SF11 | Mixed selection | Open… and pick `cd-16bit-stereo-10s.wav` and `a.hdsession.json` together | Each opens in its own tab, in that order, the session restored | Auto: `sessionFiles.test.ts` (`planOpens`) | ⏳ |
| SF12 | Folder moved | Close the tabs, move `takes/` elsewhere, open the session from its new place | `a.wav` opens from the new folder with the session restored | Auto: `session::tests::resolves_absolute_then_relative` | ⏳ |
| SF13 | WAV missing | Delete `a.wav`, open the session, choose Cancel in the dialog | A dialog says the audio file `a.wav` could not be found and offers Choose Another WAV…; after Cancel no tab is left open | Auto: `tests/state.rs::a_missing_wav_is_reported_by_name` | ⏳ |
| SF14 | WAV re-rendered shorter | Overwrite `a.wav` with a shorter WAV, open the session, choose Cancel | Its tab closes; the dialog says the audio file no longer matches the session and offers Choose Another WAV… | Auto: `tests/state.rs::a_session_is_not_applied_to_a_different_recording` | ⏳ |
| SF15 | WAV touched but same format | Copy `a.wav` over itself (new date), open the session | The session is restored, with a warning that the audio file has changed since the session was saved | Auto: `tests/state.rs::a_touched_wav_is_restored_with_a_warning` | ⏳ |
| SF16 | Same WAV open | With `a.wav` open, open its session | `a.wav`'s tab becomes active, unchanged, and a notice says to close it first | | ⏳ |
| SF17 | Plugin uninstalled | Disable Supermassive in Manage Plugins, open SF2's session | The chain is Pro-Q 4 alone, with its settings; one notice names Supermassive and why | Auto: `tests/state.rs::effects_that_cant_be_found_are_left_out` | ⏳ |
| SF18 | Saved state rejected | In the session's JSON, replace an effect's `state.component` with `AQID`, open it | The effect is in the chain at its defaults; the notice says its settings could not be restored | Auto: `tests/state.rs::an_effect_that_rejects_its_settings_keeps_its_defaults` | ⏳ |
| SF19 | Not a session | Write "hello" into `x.hdsession.json`, open it | No tab; the notice says it is not a valid session file | Auto: `session::tests::rejects_other_files_and_newer_versions` | ⏳ |
| SF20 | Newer format | Set `"version": 99` in a session, open it | No tab; the notice says it was made by a newer version of HitherDither | Auto: `session::tests::rejects_other_files_and_newer_versions` | ⏳ |
| SF21 | Other JSON file dropped | Drop `notes.json` on the window | "Could not open file", not a session | Auto: `sessionFiles.test.ts` | ⏳ |
| SF22 | Restored session is clean | Open a saved session | No unsaved dot; bypassing one of its effects shows it | Auto: `tests/state.rs::a_change_after_restoring_marks_the_tab` | ⏳ |
| SF23 | Save As switches session | From `mix.hdsession.json`, Save As `mix-alt.hdsession.json`, change something, Ctrl/Cmd+S | `mix.hdsession.json` is unchanged; `mix-alt.hdsession.json` has the change | | ⏳ |
| SF24 | Nothing new to save | Open a session and change nothing | File › Save is greyed out; Save As… and Export… aren't | | ⏳ |
| SF25 | Session tab after a restart | Open `mix.hdsession.json` and `b.wav`, quit (Don't Save after editing the session tab), start the app | The session reopens as last saved, then `b.wav` as a plain file, in that order | Auto: `session.test.ts` (`rememberedTabs`) | ⏳ |
| SF26 | Shaped take in a session | Trim both ends, add fades, save the session, close, reopen it | The handles show the same trims and fades | Auto: `tests/state.rs::a_restored_session_sounds_and_saves_the_same` | ⏳ |
| SF27 | Renamed WAV | Rename `a.wav` to `a-final.wav`, open the session, Choose Another WAV…, pick `a-final.wav` | The tab shows `a-final.wav` with the session's edits and effects, and the unsaved dot | Auto: `tests/state.rs::a_renamed_wav_keeps_the_session_edits` | ⏳ |
| SF28 | Different recording | With SF14's shorter `a.wav`, open the session, Choose Another WAV…, pick a WAV of another length | The tab shows it unedited with the session's effects and settings; a notice says the edits were left out | Auto: `tests/state.rs::a_different_recording_keeps_only_the_effects` | ⏳ |
| SF29 | Saving points the session at the new WAV | After SF27, Ctrl/Cmd+S, close the tab, open the session again | It opens `a-final.wav` with no dialog | | ⏳ |
| SF30 | Chosen WAV already open | With `b.wav` open, open a broken session and choose `b.wav` | `b.wav`'s tab becomes active, unchanged; a notice says to close it first | Auto: `sessionFiles.test.ts` (`planOpens`) | ⏳ |

## replace-audio

From the replace-audio change (`openspec/changes/replace-session-audio/specs/replace-audio/`). "A" is `takes/take1.wav` (stereo) with Supermassive and HD Test Gain (+6 dB) in its chain, the second bypassed; "B" is `takes/take2.wav` (stereo) and "M" a mono WAV.

| # | Scenario | Steps | Expected | Evidence | Result |
|---|---|---|---|---|---|
| RA1 | Replace with another take | With Supermassive's editor open, File › Replace Audio…, pick B | The editor closes; the tab shows `take2.wav` with both effects in order, the same settings (reopen Supermassive's editor) and HD Test Gain still bypassed | Auto: `tests/state.rs::replacing_the_audio_keeps_the_effects` | ⏳ |
| RA2 | Cancel the dialog | Replace Audio…, Cancel | The tab is unchanged | | ⏳ |
| RA3 | Confirmation shown | Fade in a selection, Replace Audio…, pick B; first Cancel, then Replace | Cancel leaves the fade; Replace shows B unedited with Undo greyed out | | ⏳ |
| RA4 | No confirmation without edits | Open a session with effects and no edits, Replace Audio…, pick B | Replaced without a dialog | Auto: `editRules.test.ts` | ⏳ |
| RA5 | Open in another tab | With B open in another tab, Replace Audio… in A's tab, pick B | Nothing changes; a notice says B is open in another tab | | ⏳ |
| RA6 | Not a WAV | Replace Audio…, pick a text file renamed `.wav` | A keeps its audio, edits and effects; "Could not replace the audio" gives the reason | Auto: `tests/state.rs::a_failed_replacement_changes_nothing` | ⏳ |
| RA7 | Session follows the new audio | A opened from `mix.hdsession.json`: Replace Audio with B, Ctrl/Cmd+S, close, reopen the session | It opens `take2.wav` with the same effects | Auto: `tests/state.rs::replacing_marks_a_saved_tab_unsaved` | ⏳ |
| RA8 | Stereo-only effect, mono take | With a stereo-only effect in A's chain, Replace Audio with M | The stereo-only effect is left out; a notice says it doesn't support mono audio | Auto: `tests/state.rs::effects_that_dont_fit_the_new_audio_are_left_out` | ⏳ |
| RA9 | Marked unsaved | Save A's session, then Replace Audio with B | The unsaved dot appears; Export renders B through the kept effects | Auto: `tests/state.rs::replacing_marks_a_saved_tab_unsaved`, `replacing_the_audio_keeps_the_effects` | ⏳ |
