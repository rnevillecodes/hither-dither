# Design

## Context

- **Playback pipeline** (`src-tauri/src/playback/engine.rs`): a feeder thread reads file frames, runs them through the tab's live effect chain (`Live`, on the file's own channels at the file rate), maps them to device channels (`ChannelMap`, which may down-mix and soft-limit), resamples if needed, and pushes them into an `rtrb` ring buffer. `Engine::render` (the audio callback) drains the ring, applies the volume gain, and publishes the audible file frame (`audible` + `stamp_nanos`, latency-compensated). The ring holds up to 0.2 s (0.15 s while effects run), so the feeder runs ahead of what is heard.
- **Output indices**: the feeder counts output frames it has queued (`Feeder::written`); the engine counts output frames it has played (`out_index`). Switches (edits during playback) flush queued output: `flush_to`/`flush_anchor` tell the engine where the new audio starts.
- **Position reporting** (`commands.rs::play`): a thread sends `PlaybackEvent::Position { frame }` every 16 ms (`POSITION_INTERVAL`) over a Tauri `Channel`, plus `Ended` and `Overload`.
- **Frontend**: `transport.svelte.ts` receives events and interpolates the playhead each animation frame. `App.svelte` lays out `wave | sidebar | activity bar` in a grid (from the `tabbed-details-sidebar` change on this branch).
- **Levels table convention** (`stats/levels.rs`): peak = max |sample|, RMS = sqrt(mean of squares), with no +3 dB sine correction. The meter uses the same definitions so the numbers agree.

## Goals / Non-Goals

**Goals:**
- Per-file-channel peak and RMS of the post-effects, pre-volume audio, aligned with what is heard to within one report interval (16 ms) plus one measurement block.
- No work or allocation added to the realtime audio callback.
- Pure, tested logic on both sides: block measurement and windowing in Rust, scale and ballistics in TypeScript.

**Non-Goals:**
- Loudness (LUFS/LU) metering, true-peak (inter-sample) detection, or K-system scales.
- Metering when stopped (e.g. scrubbing or levels at the playhead).
- Metering the device output (after mapping, the down-mix limiter or volume).
- A setting to hide the meter or change its scale or ballistics. These could follow later as View menu items.

## Decisions

**Measure in the feeder, after `Live`, before `ChannelMap`.** This is the only point where the audio is post-effects but still in file channels at the file rate. *Alternative:* measure in `Engine::render`. Rejected: that's device channels (a mono file shows two bars, 5.1 is down-mixed and limited), it includes volume, and it puts work in the realtime callback.

