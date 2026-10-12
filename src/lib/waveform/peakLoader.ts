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
  version: number;
}

export class PeakLoader {
  /** The latest tile, kept (and drawn) after it goes stale until a fresh one arrives. */
  tile: PeakTile | null = null;
  private tileVersion = 0;
  private version = 0;
  private inFlight = false;
  private wanted: Request | null = null;

  /** One loader per mounted waveform, so it only ever serves `fileId`. */
  constructor(
    private fileId: number,
    private onTile: () => void,
  ) {}

  /** The audio changed (or its analysis finished): tiles fetched before are stale. */
  setVersion(version: number) {
    this.version = version;
  }

  /**
   * Fetch the tile `vp` will need once the audio is at peaks version `version`, ahead of
   * showing it. Resolves when it is in place (or failed).
   */
  async prefetch(vp: Viewport, dpr: number, version: number): Promise<void> {
    const req = { ...this.request(vp, dpr), version };
    const tile = await getPeaks(this.fileId, req.start, req.end, req.columns, vp.totalFrames);
    if (version < this.tileVersion) return;
    this.tile = tile;
    this.tileVersion = version;
  }

  /** Request a tile for `vp` at `dpr` device pixels per CSS pixel, unless the cached one fits. */
  ensure(vp: Viewport, dpr: number) {
    if (this.covers(vp, dpr)) return;
    this.wanted = { ...this.request(vp, dpr), version: this.version };
    this.pump(vp.totalFrames);
  }

  /** The frames and columns of a tile for `vp`, with a margin either side. */
  private request(vp: Viewport, dpr: number) {
    const span = vp.width * vp.fpp;
    const margin = span * MARGIN_WIDTHS;
    const start = Math.max(0, Math.floor(vp.start - margin));
    const end = Math.min(vp.totalFrames, Math.ceil(visibleEnd(vp) + margin));
    const columnsPerFrame = dpr / vp.fpp;
    const columns = Math.max(1, Math.min(MAX_COLUMNS, Math.round((end - start) * columnsPerFrame)));
    return { start, end, columns };
  }

  private covers(vp: Viewport, dpr: number): boolean {
    const t = this.tile;
    if (!t || this.tileVersion !== this.version) return false;
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
    getPeaks(this.fileId, req.start, req.end, req.columns, totalFrames)
      .then((tile) => {
        // A prefetch for newer audio may have landed meanwhile.
        if (req.version < this.tileVersion) return;
        this.tile = tile;
        this.tileVersion = req.version;
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
