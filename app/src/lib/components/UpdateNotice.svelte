<script lang="ts">
  // Small notice above the tab bar when the startup check found a newer release.
  import { goto } from "$app/navigation";
  import { updates } from "$lib/updates.svelte";

  const what = $derived(
    updates.appOutdated && updates.agentOutdated ? "App and agent" : updates.appOutdated ? "App" : "Agent",
  );
</script>

<div class="notice" role="status">
  <button class="body" onclick={() => goto("/settings")}>
    <strong>Update available: v{updates.latest?.version}</strong>
    <span>{what} can be updated. Tap for details.</span>
  </button>
  <button class="close" onclick={() => (updates.dismissed = true)} aria-label="Dismiss">✕</button>
</div>

<style>
  .notice {
    display: flex;
    align-items: stretch;
    margin: 0 var(--gutter) 10px;
    background: var(--surface-2);
    border-left: 3px solid var(--accent);
    border-radius: 2px;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.4);
  }
  .body {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 12px;
    background: none;
    border: 0;
    text-align: left;
    cursor: pointer;
  }
  .body span {
    font-size: 13px;
    color: var(--muted);
  }
  .close {
    padding: 0 14px;
    background: none;
    border: 0;
    color: var(--muted);
    cursor: pointer;
  }
</style>
