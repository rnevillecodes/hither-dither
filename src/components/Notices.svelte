<script lang="ts">
  import { dismiss, notices } from "../lib/notices.svelte";
</script>

<div class="notices" aria-live="polite">
  {#each notices as n (n.id)}
    <div class="notice {n.level}" role={n.level === "error" ? "alert" : "status"}>
      <div class="text">
        <strong>{n.title}</strong>
        {#if n.body}<p>{n.body}</p>{/if}
      </div>
      <button class="close" onclick={() => dismiss(n.id)} aria-label="Dismiss">×</button>
    </div>
  {/each}
</div>

<style>
  .notices {
    position: fixed;
    right: 16px;
    bottom: 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: min(440px, calc(100vw - 32px));
    z-index: 20;
  }
  .notice {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    background: var(--panel-bg);
    border: 1px solid var(--border);
    border-left: 4px solid var(--accent);
    border-radius: 8px;
    padding: 10px 12px;
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.45);
  }
  .notice.warning {
    border-left-color: var(--warning);
  }
  .notice.error {
    border-left-color: var(--danger);
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  p {
    margin: 4px 0 0;
    color: var(--text-dim);
    overflow-wrap: anywhere;
    /* Lists in a notice, such as effects a session couldn't restore, are one per line. */
    white-space: pre-line;
  }
  .close {
    border: none;
    background: none;
    font-size: 18px;
    line-height: 1;
    padding: 0 4px;
    color: var(--text-dim);
  }
</style>
