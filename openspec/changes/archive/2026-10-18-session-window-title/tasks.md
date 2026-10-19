# Tasks

## 1. Title

- [x] 1.1 Add `src/lib/windowTitle.ts` with a pure `windowTitle(tab)` that takes the active tab (or null) and returns the title by the window-title spec: the session name without `.hdsession.json` (matched ignoring case, from a Windows or POSIX path), " •" when dirty, and "HitherDither" otherwise, including while loading. Add `src/lib/windowTitle.test.ts` covering "Saved session", "Never saved", "No tabs", "Change after saving", "Save As", a loading tab and both path styles. Verify `npm test` passes.
- [x] 1.2 Add `core:window:allow-set-title` to `src-tauri/capabilities/default.json`, and an `$effect` in `src/App.svelte` that calls `getCurrentWindow().setTitle(windowTitle(activeTab()))` whenever the title changes. Verify `npm run check` passes, then check every window-title scenario by hand in `npm run tauri dev`, including the taskbar or window switcher on the platform used.

## 2. Docs

- [x] 2.1 Add a `window-title` section to `docs/acceptance.md` with one row per scenario, naming `windowTitle.test.ts` where it covers one, and mention the title in the README's session files section. Verify `openspec validate session-window-title --strict` passes.
