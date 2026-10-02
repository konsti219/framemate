<script lang="ts">
  // Horizontal battery glyph (original drawing): outline in currentColor, fill drawn to scale.
  let { level, charging = false, size = 20 }: { level: number | null | undefined; charging?: boolean; size?: number } =
    $props();
  const fill = $derived(Math.max(0, Math.min(1, level ?? 0)));
  const color = $derived(charging || fill > 0.2 ? "var(--green)" : fill > 0.1 ? "var(--yellow)" : "var(--red)");
</script>

<svg viewBox="-4 0 52 36" width={(size * 52) / 36} height={size} aria-hidden="true">
  <rect x="1.5" y="7.5" width="36" height="21" rx="3" fill="none" stroke="currentColor" stroke-width="3" />
  <rect x="39" y="13.5" width="3.5" height="9" rx="1.2" fill="currentColor" />
  {#if level != null}
    <rect x="5" y="11" width={29 * fill} height="14" rx="1" fill={color} />
  {/if}
  {#if charging}
    <path d="M21.5 9.5L14 19.5H19L17.5 26.5L25 16.5H20Z" fill="white" />
  {/if}
</svg>
