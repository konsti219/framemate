<script lang="ts" module>
  // Open-source icons (Material Symbols, see ../icons/LICENSES.md); they use currentColor.
  const icons = import.meta.glob("../icons/*.svg", { query: "?raw", import: "default", eager: true }) as Record<string, string>;
</script>

<script lang="ts">
  let { name, size = 24 }: { name: string; size?: number } = $props();
  const svg = $derived(icons[`../icons/${name}.svg`] ?? "");
</script>

<span class="icon" style:width="{size}px" style:height="{size}px" aria-hidden="true">{@html svg}</span>

<style>
  .icon {
    display: inline-flex;
    flex: none;
  }
  .icon :global(svg) {
    width: 100%;
    height: 100%;
  }
</style>
