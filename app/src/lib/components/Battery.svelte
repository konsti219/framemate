<script lang="ts">
  // Steam's battery glyph (same geometry as the client's), with the fill drawn to scale.
  let { level, charging = false, size = 20 }: { level: number | null | undefined; charging?: boolean; size?: number } =
    $props();
  const fill = $derived(Math.max(0, Math.min(1, level ?? 0)));
  const color = $derived(charging || fill > 0.2 ? "var(--green)" : fill > 0.1 ? "var(--yellow)" : "var(--red)");
</script>

<svg viewBox="-4 0 52 36" width={(size * 52) / 36} height={size} aria-hidden="true">
  <path fill-rule="evenodd" d="M39 6H0V30H39V22H42V14H39V6ZM36 9H3V27H36V9Z" fill="currentColor" />
  {#if level != null}
    <rect x="6" y="12" width={27 * fill} height="12" fill={color} />
  {/if}
  {#if charging}
    <path d="M16 20L21 11V16H26L21 25V20H16Z" fill="white" />
  {/if}
</svg>
