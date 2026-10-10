# Design

## Context

Single-file state is built into both halves of the app today:

- **Backend** (`src-tauri/src/state.rs`): `AppState` holds one `RwLock<Option<Arc<LoadedFile>>>` and one `loading` cancel token, so any new `open` cancels the one in progress. Every file command (`get_metadata`, `get_peaks`, `compute_stats`, `get_clip_runs`, `play`) reads `state.current()`. `open_file` stops the player when it succeeds.
- **Frontend**: `app` (`src/lib/app.svelte.ts`) is a single `$state` object (`meta`, `clipRuns`, `wholeStats`, `selection`, `stats`, `statsPending`, `loading`, `generation`). `view.vp` (`view.svelte.ts`) and `transport.playhead/playStart` (`transport.svelte.ts`) are globals. `Waveform.svelte` resets its `PeakLoader`, overview tile and viewport whenever `app.generation` changes. `actions.ts` opens with `multiple: false` and `dropPaths` keeps only the first path.
- **Menu**: `menu.ts` builds File (Open…, Quit) and View menus. Ctrl/Cmd+O is handled both by the menu accelerator and by the webview keydown handler, and the `dialogOpen` flag stops it running twice.
- **Icons**: `src-tauri/icons/*` and `public/favicon.png` are the stock Tauri logo. `tauri.conf.json` lists the bundle icons.

See proposal.md for why this change is needed.

## Goals / Non-Goals

**Goals:**
- Make the backend hold any number of loaded files, each addressed by an ID. Remove the "newer open cancels older" rule.
- Keep each tab's state (viewport, selection, stats, playhead, overview tile) on the frontend, and make tab switches instant: no backend recomputation or re-analysis.
- Keep the change to existing components small by keeping the `app` / `view` / `transport` access patterns they already use.
- Produce one source icon and generate every platform variant from it.

**Non-Goals:**
- Reordering tabs by drag, split/side-by-side views, or comparing two files in one view.
- Restoring open tabs when the app restarts.
- Playing more than one file at a time.
- A limit on the number of open tabs (see Risks).

## Decisions

### 1. Backend: a map of loaded files keyed by a frontend-assigned ID
`AppState.file` becomes `files: RwLock<HashMap<FileId, Arc<LoadedFile>>>` and `loading` becomes `Mutex<HashMap<FileId, Arc<AtomicBool>>>`, with `type FileId = u32`. `open(id, path, progress)` installs the file under `id` and no longer cancels other loads. New methods: `get(id)`, `close(id)` (removes the file and cancels its load if one is running) and `cancel_open(id)`. All file commands take a `fileId` argument. New commands `close_file(fileId)` and `cancel_open(fileId)` are added. `get_metadata` returns metadata for a `fileId`.
- *Why the frontend assigns the ID:* the tab, and its ID, has to exist before the load finishes so that closing a loading tab can cancel it. A frontend counter avoids an extra "reserve ID" round trip.
- *Alternative:* key files by path. Rejected: paths are long, and it couples identity to how paths are normalised. The duplicate-file check (Decision 4) is a UI rule, not a storage key.
- `StatsRunner` stays global. Only the active tab ever asks for selection stats, and a new request already cancels the previous one.
- The `Player` stays a single instance. `play` takes `fileId`. `close_file` stops the player first if it is playing that file's source (compared with `Arc::ptr_eq`), so the mmap is released.

### 2. Frontend: tab store plus an "active document" facade
A new `src/lib/tabs.svelte.ts` owns `tabs: Tab[]` and `activeId`. A `Tab` holds `id`, `path`, `name`, `status: "loading" | "ready"`, load progress, and the per-file fields currently in `app` (`meta`, `clipRuns`, `wholeStats`, `selection`, `stats`, `statsPending`), plus saved view state (`vp`, `userScrolled`, `playhead`, `playStart`, `overviewTile`).

