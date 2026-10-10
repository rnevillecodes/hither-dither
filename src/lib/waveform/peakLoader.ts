// Fetches waveform tiles for the viewport, coalescing requests so only the latest is served.
import { type PeakTile, PEAKS_KIND_SAMPLES, getPeaks } from "../api";
import { type Viewport, visibleEnd } from "../viewport";

/** Tiles extend this many view-widths beyond each side so small scrolls need no fetch. */
const MARGIN_WIDTHS = 1;
const MAX_COLUMNS = 16_384;

interface Request {
  start: number;
  end: number;
  columns: number;
  generation: number;
}

export class PeakLoader {
  tile: PeakTile | null = null;
  private generation = 0;
  private inFlight = false;
  private wanted: Request | null = null;

  constructor(private onTile: () => void) {}

  /** Drop cached data (new file loaded). */
  reset() {
    this.generation++;
    this.tile = null;
    this.wanted = null;
  }

  /** Request a tile for `vp` at `dpr` device pixels per CSS pixel, unless the cached one fits. */
  ensure(vp: Viewport, dpr: number) {
    if (this.covers(vp, dpr)) return;
    const span = vp.width * vp.fpp;
    const margin = span * MARGIN_WIDTHS;
    const start = Math.max(0, Math.floor(vp.start - margin));
    const end = Math.min(vp.totalFrames, Math.ceil(visibleEnd(vp) + margin));
    const columnsPerFrame = dpr / vp.fpp;
    const columns = Math.max(1, Math.min(MAX_COLUMNS, Math.round((end - start) * columnsPerFrame)));
    this.wanted = { start, end, columns, generation: this.generation };
    this.pump(vp.totalFrames);
  }

  private covers(vp: Viewport, dpr: number): boolean {
    const t = this.tile;
    if (!t) return false;
    if (vp.start < t.start || visibleEnd(vp) > t.start + t.span) return false;
    if (t.kind === PEAKS_KIND_SAMPLES) return true;
    const tileFpp = t.span / t.count;
    const wantFpp = vp.fpp / dpr;
    return Math.abs(tileFpp / wantFpp - 1) < 0.02;
  }

  private pump(totalFrames: number) {
    if (this.inFlight || !this.wanted) return;
    const req = this.wanted;
    this.wanted = null;
    this.inFlight = true;
    getPeaks(req.start, req.end, req.columns, totalFrames)
      .then((tile) => {
        if (req.generation !== this.generation) return;
        this.tile = tile;
        this.onTile();
      })
      .catch(() => {
        // The file may have changed under us; the next ensure() retries.
      })
      .finally(() => {
        this.inFlight = false;
        this.pump(totalFrames);
      });
  }
}
