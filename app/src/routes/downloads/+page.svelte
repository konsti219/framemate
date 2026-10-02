<script lang="ts">
  import { agent } from "$lib/agent.svelte";
  import GameArt from "$lib/components/GameArt.svelte";
  import Progress from "$lib/components/Progress.svelte";
  import Section from "$lib/components/Section.svelte";
  import { bytes, duration } from "$lib/format";

  const topics = $derived(agent.state?.steam.topics);
  const overview = $derived(topics?.download_overview);
  const items = $derived(topics?.downloads ?? []);
  const queued = $derived(items.filter(i => !i.completed).sort((a, b) => a.queue_index - b.queue_index));
  const finished = $derived(items.filter(i => i.completed).sort((a, b) => b.completed_time - a.completed_time));

  function finishedAgo(unixSeconds: number) {
    return unixSeconds ? `${duration(Date.now() / 1000 - unixSeconds)} ago` : "";
  }
</script>

<Section title="Queue">
  {#if queued.length}
    {#each queued as item (item.appid)}
      {@const live = overview?.appid === item.appid ? overview : null}
      <div class="item">
        <GameArt appid={item.appid} kind="capsule" />
        <div class="body">
          <div class="line">
            <span class="name">{item.name ?? item.appid}</span>
            <span class="muted">{live?.percent ?? item.percent ?? 0}%</span>
          </div>
          <Progress value={live?.percent ?? item.percent} paused={item.paused} />
          <div class="line muted small">
            <span>
              {#if item.error}Error: {item.error}
              {:else if item.paused}Paused
              {:else if live}{bytes(live.bytes_per_sec)}/s
              {:else}Queued{/if}
            </span>
            <span>{live?.eta_sec ? `${duration(live.eta_sec)} left` : ""}</span>
          </div>
        </div>
      </div>
    {/each}
  {:else}
    <p class="empty">Nothing queued</p>
  {/if}
</Section>

{#if finished.length}
  <Section title="Recently finished">
    {#each finished as item (item.appid)}
      <div class="item">
        <GameArt appid={item.appid} kind="capsule" />
        <div class="body">
          <span class="name">{item.name ?? item.appid}</span>
          <span class="muted small">Updated {finishedAgo(item.completed_time)}</span>
        </div>
      </div>
    {/each}
  </Section>
{/if}

<style>
  .item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px var(--gutter);
    background: var(--surface);
    border-bottom: 1px solid var(--divider);
  }
  .body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .line {
    display: flex;
    justify-content: space-between;
    gap: 8px;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .small {
    font-size: 13px;
  }
</style>
