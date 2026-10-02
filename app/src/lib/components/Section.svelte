<script lang="ts">
  import type { Snippet } from "svelte";
  let { title, aside, children }: { title: string; aside?: Snippet; children: Snippet } = $props();
</script>

<section>
  <header>
    <h2>{title}</h2>
    {@render aside?.()}
  </header>
  {@render children()}
</section>

<style>
  section {
    margin: 20px 0 0;
  }
  header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    padding: 0 var(--gutter) 8px;
  }
  h2 {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text);
  }
</style>
