// Pure rules for the selection, playhead and view after an edit, undo or redo.
import type { ClipInfo, EditOp } from "./api";
import type { Selection } from "./selection";

/** An edit, undo/redo, or a clip-handle change (trim, end fades, crossfade). */
export type EditKind = EditOp | "undo" | "redo" | "clip";

export interface AfterEdit {
  selection: Selection | null;
  playhead: number;
  /** Zoom to fit the new audio instead of keeping the current zoom and scroll. */
  fit: boolean;
}

/**
 * Where things stand after `kind` changed the audio from `oldFrames` to `newFrames`
 * frames, given the selection it acted on (edits) or the current one (undo/redo).
 */
export function afterEdit(
  kind: EditKind,
  selection: Selection | null,
  playhead: number,
  oldFrames: number,
  newFrames: number,
): AfterEdit {
  const clampFrame = (f: number) => Math.max(0, Math.min(f, newFrames));
  switch (kind) {
    case "crop":
      return { selection: null, playhead: 0, fit: true };
    case "delete":
      // The playhead goes to the join.
      return { selection: null, playhead: clampFrame(selection?.start ?? playhead), fit: false };
    case "silence":
    case "fadeIn":
    case "fadeOut":
      return { selection, playhead: clampFrame(playhead), fit: false };
    case "undo":
    case "redo":
    case "clip":
      return {
        selection: newFrames === oldFrames ? selection : null,
        playhead: clampFrame(playhead),
        fit: false,
      };
  }
}

/** Whether `op` can be applied to `selection` in a file of `frames` frames. */
export function canApply(op: EditOp, selection: Selection | null, frames: number): boolean {
  if (!selection || selection.end <= selection.start) return false;
  const whole = selection.start <= 0 && selection.end >= frames;
  // Cropping to the whole file changes nothing; deleting it would leave no audio.
  if ((op === "crop" || op === "delete") && whole) return false;
  return true;
}

/**
 * Where frame `frame` of the audio before `kind` is afterwards: the same audio if it is
 * still there, else the join that replaced it (or the nearest end). `selection` is the
 * range the edit acted on; `oldClip`/`newClip` locate trim changes.
 */
export function remapFrame(
  kind: EditKind,
  selection: Selection | null,
  frame: number,
  newFrames: number,
  oldClip: ClipInfo | null,
  newClip: ClipInfo | null,
): number {
  let f = frame;
  if ((kind === "crop" || kind === "delete") && selection) {
    const { start, end } = selection;
    if (kind === "crop") f = f < start ? 0 : f - start;
    else if (f >= end) f -= end - start;
    else if (f > start) f = start;
  } else if ((kind === "clip" || kind === "undo" || kind === "redo") && oldClip && newClip) {
    // A trim moves the start; other changes keep positions (beyond that, clamp).
    if (oldClip.untrimmed_frames === newClip.untrimmed_frames) f += oldClip.trim_start - newClip.trim_start;
  }
  return Math.max(0, Math.min(f, newFrames));
}
