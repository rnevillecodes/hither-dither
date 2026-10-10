<script lang="ts">
  import type { Metadata } from "../lib/api";
  import {
    formatBitDepth,
    formatBytes,
    formatDuration,
    formatInt,
    formatRate,
  } from "../lib/format";

  let { meta }: { meta: Metadata } = $props();

  const rows = $derived([
    ["File", meta.name],
    ["Size", formatBytes(meta.size_bytes)],
    ["Encoding", meta.encoding],
    ["Bit depth", formatBitDepth(meta.bit_depth, meta.valid_bits, meta.encoding)],
    ["Sample rate", formatRate(meta.sample_rate)],
    ["Channels", `${meta.channels} (${meta.channel_labels.join(", ")})`],
    ["Frames", formatInt(meta.frames)],
    ["Duration", formatDuration(meta.duration_secs)],
  ]);
</script>

<section>
  <h2>File</h2>
  <dl>
    {#each rows as [k, v] (k)}
      <dt>{k}</dt>
      <dd class="mono" title={k === "File" ? meta.path : undefined}>{v}</dd>
    {/each}
  </dl>
  {#if meta.truncated}
    <p class="warn">The file appears truncated. Only the audio present was loaded.</p>
  {/if}
</section>

<style>
  h2 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-dim);
    margin: 0 0 8px;
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 4px 12px;
    margin: 0;
  }
  dt {
    color: var(--text-dim);
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .warn {
    color: var(--warning);
    margin: 8px 0 0;
  }
</style>