`app` keeps its shape, but each per-file field becomes a getter/setter that forwards to the active tab. `App.svelte`, `Toolbar.svelte`, `StatsPanel`, `Transport` and `Waveform` can then keep reading `app.meta`, `app.selection` and so on.

`view.vp` and the transport playhead stay global "live" values. `activateTab(id)` saves them into the outgoing tab and restores them from the incoming one.
- *Why:* every component today reads these globals. A facade plus save/restore on switch confines the change to the tab store and a few call sites, instead of threading a tab through every component.
- *Alternative:* pass a `doc` prop down to every component. Cleaner in theory, but it touches every component and the transport/view modules for no behavioural gain.

### 3. Waveform remounts per tab
`App.svelte` wraps `<Waveform>` in `{#key tabs.activeId}`. A fresh mount gets a fresh `PeakLoader`. On mount it uses the tab's saved `vp` if there is one (resized to the current width), otherwise it fits the whole file. It reuses the tab's cached `overviewTile` instead of fetching it again. The `app.generation` reset effect is replaced by this mount logic. Stale-fetch guards use a component-local token.
- *Why:* remounting is the simplest way to guarantee no peak tile or overview from one file is drawn for another. Peak tiles are viewport-sized, so the first fetch after a switch is cheap.

### 4. Opening: multi-select, a sequential load queue, duplicate check
`openDialog()` calls `open({ multiple: true, ... })`. Both the dialog and `dropPaths` call `openPaths(paths)`. For each path, `openPaths`:
1. activates the existing tab and skips the path if a tab already has that path. Paths are compared after normalising separators, and case-insensitively on Windows.
2. otherwise creates a `loading` tab right away.
3. queues the load.

Loads run one at a time, in order. Each finished tab becomes active, so the last successful file ends up active. A failed load removes its tab and shows the existing error notice. The other files carry on.
- *Why sequential:* parallel analysis of several large files competes for disk I/O, and none finishes sooner. A queue also keeps the progress display simple.
- The full-window `ProgressOverlay` only shows when the active tab is the one loading (and the 500 ms delay has passed). Tabs that are loading in the background show a small spinner in the tab instead, so the user can work in other tabs (spec: Loading tabs).
- The "Only one file can be opened at a time" notice is removed.

### 5. Closing and shortcuts
`closeTab(id)` does the following:
1. stops playback if that tab is active and playing.
2. calls `cancel_open` if the tab is loading, otherwise `close_file`.
3. removes the tab and activates the right-hand neighbour, or the left one if the tab was last.

File menu: `Open…` (CmdOrCtrl+O), `Close Tab` (CmdOrCtrl+W, disabled when no tabs), separator, `Quit`. Ctrl+W is handled both by the menu accelerator and in the webview keydown handler, so `closeActiveTab()` ignores a second call that arrives within 100 ms. Without that guard, one key press could close two tabs; the existing `dialogOpen` flag solves the same problem for Open. Ctrl+Tab and Ctrl+Shift+Tab are handled in the webview keydown handler only.

### 6. Tab strip component
New `src/components/TabBar.svelte` sits between `Toolbar` and `<main>`. It uses `role="tablist"`, and each tab is a `role="tab"` button with `aria-selected`. Tabs are clicked to activate, and `auxclick` with button 1 (middle-click) closes them. Each tab has a close "×" button with `aria-label="Close <name>"` and a `title` showing the full path. When there are too many tabs to fit, the strip scrolls horizontally and keeps the active tab in view. It's styled with the existing CSS variables (`--panel-bg`, `--border`, `--accent`). `Toolbar.svelte` no longer has an Open… button: the empty screen has one, and File › Open… or drag-and-drop work at any time.

### 7. App icon: one SVG source, generated with `tauri icon`
The source is a hand-authored `src-tauri/icons/app-icon.svg` (1024×1024):
- a rounded-square tile with a deep indigo→navy gradient, so it stands out on both light and dark taskbars.
- a bold, symmetric waveform of 5–7 thick rounded vertical bars in a teal→amber gradient. These colours carry over from the current palette.
- the bars on the right side break up into an ordered-dither dot pattern, as a visual pun on the app's name.

