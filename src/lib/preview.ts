// Lets playback follow a setting while it is being tried out (a handle drag, or the fade
// curve dialog): one call in flight at a time, and only the latest waiting one is sent.
import * as api from "./api";
import { tabs } from "./tabs.svelte";
import { transport } from "./transport.svelte";

export class PreviewSender {
  private inFlight: Promise<void> | null = null;
  private waiting: api.Preview | undefined;
  /** Something other than the file as it is may be playing. */
  private heard = false;

  constructor(private fileId: number) {}

  /** Hear `preview`, if this file is playing. */
  send(preview: api.Preview) {
    if (!transport.playing || tabs.activeId !== this.fileId) return;
    this.heard = true;
    this.waiting = preview;
    this.pump();
  }

  private pump() {
    const preview = this.waiting;
    if (this.inFlight || !preview) return;
    this.waiting = undefined;
    this.inFlight = api
      .previewEdits(this.fileId, preview)
      .catch(() => {})
      .finally(() => {
        this.inFlight = null;
        this.pump();
      });
  }

  /** Drop any preview not yet sent, and wait for the one being sent (before committing). */
  async settle() {
    this.waiting = undefined;
    while (this.inFlight) await this.inFlight;
  }

  /** Go back to hearing the file as it is (on cancel). */
  async cancel() {
    await this.settle();
    if (!this.heard) return;
    this.heard = false;
    await api.previewEdits(this.fileId, null).catch(() => {});
  }
}
