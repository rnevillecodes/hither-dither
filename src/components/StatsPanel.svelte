<script lang="ts">
  import type { Stats } from "../lib/api";
  import { formatDb, formatInt, formatLinear, formatPercent } from "../lib/format";

  interface Props {
    stats: Stats | null;
    labels: string[];
    scope: "Whole file" | "Selection";
    pending: boolean;
    /** The figures describe the audio before the latest edits; new ones are on their way. */
    updating: boolean;
  }
  let { stats, labels, scope, pending, updating }: Props = $props();

  const loudness = $derived.by(() => {
    const l = stats?.loudness;
    if (!l) return { text: "…", tip: "" };
    if (l.kind === "value") return { text: formatDb(l.lufs, "LUFS"), tip: "Integrated loudness (ITU-R BS.1770-4 / EBU R128)" };
    if (l.kind === "silent") return { text: "-inf LUFS", tip: "All audio is below the −70 LUFS absolute gate" };
    return { text: "n/a", tip: l.reason };
  });
</script>

<section>
  <h2>
    Levels
    <span class="scope" class:sel={scope === "Selection"}>{scope}</span>
    {#if pending}<span class="pending">computing…</span>{:else if updating}<span class="pending">Updating…</span>{/if}
  </h2>

  {#if stats}
    <div class="table-wrap" class:stale={pending || updating}>
      <table class="mono">
        <thead>
          <tr>
            <th></th>
            {#each stats.channels as _, c (c)}<th>{labels[c] ?? `Ch ${c + 1}`}</th>{/each}
          </tr>
        </thead>
        <tbody>
          <tr>
            <th>Peak</th>
            {#each stats.channels as ch, c (c)}<td>{formatDb(ch.peak_dbfs)}</td>{/each}
          </tr>
          <tr>
            <th>Peak (linear)</th>
            {#each stats.channels as ch, c (c)}<td>{formatLinear(ch.peak_linear)}</td>{/each}
          </tr>
          <tr>
            <th>RMS</th>
            {#each stats.channels as ch, c (c)}<td>{formatDb(ch.rms_dbfs)}</td>{/each}
          </tr>
          <tr>
            <th>DC offset</th>
            {#each stats.channels as ch, c (c)}<td>{formatPercent(ch.dc_offset_percent)}</td>{/each}
          </tr>
          <tr>
            <th>Crest factor</th>
            {#each stats.channels as ch, c (c)}<td>{ch.crest_db === null ? "–" : formatDb(ch.crest_db, "dB")}</td>{/each}
          </tr>
          <tr>
            <th title="Runs of 3 or more consecutive full-scale samples">Clipped runs</th>
            {#each stats.channels as ch, c (c)}
              <td class:clip={ch.clip_runs > 0}>{formatInt(ch.clip_runs)}</td>
            {/each}
          </tr>
        </tbody>
      </table>
    </div>
    <p class="loudness" class:stale={updating}>
      <span class="label">Integrated loudness</span>
      <span class="mono value" title={loudness.tip}>{loudness.text}</span>
    </p>
  {/if}
</section>

<style>
  h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-dim);
    margin: 0 0 8px;
  }
  .scope {
    text-transform: none;
    letter-spacing: 0;
    font-size: 11px;
    padding: 1px 7px;
    border-radius: 10px;
    border: 1px solid var(--border);
    color: var(--text);
  }
  .scope.sel {
    border-color: var(--accent);
    color: var(--accent);
  }
  .pending {
    text-transform: none;
    letter-spacing: 0;
    font-weight: normal;
  }
  .table-wrap {
    overflow-x: auto;
  }
  .stale {
    opacity: 0.55;
  }
  table {
    border-collapse: collapse;
    width: 100%;
  }
  th,
  td {
    padding: 3px 6px;
    text-align: right;
    white-space: nowrap;
    border-bottom: 1px solid var(--border);
  }
  tbody th {
    text-align: left;
    font-weight: normal;
    color: var(--text-dim);
  }
  thead th {
    font-weight: 600;
  }
  .clip {
    color: var(--danger);
    font-weight: 600;
  }
  .loudness {
    display: flex;
    justify-content: space-between;
    margin: 10px 0 0;
  }
  .label {
    color: var(--text-dim);
  }
  .value {
    font-weight: 600;
    cursor: help;
  }
</style>
