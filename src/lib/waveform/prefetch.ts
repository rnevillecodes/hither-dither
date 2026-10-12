// Lets edits fetch the waveform's next tile before they show their result, so the view
// switches from the old audio to the new in one frame instead of flickering.
import type { Viewport } from "../viewport";

type Prefetch = (vp: Viewport, version: number) => Promise<void>;

/** The mounted waveform's prefetch, by file id. */
const prefetchers = new Map<number, Prefetch>();

/** Called by the waveform showing `fileId` while it is mounted (null on unmount). */
export function setPrefetch(fileId: number, prefetch: Prefetch | null) {
  if (prefetch) prefetchers.set(fileId, prefetch);
  else prefetchers.delete(fileId);
}

/** Longest an edit waits for the tile before showing its result anyway. */
const MAX_WAIT_MS = 250;

/**
 * Fetch the tile `fileId`'s waveform will need for `vp` at peaks version `version`, if it is
 * showing that file. Resolves when it has arrived (or after a short wait at most).
 */
export function prefetchPeaks(fileId: number, vp: Viewport | null, version: number): Promise<void> {
  const prefetch = prefetchers.get(fileId);
  if (!prefetch || !vp) return Promise.resolve();
  const timeout = new Promise<void>((resolve) => setTimeout(resolve, MAX_WAIT_MS));
  return Promise.race([prefetch(vp, version).catch(() => {}), timeout]);
}
