<script lang="ts">
  let { name, fraction }: { name: string; fraction: number } = $props();
</script>

<div class="progress" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(fraction * 100)}>
  <div class="label">Loading “{name}”… <span class="mono">{Math.round(fraction * 100)}%</span></div>
  <div class="bar"><div class="fill" style:width="{fraction * 100}%"></div></div>
</div>

<style>
  .progress {
    position: fixed;
    left: 50%;
    top: 64px;
    transform: translateX(-50%);
    width: min(420px, calc(100vw - 32px));
    background: var(--panel-bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 12px 14px;
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.18);
    z-index: 15;
  }
  .label {
    margin-bottom: 8px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .bar {
    height: 6px;
    border-radius: 3px;
    background: var(--border);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--accent);
    transition: width 80ms linear;
  }
</style>
