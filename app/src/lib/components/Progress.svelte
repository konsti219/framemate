<script lang="ts">
  let { value, paused = false }: { value: number | null | undefined; paused?: boolean } = $props();
</script>

<div class="track" role="progressbar" aria-valuenow={value ?? 0} aria-valuemin="0" aria-valuemax="100">
  <div class="bar" class:paused style:width="{Math.max(0, Math.min(100, value ?? 0))}%"></div>
</div>

<style>
  .track {
    height: 6px;
    background: var(--track);
    border-radius: 3px;
    overflow: hidden;
  }
  .bar {
    height: 100%;
    background: linear-gradient(90deg, var(--accent), var(--accent-2));
    transition: width 0.4s ease;
  }
  .paused {
    background: var(--muted);
  }
</style>
