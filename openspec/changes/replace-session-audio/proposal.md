# Proposal

## Why

A tab's effect chain and plugin settings are tied to the WAV it was opened from. Two common situations have no way out. One is applying the same processing to a different take, or to a new render of the mix. The other is reopening a session whose WAV has been moved, renamed or re-rendered. Today the only option is to rebuild the chain by hand, and a session whose WAV is missing or changed simply refuses to open.

## What Changes

- **File › Replace Audio…** swaps the active tab's WAV for another one. The tab keeps its effect chain in order, with each effect's settings, bypass switch and Bypass All, and keeps its session file. The edits are dropped, because their positions belong to the old recording, and the undo history starts again at the new audio. If the tab has edits or anything to undo, a confirmation explains this first. The tab is marked unsaved until the session is saved.
- **Effects that don't fit are left out.** A new WAV may have a different channel layout. Effects that can't process it are left out with the same notice a session restore gives, and the rest stay in order.
- **Broken sessions offer to choose another WAV.** When a session's WAV can't be found, or no longer matches the session, a dialog explains why and offers **Choose Another WAV…** besides Cancel.
  - If the chosen WAV has the recorded format and length, the session is restored in full, edits included, as a WAV that was only moved or renamed.
  - Otherwise only the effects are restored, as with Replace Audio.

  Either way the tab is marked unsaved, because its session now points to a different file.
- **Out of scope:**
  - undoing a replacement;
  - mapping or keeping edits on a WAV of a different length;
  - replacing the audio of several tabs at once;
  - choosing the replacement automatically (searching folders for a missing WAV).

Assumptions recorded from the user's answers:
- Replace Audio always drops the edits, whatever the new WAV's length.
- When re-choosing a broken session's WAV, a file with the recorded format and length counts as the same recording, so its edits are kept. This is the "moved or renamed" case. Any other file is treated like Replace Audio.
- Any channel layout is accepted; only the effects that can't process it are left out.

## Capabilities

### New Capabilities
- `replace-audio`: swapping a tab's WAV for another one while keeping its effect chain and session, and what happens to its edits, history, effects and unsaved marker.

### Modified Capabilities
- `session-files`: "Locating the WAV" and "WAV must match the session" offer to choose another WAV instead of only failing.

## Impact

- **Rust (`src-tauri`):**
  - `state.rs`: `replace_audio(id, path, plugins, progress)`. It captures the chain in memory with the same code as a session save, loads the new WAV under the same file id, and rebuilds the chain on it with the restore code (`load_slot`, warnings for effects that don't fit). The history starts at the new audio and is then touched so the tab is unsaved.
  - `restore_session` gains a mode: exact (today's checks; a mismatch is reported as a result, not an error) or relink (keep the edits only if the format and length match; otherwise effects only).
  - `read_session` reports a missing WAV as a result rather than an error.
  - `commands.rs`/`lib.rs`: a `replace_audio` command that stops playback and closes the old effects' editors.
- **Frontend (`src`):**
  - `editActions.ts`/`actions.ts`: Replace Audio (dialog, refusals, confirmation, reloading the tab like a fresh load).
  - The broken-session dialog with Choose Another WAV…, in the open flow.
  - `api.ts` types and a `menu.ts` item.
- **Docs:** `docs/ipc.md`, `README.md`, `docs/acceptance.md`.
