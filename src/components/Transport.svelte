<script lang="ts">
  import { app } from "../lib/app.svelte";
  import { formatDuration } from "../lib/format";
  import {
    changeVolume,
    playbackAvailable,
    stop,
    toggleLoop,
    togglePlay,
    transport,
  } from "../lib/transport.svelte";

  const disabled = $derived(!playbackAvailable());
  const reason = $derived(transport.info && !transport.info.available ? `Playback disabled: ${transport.info.reason}` : "");
  const time = $derived(app.meta ? formatDuration(transport.playhead / app.meta.sample_rate) : "0:00:00.000");
</script>

<div class="transport" title={reason || undefined}>
  <button class="play" onclick={togglePlay} {disabled} title={reason || (transport.playing ? "Pause (Space)" : "Play (Space)")} aria-label={transport.playing ? "Pause" : "Play"}>
    {#if transport.playing}
      <svg viewBox="0 0 16 16" aria-hidden="true"><rect x="3" y="2" width="3.5" height="12" /><rect x="9.5" y="2" width="3.5" height="12" /></svg>
    {:else}
      <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 2l10 6-10 6z" /></svg>
    {/if}
  </button>
  <button onclick={() => void stop()} {disabled} title={reason || "Stop and return to where playback began"} aria-label="Stop">
    <svg viewBox="0 0 16 16" aria-hidden="true"><rect x="3" y="3" width="10" height="10" /></svg>
  </button>
  <button class:active={transport.looping} onclick={toggleLoop} {disabled} title={reason || "Loop the selection (or the whole file)"} aria-pressed={transport.looping}>
    Loop
  </button>
  <span class="time mono" aria-label="Playhead position">{time}</span>
  <label class="volume" title={reason || "Playback volume (does not affect statistics)"}>
    <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M2 6h3l4-3v10l-4-3H2z" /><path d="M11 5.5a3.5 3.5 0 010 5" fill="none" stroke="currentColor" stroke-width="1.4" /></svg>
    <input type="range" min="0" max="100" step="1" value={Math.round(transport.volume * 100)} oninput={(e) => changeVolume(Number(e.currentTarget.value) / 100)} {disabled} aria-label="Volume" />
    <span class="mono pct">{Math.round(transport.volume * 100)}%</span>
  </label>
  {#if reason}
    <span class="reason">{reason}</span>
  {/if}
</div>

<style>
  .transport {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 32px;
    height: 28px;
  }
  svg {
    width: 14px;
    height: 14px;
    fill: currentColor;
  }
  .time {
    min-width: 92px;
    margin: 0 8px;
  }
  .volume {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-dim);
  }
  .volume input {
    width: 90px;
  }
  .pct {
    min-width: 34px;
    font-size: 12px;
  }
  .reason {
    color: var(--warning);
    margin-left: 8px;
    font-size: 12px;
    max-width: 280px;
  }
</style>