**Measurement blocks tagged with output position.** The feeder splits each processed chunk into 256-frame blocks (about 6 ms at 44.1 kHz). For each block and channel it records `peak` (max |x|) and `sum_sq` and `frames`, plus `out_end`: the output index at which the block will have been heard, `base + round(frames measured since the latest switch × out_rate / file_rate)`, as the loop markers compute it (the feeder's `written` count would lag behind while the resampler holds input). Blocks go into a `Mutex<VecDeque<LevelBlock>>` on `Shared`, capped at 1 s of blocks; the oldest are dropped. The feeder isn't realtime, and the reporter takes the lock briefly every 16 ms, so a mutex is fine. *Alternative:* a second `rtrb` ring. Not needed, since neither side is realtime and the deque makes windowing simpler.

**Aligning to what is heard.** The engine already publishes the audible file frame. It also needs to publish the **audible output index**: `out_index − latency × device_rate`, written next to `audible` with the same stamp so it can be interpolated the same way. The reporter does this every tick:
1. Read the audible output index `a`.
2. Pop every block with `out_end ≤ a` into a short history covering 300 ms.
3. `peak` for this report is the max over blocks popped since the last report (so a short transient between reports isn't missed). `rms` is `sqrt(Σ sum_sq / Σ frames)` over the history.
4. On a switch, some measured audio is never heard: the rest of the stale audio the engine stops playing, and new audio it skips because time moved on while waiting. When the engine completes a switch it publishes the output range it jumped over (`skip_from`, `skip_to`, bumped `skip_seq`: three atomic stores, nothing else in the callback). The reporter reads the audible index first, then the skip, and drops blocks ending in that range before advancing. (The plan first had the feeder drop blocks past `flush_to`; it always finishes writing before taking a switch, so there are none. The engine-side skip is where audio is lost.)

This logic lives in a new `src-tauri/src/playback/levels.rs` (`LevelBlock`, `Meter` accumulator with `push_block`, `drop_range`, `advance_to(a) -> Option<Levels>`) and is unit-tested without a device.

**IPC shape.** `PlaybackEvent::Position { frame, levels }` where `levels: Vec<[f32; 2]>` holds linear `[peak, rms]` per file channel. It's empty when nothing new has been heard (e.g. before the first callback). Linear values keep the backend free of display policy; the frontend converts to dB. The payload is about 20 bytes per channel every 16 ms. *Alternative:* a separate `Levels` event. Rejected: it needs its own timing, and the two would drift.

**Frontend split.**
- `src/lib/meter.ts` (pure, Vitest):
  - `dbfs(linear)`;
  - `levelToFraction(db)` for the −60…0 scale;
  - `stepBallistics(state, input | null, dtSeconds)`, which handles peak release at 20 dB/s, hold of 1.5 s then release, max-peak and clip latching;
  - `clearReadouts(state)`.
- `src/lib/meterFeed.ts`: a plain (non-reactive) holder for the latest levels and a sequence number. `transport.svelte.ts` writes to it from position events and clears it on play start. Keeping it out of `$state` avoids triggering Svelte updates 60 times a second.
- `src/components/LevelMeter.svelte`:
  - a `<canvas>` sized with `devicePixelRatio`, redrawn in a `requestAnimationFrame` loop that runs while playing or while bars are still falling, and stops once everything is at rest;
  - the readouts are DOM text above the canvas (so they are selectable and readable by screen readers), updated at most 10 times a second;
  - the whole meter is a `<button>`-like focusable element (`role="button"`, `aria-label="Level meter"`) that clears on click, Enter or Space. Readouts sit in an `aria-live="off"` region with `aria-label`s like "L maximum peak −4.2 dBFS".

**Drawing.**
- Each channel is a column 30px wide, so the readout fits. A shared scale column of about 24px sits on the left, with tick labels at 0, −6, −12, −18, −24, −36 and −60.
- The peak bar is 12px wide, in `--accent` (or `--warning` from −6 dBFS up). The RMS bar is 6px wide, centred and lighter (`--wf-wave`).
- The hold tick is a 2px line. The clip light is a 6px box in `--danger` when latched, otherwise `--border`.
- Colours are read from CSS variables at mount, like `waveform/theme.ts`.
- A stereo meter is therefore about 96px wide.

**Layout.** `App.svelte` grid becomes `1fr auto 340px 44px` (sidebar open) / `1fr auto 44px` (collapsed), with a new `meter` area. Below 820px: rows `wave / meter / bar / side`, and the meter draws horizontally (scale along the bottom, readouts at the right end).

**Switching tabs.** `LevelMeter` is keyed on the active tab id, so switching remounts it with the new channel count and fresh state. Playback already stops on a tab switch, so no stale levels arrive.

## Risks / Trade-offs

- [Wide meter for many channels: 5.1 is about 210px] → Acceptable for an occasional format. If it proves too wide, narrow the columns above two channels and show readouts on hover. This would only change the drawing, not the spec.
- [Block granularity: 256 frames is 2.7 ms at 96 kHz and 5.8 ms at 44.1 kHz] → Fine for a visual meter. Peaks between reports are kept by taking the max.
- [Alignment across a switch (edit during playback)] → The engine publishes the range it skips and the reporter drops it. If two switches complete between reports (16 ms), only the latest range is dropped; edits are far rarer than that. An engine test covers an edit mid-playback and fails without the skip.
- [Effects with latency] → `Live` already removes the chain latency before output, so measured blocks line up with output indices (block boundaries shift by the latency, which only matters to tests). The HD Test Gain test (64 frames latency) covers it.
- [CPU] → One pass over the processed samples (max and multiply-add) per chunk. That's negligible next to resampling and effects.
- [`details-sidebar` wording] → Its "Collapse" scenario says the waveform widens "up to the activity bar". With the meter in between, it becomes "up to the meter". tasks.md updates that change's spec.

## Migration Plan

Additive. The `position` event gains a field; the frontend in this change is its only consumer. Rollback reverts both sides together.
