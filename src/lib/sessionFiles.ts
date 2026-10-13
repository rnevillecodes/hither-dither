// Session file names (`*.hdsession.json`), matched the same way as in `src-tauri/src/session.rs`,
// and planning which tabs a mix of WAV and session paths opens.
import { samePath } from "./tabOrder";

export const SESSION_EXTENSION = ".hdsession.json";

/** A path to open, with its session read (or failed to read) when it is one. */
export type OpenRequest =
  | { kind: "wav"; path: string }
  | { kind: "session"; sessionPath: string; audioPath: string }
  | { kind: "invalid"; sessionPath: string; message: string };

export type OpenAction =
  /** Open `path` in a new tab, restoring `sessionPath` once it has loaded. */
  | { kind: "open"; path: string; sessionPath: string | null }
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
    const [path, sessionPath] = r.kind === "wav" ? [r.path, null] : [r.audioPath, r.sessionPath];
    if (isOpen(path) || opening.some((p) => samePath(p, path))) return { kind: "activate", path, sessionPath };
    opening.push(path);
    return { kind: "open", path, sessionPath };
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
 * `path` with the full session extension. Native dialogs handle a two-part extension
 * differently, so a name typed as `mix` or `mix.json` becomes `mix.hdsession.json`.
 */
export function withSessionExtension(path: string): string {
  if (isSessionPath(path)) return path;
  return path.replace(/(\.hdsession)?(\.json)?$/i, "") + SESSION_EXTENSION;
}
