<script lang="ts">
  // Steam CDN artwork; tools and runtimes have none, so fall back to a neutral tile.
  import { appArt } from "$lib/format";
  import Icon from "./Icon.svelte";

  let { appid, kind = "header" }: { appid: number; kind?: "header" | "capsule" } = $props();
  let failed = $state(false);
  $effect(() => {
    appid;
    failed = false;
  });
</script>

<div class="art {kind}">
  {#if failed}
    <Icon name="library" size={kind === "header" ? 40 : 22} />
  {:else}
    <img src={appArt(appid, kind)} alt="" loading="lazy" onerror={() => (failed = true)} />
  {/if}
</div>

<style>
  .art {
    display: grid;
    place-items: center;
    background: var(--surface-2);
    color: var(--muted);
    overflow: hidden;
    flex: none;
  }
  .header {
    width: 100%;
    aspect-ratio: 460 / 215;
  }
  .capsule {
    width: 92px;
    aspect-ratio: 184 / 69;
    border-radius: 2px;
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
</style>
