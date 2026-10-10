// Playback transport: play/pause/stop, seeking, looping, volume and the playhead.
import * as api from "./api";
import { app } from "./app.svelte";
import { notify } from "./notices.svelte";
import { tabs } from "./tabs.svelte";
import { reveal, view } from "./view.svelte";
import { visibleEnd } from "./viewport";

export const transport = $state({
  info: null as api.PlaybackInfo | null,
  playing: false,
  /** Playhead frame shown in the view (interpolated while playing). */
  playhead: 0,
  /** Where the current/last playback began; Stop returns here. */
  playStart: 0,
  looping: false,
  /** 0 to 1. */
  volume: 1,
});

/** Last position reported by the backend and when it arrived. */
let reported = { frame: 0, at: 0 };
let session = 0;
/** The session in which playback was last stopped (pause, stop or halt). */
let stoppedSession = -1;
let raf = 0;

export async function initTransport() {
  try {
    transport.info = await api.playbackInfo();
  } catch (e) {
    transport.info = { available: false, reason: String(e), device: null };
  }
}

export function playbackAvailable(): boolean {
  return !!transport.info?.available && !!app.meta;
}

/** Stop playback at once (for switching or closing tabs), leaving the playhead where it is. */
export function haltPlayback() {
  stoppedSession = ++session;
  stopTicking();
  transport.playing = false;
  void api.stopPlayback();
}

export async function play() {
  const meta = app.meta;
  const fileId = tabs.activeId;
  if (!meta || fileId === null || !playbackAvailable()) return;
  const total = meta.frames;
  const sel = app.selection;
  let start: number;
  let end: number;
  let from: number;
  if (sel) {
    start = sel.start;
    end = sel.end;
    // Resume inside the selection, otherwise start at its beginning.
    from = transport.playhead >= sel.start && transport.playhead < sel.end ? transport.playhead : sel.start;
  } else {
    start = 0;
    end = total;
    from = transport.playhead >= total ? 0 : transport.playhead;
  }
  from = Math.round(from);
  const token = ++session;
  transport.playStart = from;
  transport.playhead = from;
  reported = { frame: from, at: performance.now() };
  view.userScrolled = false;
  try {
    await api.play(fileId, start, end, from, transport.looping, (e) => {
      if (token !== session) return;
      if (e.kind === "position") {
        reported = { frame: e.frame, at: performance.now() };
      } else {
        stopTicking();
        transport.playing = false;
        transport.playhead = e.frame;
      }
    });
    if (token !== session) {
      // Stopped while the backend was starting: make sure it does not keep playing.
      if (session === stoppedSession) void api.stopPlayback();
      return;
    }
    transport.playing = true;
    tick();
  } catch (e) {
    if (token === session) {
      transport.playing = false;
      notify("error", "Could not start playback", String(e));
    }
  }
}

export async function pause() {
  stoppedSession = ++session;
  stopTicking();
  const pos = await api.stopPlayback();
  transport.playing = false;
  if (pos !== null) transport.playhead = Math.round(pos);
}

export async function stop() {
  stoppedSession = ++session;
  stopTicking();
  await api.stopPlayback();
  transport.playing = false;
  transport.playhead = transport.playStart;
}

export function togglePlay() {
  if (transport.playing) void pause();
  else void play();
}

/** Move the playhead; playback continues from there if it was running. */
export function seek(frame: number) {
  const total = app.meta?.frames ?? 0;
  transport.playhead = Math.max(0, Math.min(Math.round(frame), total));
  if (transport.playing) void play();
}

export function toggleLoop() {
  transport.looping = !transport.looping;
  void api.setLoop(transport.looping);
}

export function changeVolume(v: number) {
  transport.volume = Math.max(0, Math.min(1, v));
  void api.setVolume(transport.volume);
}

function stopTicking() {
  cancelAnimationFrame(raf);
}

/** Per-frame playhead interpolation and auto-scroll. */
function tick() {
  const meta = app.meta;
  if (!transport.playing || !meta) return;
  const elapsed = Math.min(performance.now() - reported.at, 50) / 1000;
  transport.playhead = reported.frame + elapsed * meta.sample_rate;
  const vp = view.vp;
  if (vp && !view.userScrolled) {
    const end = visibleEnd(vp);
    if (transport.playhead >= end || transport.playhead < vp.start) reveal(transport.playhead, 0.05);
  }
  raf = requestAnimationFrame(tick);
}
