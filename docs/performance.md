# Performance

## Waveform scrolling (waveform-view: "Scrolling a long file")

Target: ≥ 30 fps while scrolling a one-hour stereo 48 kHz file at the default window size.

**How to measure:**

1. Generate fixtures (`cd src-tauri && cargo run --release --example gen_fixtures -- ../fixtures --large`).
2. Run the release build (`npm run tauri build`, then start the binary) or `npm run tauri dev`.
3. Open `fixtures/one-hour-stereo-48k.wav` and zoom in a few steps so the view isn't fit-to-file.
4. Press Ctrl/Cmd+Shift+B. The app scrolls continuously for a few seconds, then shows a notice with average fps, 95th-percentile frame time and draw times.

## Results

| Date | Machine / OS | Build | Window | Avg fps | p95 frame | Avg draw | Pass |
|---|---|---|---|---|---|---|---|
| _pending_ | | | | | | | |

## Large-file load

`fixtures/large-1gb.wav` (`--large`), about 1 GiB of 16-bit stereo audio at 48 kHz.

| Date | Machine / OS | Build | Load time | Window responsive during load | Pass |
|---|---|---|---|---|---|
| _pending_ | | | | | |
