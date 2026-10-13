# Design

## Context

- `src/App.svelte` renders one `<aside id="details-panel">` in a two-column grid (`1fr 340px`) holding `EffectsPanel`, `MetadataPanel` and `StatsPanel` stacked. Below 820px the grid becomes rows and the aside sits under the waveform (`max-height: 40vh`).
- `src/lib/layout.svelte.ts` holds a single `layout.detailsOpen` boolean, persisted to `localStorage["hither-dither.detailsOpen"]` (defaults to shown), toggled by the icon button at the right end of `Toolbar.svelte`.
- Icons are hand-drawn inline SVGs (16×16 viewBox, `stroke: currentColor`), e.g. the panel toggle in `Toolbar.svelte` and the row buttons in `EffectsPanel.svelte`. There is no icon library.
- Effects › Add Effect… (`menu.ts`) sets `effectsUi.browserOpen`; choosing an effect in `EffectBrowser` calls `addEffect()` in `editActions.ts`.
- The current details-panel requirements live in the **unarchived** changes `add-multi-file-tabs` (`audio-stats` › *Details panel visibility*) and `add-vst3-effect-chain` (`effect-chain` › *Effects panel*). They are not yet in `openspec/specs/`, so this change cannot write MODIFIED deltas against them; the new `details-sidebar` capability supersedes the visibility requirement and the "Effects section of the details panel" wording.

## Goals / Non-Goals

**Goals:**
- A VS Code–style activity bar + single-view sidebar with two views: Metrics and Effects.
- Pure, unit-tested state transitions for the click behaviour and persistence migration.
- Keep `EffectsPanel`, `MetadataPanel` and `StatsPanel` unchanged inside; only where they are mounted changes.

**Non-Goals:**
- Resizable sidebar width, drag-to-reorder or hiding activity bar icons.
- Keyboard shortcuts for switching views (e.g. Ctrl+Shift+E); could follow later.
- A Plugins/Plugin Manager view — the Plugin Manager stays a dialog.
- Moving the sidebar to the left side.

## Decisions

**Activity bar on the right edge, sidebar to its left.** The panel is already on the right; keeping it there avoids moving the waveform's origin and mirrors VS Code's "secondary side bar" arrangement (activity bar on the outer edge). Grid becomes `1fr [340px] 44px` — the sidebar column is omitted when collapsed. *Alternative:* left-side activity bar like VS Code's primary sidebar — rejected, it would shift the waveform and break muscle memory.

**State model.** Replace `detailsOpen` with:
```ts
type SidebarView = "metrics" | "effects";
layout = { sidebarOpen: boolean; sidebarView: SidebarView }
```
A pure helper in a new `src/lib/sidebar.ts`:
- `clickView(state, view)` → if open and `view === sidebarView`, collapse; otherwise `{ sidebarOpen: true, sidebarView: view }`.
- `revealView(state, view)` → always `{ sidebarOpen: true, sidebarView: view }`.
- `readSidebarState(get)` → parses stored values; unknown/invalid view → `"metrics"`.

Kept separate from `layout.svelte.ts` (which holds the runes state and storage side effects) so it can be tested with Vitest like `tabOrder.ts` / `chainOrder.ts`.

**Persistence and migration.** New keys `hither-dither.sidebarOpen` and `hither-dither.sidebarView`. If `sidebarOpen` is absent, fall back to the old `hither-dither.detailsOpen` (`"false"` → collapsed), else default open. The old key is left in place (harmless) so a rollback to the previous build still finds it.

**Icons.** Two new 16×16 inline SVGs in the existing stroke style:
- Metrics: a bar chart (three vertical bars of different heights on a baseline) — reads as "levels/statistics".
- Effects: a plug (two prongs, body, cable) — the user calls these "plugins", and a plug is the clearest metaphor for VST3 plug-ins. *Alternatives considered:* sliders/knob (could be mistaken for the generic-controls window), puzzle piece (VS Code's extensions icon — implies installing extensions, which the Plugin Manager does, not the chain).
Active button: accent-coloured icon only. A VS Code–style edge bar was tried and dropped; with two icons the colour alone reads clearly. Badge: small pill in `--accent` with `--accent-text`, bottom-right of the icon.

**Accessibility.** The activity bar is `role="tablist"` with `aria-orientation="vertical"`; each button `role="tab"`, `aria-selected`, `aria-controls="sidebar"`. Because clicking the selected tab collapses, `aria-expanded` is also set on the selected tab. Up/Down arrow keys move focus between the two tabs (Left/Right in the narrow horizontal layout); Enter/Space activate.

**Revealing Effects after Add Effect.** `addEffect()` in `editActions.ts` calls `revealSidebarView("effects")` after a successful add. This covers every route (menu, empty-chain button, and any future one) in one place rather than in the menu handler. *Alternative:* reveal when the browser opens — rejected; cancelling the browser would still have switched views.

**Toolbar toggle removed.** The activity bar is always visible while files are open and does both jobs (show/hide and choose view), so the toolbar button and its `.panel-toggle` styles are deleted. `toggleDetails` is removed.

**Narrow layout (<820px).** Keep the existing breakpoint. Grid rows: waveform `1fr`, activity bar `auto` (horizontal strip, `aria-orientation="horizontal"`), sidebar `auto` with `max-height: 40vh`.

## Risks / Trade-offs

- [Effects become one click further away for users who kept the chain and stats visible together] → The badge shows chain size at a glance, and Add Effect auto-reveals the view.
- [44px activity bar takes waveform width even when collapsed] → Narrower than the 340px panel it replaces when collapsed was 0px; acceptable and matches VS Code. Width kept minimal.
- [Superseded requirements in unarchived changes] → When `add-multi-file-tabs` and `add-vst3-effect-chain` are archived, their "Details panel visibility" and "Effects section in the details panel" wording should be reconciled with `details-sidebar` (archive this change after them, or adjust their deltas).
- [`StatsPanel` keeps computing while hidden] → Stats are computed by the backend regardless of visibility today; no change in cost.

## Migration Plan

Front-end only. On first launch of the new build, the old `detailsOpen` preference seeds `sidebarOpen`. Rollback: the previous build reads its untouched `detailsOpen` key.
