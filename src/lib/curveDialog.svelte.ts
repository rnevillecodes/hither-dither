// The curve dialog for Edit › Fade In / Fade Out: asks for a bend, resolves null on Cancel.

export const curveDialog = $state({
  open: false,
  dir: "in" as "in" | "out",
  bend: 0,
});

let resolver: ((bend: number | null) => void) | null = null;

/** Show the dialog for a fade in `dir`, starting linear. */
export function askFadeCurve(dir: "in" | "out"): Promise<number | null> {
  resolver?.(null);
  curveDialog.dir = dir;
  curveDialog.bend = 0;
  curveDialog.open = true;
  return new Promise((resolve) => (resolver = resolve));
}

/** Close the dialog with the chosen bend, or null to cancel. */
export function closeCurveDialog(bend: number | null) {
  curveDialog.open = false;
  const r = resolver;
  resolver = null;
  r?.(bend);
}
