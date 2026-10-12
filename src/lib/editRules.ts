// Pure rules for the selection, playhead and view after an edit, undo or redo.
import type { EditOp } from "./api";
import type { Selection } from "./selection";

export type EditKind = EditOp | "undo" | "redo";

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
