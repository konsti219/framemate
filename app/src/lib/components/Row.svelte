<script lang="ts">
  // SteamOS-settings style row: label on the left, value on the right.
  import type { Snippet } from "svelte";
  let { label, value, children }: { label: string; value?: unknown; children?: Snippet } = $props();
</script>

<div class="row">
  <span class="label">{label}</span>
  <span class="value">
    {#if children}{@render children()}{:else}{value ?? "–"}{/if}
  </span>
</div>

<style>
  .row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    min-height: 48px;
    padding: 0 var(--gutter);
    background: var(--surface);
    border-bottom: 1px solid var(--divider);
  }
  .label {
    color: var(--text);
  }
  .value {
    color: var(--muted);
    text-align: right;
    overflow-wrap: anywhere;
  }
</style>
