# Proposal

## Why

The app's look comes from the OS light/dark setting and a generic blue accent, so it doesn't match the new icon (deep tile, teal waveform, amber tips). A single dark slate-grey theme with teal highlights and crisp whitish text gives HitherDither a consistent identity. A dark background also suits waveform inspection.

## What Changes

- **BREAKING** (appearance): the app is always dark and no longer follows the OS light/dark setting. The light palette is removed.
- Backgrounds, panels, toolbar and tab strip use slate greys. Body text is a whitish colour with strong contrast, and secondary text is a lighter grey.
- Teal replaces blue for every highlight: primary buttons, the active tab marker, toggled buttons, progress bars, notice accents, the waveform selection, and the overview window. Keyboard focus rings are teal too.
- The waveform is drawn in a light teal tint so it stays distinct from the teal selection. The playhead stays amber (matching the icon's amber) and clipping stays red, so both remain easy to spot.
- The native window frame and title bar use the dark theme where the platform supports it.

## Capabilities

### New Capabilities
- `app-theme`: the app's single dark colour scheme: which colour plays which role (backgrounds, text, highlights, status colours), minimum contrast, and how it applies to the window frame, focus indicators and the waveform view.

### Modified Capabilities
<!-- None: no main spec defines colours today. -->

## Impact

- `src/styles.css`: one dark palette replaces the light palette and the `prefers-color-scheme` override, with `color-scheme: dark`. A `:focus-visible` rule is added.
- `src/components/Waveform.svelte`: the listener that re-reads canvas colours when the OS theme changes is removed, since the theme no longer changes.
- `src/lib/waveform/theme.ts`: its comment no longer refers to OS themes. The canvas still reads the same CSS variables.
- `src-tauri/tauri.conf.json`: the main window gets `"theme": "Dark"`.
- `src/components/Notices.svelte`, `ProgressOverlay.svelte`: shadows deepened for a dark background.
- New `src/lib/palette.test.ts`: checks that the palette's colour pairs meet the contrast minimums.
- `docs/acceptance.md`: new rows for this change. Rows that mention light/dark OS themes are updated.
- No new dependencies, and no backend or IPC changes.
