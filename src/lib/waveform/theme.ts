// Canvas colours, read once from the CSS custom properties in styles.css (the fixed dark palette).

export interface Theme {
  laneBg: string;
  laneAltBg: string;
  wave: string;
  zero: string;
  grid: string;
  text: string;
  textDim: string;
  selection: string;
  selectionEdge: string;
  clip: string;
  clipMarker: string;
  playhead: string;
  overviewWave: string;
  overviewWindow: string;
}

const VARS: Record<keyof Theme, string> = {
  laneBg: "--wf-lane-bg",
  laneAltBg: "--wf-lane-alt-bg",
  wave: "--wf-wave",
  zero: "--wf-zero",
  grid: "--wf-grid",
  text: "--text",
  textDim: "--text-dim",
  selection: "--wf-selection",
  selectionEdge: "--wf-selection-edge",
  clip: "--wf-clip",
  clipMarker: "--wf-clip-marker",
  playhead: "--wf-playhead",
  overviewWave: "--wf-overview-wave",
  overviewWindow: "--wf-overview-window",
};

export function readTheme(el: Element): Theme {
  const style = getComputedStyle(el);
  const out = {} as Theme;
  for (const [key, v] of Object.entries(VARS) as [keyof Theme, string][]) {
    out[key] = style.getPropertyValue(v).trim() || "magenta";
  }
  return out;
}
