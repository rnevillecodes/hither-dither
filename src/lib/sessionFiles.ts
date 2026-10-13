// Session file names (`*.hdsession.json`), matched the same way as in `src-tauri/src/session.rs`,
// and planning which tabs a mix of WAV and session paths opens.
import { samePath } from "./tabOrder";

export const SESSION_EXTENSION = ".hdsession.json";

/** A path to open, with its session read (or failed to read) when it is one. */
export type OpenRequest =
  | { kind: "wav"; path: string }
  /** `relink`: `audioPath` is a WAV the user chose instead of the session's own. */
  | { kind: "session"; sessionPath: string; audioPath: string; relink: boolean }
  | { kind: "invalid"; sessionPath: string; message: string };

export type OpenAction =
  /** Open `path` in a new tab, restoring `sessionPath` once it has loaded. */
  | { kind: "open"; path: string; sessionPath: string | null; relink: boolean }
  /** `path` is already open (or opening): activate its tab. A session isn't applied to it. */
  | { kind: "activate"; path: string; sessionPath: string | null }
  | { kind: "error"; sessionPath: string; message: string };

/**
 * What to do for each request, in the order requested. A WAV that is already open, or
 * opened earlier in the same batch, activates its tab instead.
 */
export function planOpens(requests: readonly OpenRequest[], isOpen: (path: string) => boolean): OpenAction[] {
  const opening: string[] = [];
  return requests.map((r): OpenAction => {
    if (r.kind === "invalid") return { kind: "error", sessionPath: r.sessionPath, message: r.message };
    const [path, sessionPath, relink] = r.kind === "wav" ? [r.path, null, false] : [r.audioPath, r.sessionPath, r.relink];
    if (isOpen(path) || opening.some((p) => samePath(p, path))) return { kind: "activate", path, sessionPath };
    opening.push(path);
    return { kind: "open", path, sessionPath, relink };
  });
}

function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/** Whether `path` names a session file: its name ends in `.hdsession.json`, ignoring case. */
export function isSessionPath(path: string): boolean {
  const name = baseName(path).toLowerCase();
  return name.length > SESSION_EXTENSION.length && name.endsWith(SESSION_EXTENSION);
}

/**
 * Where Save Session As proposes saving: the tab's session file if it has one, else next
 * to its WAV with the same name (`take.wav` → `take.hdsession.json`).
 */
export function defaultSessionPath(tab: { path: string; sessionPath: string | null }): string {
  if (tab.sessionPath) return tab.sessionPath;
  return tab.path.replace(/\.wave?$/i, "") + SESSION_EXTENSION;
}

/**
 * The close prompt's question. Saving writes the session, never the WAV, so it names the
 * session file, or says a session will be saved when the tab has none yet.
 */
export function closePrompt(tab: { name: string; sessionPath: string | null }): string {
  if (tab.sessionPath) return `Save changes to the session “${baseName(tab.sessionPath)}” before closing?`;
  return `Save a session for “${tab.name}” before closing? The WAV itself isn't changed.`;
}

/** Where Export proposes writing: next to the WAV, with `-export` (`take.wav` → `take-export.wav`). */
export function defaultExportPath(wavPath: string): string {
  return wavPath.replace(/\.wave?$/i, "") + "-export.wav";
}

/**
 * `path` with the full session extension. Native dialogs handle a two-part extension
 * differently, so a name typed as `mix` or `mix.json` becomes `mix.hdsession.json`.
 */
export function withSessionExtension(path: string): string {
  if (isSessionPath(path)) return path;
  return path.replace(/(\.hdsession)?(\.json)?$/i, "") + SESSION_EXTENSION;
}
