# Design

## Context

All UI colours are CSS custom properties in `src/styles.css`. A light palette sits on `:root`, a dark override sits under `@media (prefers-color-scheme: dark)`, and `color-scheme: light dark` is set. Components use only these variables (`--accent`, `--text`, `--text-dim`, `--panel-bg`, …), apart from two hard-coded `rgba(0,0,0,0.18)` shadows in `Notices.svelte` and `ProgressOverlay.svelte` and one `color-mix` in `App.svelte` that already uses `var(--bg)`. The waveform canvas reads its colours from the `--wf-*` variables through `readTheme()` in `src/lib/waveform/theme.ts`. `Waveform.svelte` re-reads them when the `prefers-color-scheme` media query changes. The window in `src-tauri/tauri.conf.json` doesn't set a theme, so the title bar follows the OS.

See proposal.md for why.

## Goals / Non-Goals

**Goals:**
- One palette, with every colour still defined once as a CSS variable. Components and the canvas keep reading the same variable names.
- Contrast minimums from the spec checked by an automated test, not just by eye.

**Non-Goals:**
- A user-selectable theme or a light mode toggle.
- Recolouring the app icon or the empty-screen imprint. The imprint already uses `var(--text)`, so it simply becomes a faint whitish imprint.
- Changing layout, spacing or typography.

## Decisions

### 1. Single dark palette in `:root`; no media query
`:root` holds the dark palette directly and sets `color-scheme: dark`, so native scrollbars, the range slider and form controls render dark. It also sets `accent-color: var(--accent)`, so the volume slider's filled track and thumb are teal instead of the browser's default blue. The `@media (prefers-color-scheme: dark)` block is deleted. Variable names stay the same, so no component changes are needed for colour.
- *Alternative:* keep both palettes and force dark with a `data-theme` attribute. That leaves a light palette nobody uses or tests. It can be reintroduced if a light mode is ever wanted.

### 2. Palette values
These are the starting values. They can be tuned during implementation within the contrast minimums, which the test in Decision 5 checks.

| Role | Variable(s) | Value |
|---|---|---|
| Window background | `--bg` | `#1c222a` |
| Raised surface (panels, toolbar, active tab) | `--panel-bg` | `#252c35` |
| Button / hover | `--button-bg` / `--button-hover` | `#2e3640` / `#38414d` |
| Border | `--border` | `#3b4552` |
| Primary text | `--text` | `#eef2f6` |
| Dimmed text | `--text-dim` | `#a8b3bf` |
| Teal highlight | `--accent` | `#2dd4bf` |
| Text on teal | `--accent-text` | `#062b27` |
| Danger / warning | `--danger` / `--warning` | `#ff7b7b` / `#f5b041` |
| Scrollbar thumb / hover | `--scroll-thumb` / `--scroll-thumb-hover` | `#4a5561` / `#5c6875` |
| Lanes | `--wf-lane-bg` / `--wf-lane-alt-bg` | `#181d24` / `#1d232b` |
| Waveform | `--wf-wave` | `#9ee6dc` (light teal tint) |
| Zero line / grid | `--wf-zero` / `--wf-grid` | `#3d4752` / `#2a313a` |
| Selection band / edge | `--wf-selection` / `--wf-selection-edge` | `rgba(45, 212, 191, 0.16)` / `rgba(45, 212, 191, 0.8)` |
| Clip region / marker | `--wf-clip` / `--wf-clip-marker` | `rgba(255, 107, 107, 0.2)` / `#ff6b6b` |
| Playhead | `--wf-playhead` | `#fbbf24` (amber) |
| Overview wave / window | `--wf-overview-wave` / `--wf-overview-window` | `#4f8f87` / `rgba(45, 212, 191, 0.18)` |

Measured contrast (WCAG): primary text is 9.2–14.2:1 on all surfaces, dimmed text is 5.7–7.5:1, text on teal is 8.2:1, and teal on surfaces is 6.6–8.6:1. On the lanes, the playhead is 10.1:1, clip markers 6.1:1 and the waveform 11.9:1.
- *Why a light teal waveform rather than full teal:* a full-teal waveform under a teal selection band would blur together. The tint keeps the waveform the brightest element, and the band reads as a tint behind it.
- *Why the playhead stays amber:* it is the one moving element, and a warm colour separates it from the teal selection and waveform. It also echoes the amber end of the icon's gradient.

### 3. Teal focus ring
A global `:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }` rule goes in `styles.css`. Elements that already clip their outline (tabs inside the scrolling strip) get `outline-offset: -2px` so the ring stays visible. Only `:focus-visible` is styled, so mouse clicks don't show the ring.

### 4. Fixed theme in the waveform and window
Remove the `matchMedia("(prefers-color-scheme: dark)")` listener in `Waveform.svelte`. The canvas reads the theme once on mount, which is all a fixed palette needs. Update the comment in `theme.ts`. In `tauri.conf.json`, set `"theme": "Dark"` on the main window so Windows and macOS draw a dark title bar. On Linux the window manager decides, which the spec allows ("where the platform allows").

### 5. Contrast test
Add `src/lib/palette.test.ts`. It reads `src/styles.css` as text, extracts the `:root` hex variables, and checks each required foreground/background pair from the spec against its minimum (7:1, 4.5:1 or 3:1) using the WCAG relative-luminance formula. If a later palette tweak breaks the spec's contrast rules, `npm test` fails.
- *Alternative:* manual checks with a contrast tool. Those don't catch regressions.

### 6. Shadows
Change the two `rgba(0, 0, 0, 0.18)` drop shadows to `rgba(0, 0, 0, 0.45)`. At 0.18, black shadows are barely visible on a dark background.

## Risks / Trade-offs

- [Users who prefer a light UI lose it] → This is the user's explicit choice for the product. Decision 1 notes how a light mode could come back.
- [Linux title bars may stay light] → Covered by the spec wording. No mitigation needed.
- [Branch overlap] → The change is limited to colours and one config key. It touches `Waveform.svelte` only to remove the listener, so conflicts with other work should be small.
