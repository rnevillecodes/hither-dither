# Tasks

## 1. Palette and contrast test

- [x] 1.1 Replace the light palette and the `prefers-color-scheme: dark` block in `src/styles.css` with the single dark palette from design Decision 2 on `:root`, and set `color-scheme: dark` and `accent-color: var(--accent)`. Verify `npm run build` passes and `grep prefers-color-scheme src` finds nothing in `styles.css`.
- [x] 1.2 Add `src/lib/palette.test.ts`. It parses the `:root` hex variables from `src/styles.css` and asserts the spec's minimums: `--text` ≥ 7:1 and `--text-dim` ≥ 4.5:1 on `--bg`, `--panel-bg`, `--button-bg` and `--button-hover`; `--accent-text` ≥ 4.5:1 on `--accent`; `--accent` ≥ 3:1 on `--bg`, `--panel-bg` and `--button-bg`; `--wf-playhead` and `--wf-clip-marker` ≥ 3:1 on `--wf-lane-bg`. Verify `npm test` passes, and that temporarily setting `--text-dim` to `#555555` makes it fail.

## 2. Highlights, focus and shadows

- [ ] 2.1 Add the global teal `:focus-visible` outline (design Decision 3), with an inset offset for tabs in `TabBar.svelte` so the ring isn't clipped. Verify manually: pressing Tab moves a visible teal ring across toolbar buttons, tabs and the volume slider, and clicking with the mouse shows no ring.
- [ ] 2.2 Deepen the shadows in `Notices.svelte` and `ProgressOverlay.svelte` to `rgba(0, 0, 0, 0.45)`, and confirm no other hard-coded colours remain (`grep -rnE "#[0-9a-fA-F]{3,8}|rgba?\(" src --include=*.svelte` should show only those two shadows). Verify manually that a notice and the progress box are visibly lifted off the background.

## 3. Fixed theme in the waveform and window

- [ ] 3.1 Remove the `prefers-color-scheme` listener from `Waveform.svelte` and update the comment in `src/lib/waveform/theme.ts`. Verify `npm run check` passes with no warnings, and manually: switching the OS between light and dark while a file is open changes nothing in the app.
- [ ] 3.2 Set `"theme": "Dark"` on the main window in `src-tauri/tauri.conf.json`. Verify manually on Windows with the OS in light mode: `npm run tauri dev` shows a dark title bar.

## 4. Acceptance

- [ ] 4.1 Add an `app-theme` section to `docs/acceptance.md` with a row per scenario in this change's spec. Update rows that mention light/dark OS themes (B5 empty-screen imprint) to the always-dark behaviour. Run the manual pass with the OS in light mode and then dark mode, recording ✅/❌ per row, and verify every new row passes. Include the selection, playhead and clipped scenarios using `two-clip-runs.wav` and `beep-every-second-mono-60s.wav`.
