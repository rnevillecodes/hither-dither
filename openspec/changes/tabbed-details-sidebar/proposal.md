# Proposal

## Why

The details panel stacks the Effects chain, File metadata and Levels statistics in one long scrolling column. With a few effects in the chain, the level figures are pushed below the fold, and the user has to scroll back and forth between working on plugins and reading metrics. A VS Code–style tabbed sidebar gives each a view of its own, one click away.

## What Changes

- Add an **activity bar**: a narrow vertical strip of icon buttons on the right edge of the window, shown while files are open.
- Split the details panel into two **sidebar views**, each with its own icon in the activity bar:
  - **Metrics** (bar-chart icon): the File metadata and Levels sections.
  - **Effects** (plug icon): the effect chain (VST3 plugins).
- Only one view is shown at a time. Clicking an inactive icon switches to its view; clicking the active icon collapses the sidebar, as in VS Code. Clicking any icon while collapsed opens the sidebar on that view.
- The Effects icon shows a small badge with the number of effects in the active tab's chain.
- Adding an effect from the menu (Effects › Add Effect…) switches the sidebar to the Effects view, opening it if collapsed, so the new effect is visible.
- The chosen view and whether the sidebar is open are remembered across tab switches and restarts. The existing "details panel shown" preference is carried over.
- **BREAKING (UI)**: the toolbar's details-panel toggle button is removed; the activity bar replaces it.

## Capabilities

### New Capabilities
- `details-sidebar`: the activity bar and tabbed sidebar beside the waveform — which views exist, how they are switched, shown and hidden, and what is remembered.

### Modified Capabilities
<!-- None in openspec/specs/. The current details-panel requirements live in the unarchived
     changes add-multi-file-tabs (audio-stats › Details panel visibility) and
     add-vst3-effect-chain (effect-chain › Effects panel); see design.md for how they relate. -->

## Impact

- `src/App.svelte`: the `<aside>` layout becomes activity bar + single-view sidebar; grid columns change.
- New `src/components/ActivityBar.svelte` (icon buttons, badge) and possibly a `Sidebar.svelte` wrapper.
- `src/lib/layout.svelte.ts`: state grows from `detailsOpen` to `{ sidebarOpen, sidebarView }`, with migration from the old localStorage key; new pure helper for the click behaviour, with unit tests.
- `src/components/Toolbar.svelte`: panel toggle button removed.
- `src/lib/editActions.ts` / menu wiring: Add Effect reveals the Effects view.
- No Rust, API or dependency changes. Icons are inline SVG, matching the existing components.
