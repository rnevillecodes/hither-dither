// The curve dialog for Edit › Fade In / Fade Out: asks for a bend, resolves null on Cancel.
// While it is open, the fade is drawn over the selection and, if the file is playing, heard.
import { PreviewSender } from "./preview";
import type { Selection } from "./selection";

export const curveDialog = $state({
  open: false,
  dir: "in" as "in" | "out",
  bend: 0,
  /** The selection being faded, for the waveform's preview. */
  range: null as Selection | null,
  /** The fade applied, drawn until the edit that makes it is shown. */
  applied: null as { start: number; end: number; dir: "in" | "out"; bend: number } | null,
  /** The fade the waveform draws over the audio: the one in the dialog, or just applied. */
  get shown() {
    if (this.open && this.range) return { ...this.range, dir: this.dir, bend: this.bend };
    return this.applied;
  },
});

let resolver: ((bend: number | null) => void) | null = null;
let sender: PreviewSender | null = null;

/** Show the dialog for a fade in `dir` of `range` in file `fileId`, starting linear. */
export function askFadeCurve(dir: "in" | "out", fileId: number, range: Selection): Promise<number | null> {
  resolver?.(null);
  curveDialog.dir = dir;
  curveDialog.bend = 0;
  curveDialog.range = range;
  curveDialog.open = true;
  sender = new PreviewSender(fileId);
  return new Promise((resolve) => (resolver = resolve));
}

/** The bend changed: hear it (if playing). */
export function previewCurve() {
  const range = curveDialog.range;
  if (!sender || !range) return;
  sender.send({
    kind: "fade",
    op: curveDialog.dir === "in" ? "fadeIn" : "fadeOut",
    start: range.start,
    end: range.end,
    bend: curveDialog.bend,
  });
}

/** Close the dialog with the chosen bend, or null to cancel (going back to what was heard). */
export function closeCurveDialog(bend: number | null) {
  const range = curveDialog.range;
  curveDialog.applied = bend !== null && range ? { ...range, dir: curveDialog.dir, bend } : null;
  curveDialog.open = false;
  curveDialog.range = null;
  const r = resolver;
  resolver = null;
  const s = sender;
  sender = null;
  // Cancel goes back to the file as it is; Apply commits once the last preview has landed,
  // so the commit is what playback follows last.
  if (!s) return r?.(bend);
  const landed = bend === null ? s.cancel() : s.settle();
  void landed.then(() => r?.(bend));
}