The bars are sized so the waveform still reads at 16 px, where the dots blur into a lighter tone. All variants are generated with `npx tauri icon src-tauri/icons/app-icon.svg`, which is already a dev dependency. That command can also write Android/iOS icon folders, which are deleted because those platforms aren't targets. `public/favicon.png` is replaced with the generated 128×128 PNG. A preview sheet (16/32/128/512 on light and dark backgrounds) is shown to the user for sign-off before committing.
- *Why SVG:* it is easy to review and edit as text, and gives crisp output at every size.
- *Alternative:* a raster drawing from an external tool. It's harder to iterate on in the repo, and there's no source to regenerate from.
- *Shadow variant:* `scripts/gen-icons.mjs` builds both SVGs from one set of shapes: `src-tauri/icons/app-icon.svg` (full colour) and `src/assets/logo-shadow.svg`. The shadow variant is an imprint of the icon's details: the waveform bars and dither dots in a single colour, with no tile, cropped to the waveform's bounds. The empty screen uses it as a CSS `mask-image` (with `-webkit-mask-image` for WebKit) on an element filled with `var(--text)` at low opacity, so it follows the light/dark theme without a second asset. It is about `min(320px, 40vh)` wide, keeps the waveform's proportions, and is `aria-hidden`.

### 8. Details panel toggle
A new `src/lib/layout.svelte.ts` holds `layout.detailsOpen` (default `true`), saved to `localStorage` (key `hither-dither.detailsOpen`) on change and read at startup. Every `localStorage` access is wrapped in `try`/`catch`, so a blocked store just falls back to the default. `App.svelte` renders `<aside>` only when `tabs.list.length > 0 && layout.detailsOpen`, and switches `main` to a single grid column otherwise. The waveform's `ResizeObserver` already re-lays it out to the new width. The toggle is an icon button (a window with a right-hand panel) at the right end of the toolbar, shown only while tabs exist. It has `aria-pressed`, `aria-controls="details-panel"`, and a title of "Show details panel" or "Hide details panel". On narrow windows, where the panel sits below the waveform, the same toggle hides it there.
- *Why one global setting:* the panel is a layout preference, not a per-file view state. A per-tab setting would make the waveform width jump when switching tabs.
- *Alternative:* a View menu item. That can be added later; a visible button is easier to discover.

## Risks / Trade-offs

- [Many large files open at once] → Each tab holds a read-only mmap (virtual memory, paged in as needed) and a peak pyramid under 1% of the file size, so ten 2 GB files cost roughly 200 MB of resident pyramid data. No hard tab limit; this may need revisiting if users open dozens of huge files.
- [Windows locks mmapped files against deletion or rename] → This already applies to the single open file. With tabs it lasts until the tab is closed. `close_file` drops the `Arc<LoadedFile>`, and the player's `Arc<SampleSource>` once playback stops, so the lock is released.
- [Duplicate detection by path string misses symlinks or 8.3 short names] → Accepted. The worst case is the same file open in two tabs, which is harmless.
- [Ctrl+Tab may be intercepted by some webviews or window managers] → Clicking tabs always works. The shortcut is a convenience.
- [Facade getters hide which tab a write goes to] → Writes only happen through user actions on the active tab, and async results (selection stats, loads) carry their tab ID and write to that tab directly, never through the facade.
- [Archive order] → The `wav-import` delta modifies requirements that exist only in `add-wav-analyser-app`. That change must be archived first.

## Migration Plan

There is no persisted data to migrate. The IPC changes are internal (frontend and backend ship together). `docs/ipc.md` is updated to match. Rollback means reverting the branch.

## Open Questions

- The exact icon colours and bar count can be tuned during the icon review, within the constraints in Decision 7.
