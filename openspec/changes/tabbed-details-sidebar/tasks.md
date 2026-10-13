# Tasks

## 1. Sidebar state

- [x] 1.1 Add `src/lib/sidebar.ts` with the `SidebarView` type (`"metrics" | "effects"`), `clickView`, `revealView` and `readSidebarState` (reads `sidebarOpen`/`sidebarView`, falls back to the old `detailsOpen` key, defaults to open on Metrics, unknown view → Metrics). Add `src/lib/sidebar.test.ts` covering: switch view, click active collapses, click while collapsed opens that view, reveal always opens, first launch, old `detailsOpen = "false"` → collapsed, invalid stored view. Verify `npm test` passes.
- [x] 1.2 Rework `src/lib/layout.svelte.ts`: replace `detailsOpen`/`toggleDetails` with `layout.sidebarOpen`, `layout.sidebarView`, `clickSidebarView(view)` and `revealSidebarView(view)`, each persisting to the new localStorage keys inside try/catch (old key left untouched). Verify `npm run check` reports the remaining `toggleDetails`/`detailsOpen` uses, to be fixed in group 2.

## 2. Activity bar and sidebar UI

- [x] 2.1 Add `src/components/ActivityBar.svelte`: a `role="tablist"` of two buttons with inline 16×16 stroke SVG icons (bar chart for Metrics, plug for Effects), tooltips/`aria-label`s, `aria-selected`/`aria-expanded`/`aria-controls="sidebar"`, accent highlight on the shown view's icon, arrow-key focus movement, and an effect-count badge on the Effects button (hidden at 0). Verify `npm run check` passes.
- [ ] 2.2 Update `src/App.svelte`: replace the stacked `<aside id="details-panel">` with the sidebar (`id="sidebar"`, a heading naming the view, then either `MetadataPanel` + `StatsPanel` or `EffectsPanel`) and the `ActivityBar`; grid `1fr 340px 44px` when open, `1fr 44px` when collapsed, nothing while no tabs are open; keep the "File details and level statistics appear here." placeholder for a tab that is still loading. Verify `npm run check` passes and, by hand, the spec scenarios "No files open", "Metrics view contents", "Effects view contents", "Switch view", "Collapse by clicking the active button" and "Reopen from collapsed".
- [ ] 2.3 Narrow layout (<820px) in `App.svelte`/`ActivityBar.svelte`: rows waveform / horizontal activity bar / sidebar (`max-height: 40vh`), `aria-orientation="horizontal"`, Left/Right arrows. Verify the "Narrow window" scenario by hand.
- [x] 2.4 Remove the details-panel toggle button and its `.panel-toggle` styles from `src/components/Toolbar.svelte`. Verify `npm run check` passes with no references to `toggleDetails` or `detailsOpen` left in `src/`.

## 3. Reveal Effects on add

- [ ] 3.1 In `src/lib/editActions.ts`, call `revealSidebarView("effects")` after `addEffect()` succeeds. Verify `npm run check` and `npm test` pass, and by hand the scenarios "Add Effect from the menu while collapsed" and "Add Effect from the menu while Metrics is shown"; cancelling the effect browser leaves the view unchanged.

## 4. Documentation and integration checks

- [x] 4.1 Update `README.md` (Effects paragraph: "from the **Effects** view of the sidebar (plug icon)") and `docs/acceptance.md`: replace rows D1–D4 with a `details-sidebar` section covering every scenario in `specs/details-sidebar/spec.md`, update C5 (activity bar highlight instead of the panel toggle) and FX1 (Effects view instead of "top of the details panel"), and list this change in the intro paragraph. Verify every spec scenario has a row.
- [ ] 4.2 Run the app (`npm run tauri dev`) and walk the new `details-sidebar` acceptance rows, including "Playback continues while switching", "Badge follows the active tab", the restart scenarios and keyboard operation. Verify each passes or record its failure as a follow-up task here.
- [x] 4.3 Run `npm run check`, `npm test` and `openspec validate tabbed-details-sidebar --strict`; verify all pass.

## Workflow follow-up

- Open a pull request from `feat/tabbed-sidebar` once the tasks are complete.
- Archive after `add-multi-file-tabs` and `add-vst3-effect-chain`, then reconcile their "Details panel visibility" and "Effects panel" wording with `details-sidebar`.
