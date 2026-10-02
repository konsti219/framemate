<script lang="ts">
  import { agent } from "$lib/agent.svelte";
  import Battery from "$lib/components/Battery.svelte";
  import GameArt from "$lib/components/GameArt.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import Progress from "$lib/components/Progress.svelte";
  import Section from "$lib/components/Section.svelte";
  import { bytes, duration, hmdActivity, percent } from "$lib/format";

  const topics = $derived(agent.state?.steam.topics);
  const battery = $derived(topics?.battery);
  const charging = $derived(battery?.ac_state === 2);
  const devices = $derived(topics?.vr_devices ?? []);
  const controllers = $derived(
    devices
      .filter(d => d.device_class === 2)
      .sort((a, b) => (a.model ?? "").localeCompare(b.model ?? "")),
  );
  const running = $derived(topics?.running_apps ?? []);
  const download = $derived(topics?.download_overview?.appid ? topics.download_overview : null);
  const kernel = $derived(agent.state?.power?.battery);

  function controllerName(model: string | null) {
    if (model?.endsWith("_Left")) return "Left controller";
    if (model?.endsWith("_Right")) return "Right controller";
    return model ?? "Controller";
  }
</script>

{#if !agent.state}
  <p class="empty">
    {agent.status === "unconfigured" ? "Set up the connection in Settings." : "Connecting to your Frame…"}
  </p>
{:else}
  <Section title="Steam Frame">
    <div class="hero">
      <Icon name="headset" size={56} />
      <div class="hero-text">
        {#if battery}
          <div class="big">
            {percent(battery.level)}
            <Battery level={battery.level} {charging} size={26} />
          </div>
          <div class="muted">
            {charging
              ? `Charging · full in ${duration(battery.seconds_remaining)}`
              : `About ${duration(battery.seconds_remaining)} left`}
          </div>
        {:else}
          <div class="big">–</div>
        {/if}
        <div class="muted">
          {hmdActivity(topics?.vr_state?.hmd_activity)}
          {#if kernel?.power_w != null}· {Math.abs(kernel.power_w).toFixed(1)} W{/if}
        </div>
      </div>
    </div>
  </Section>

  {#if controllers.length}
    <Section title="Controllers">
      <div class="controllers">
        {#each controllers as c (c.path)}
          <div class="controller">
            <Icon name="controller" size={28} />
            <div>
              <div>{controllerName(c.model)}</div>
              <div class="muted small">
                {percent(c.battery)}
                {c.charging ? "· charging" : c.connected ? "" : "· asleep"}
              </div>
            </div>
            <span class="spacer"></span>
            <Battery level={c.battery} charging={!!c.charging} size={18} />
          </div>
        {/each}
      </div>
    </Section>
  {/if}

  <Section title="Now playing">
    {#if running.length}
      {#each running as app (app.appid)}
        <div class="card">
          <GameArt appid={app.appid} />
          <div class="card-body">{app.name ?? app.appid}</div>
        </div>
      {/each}
    {:else}
      <p class="empty">Nothing running</p>
    {/if}
  </Section>

  <Section title="Downloading">
    {#snippet aside()}<a href="/downloads" class="more">All downloads ›</a>{/snippet}
    {#if download?.appid}
      <div class="card">
        <GameArt appid={download.appid} />
        <div class="card-body">
          <div class="line">
            <span>{download.name ?? download.appid}{download.is_workshop ? " · Workshop" : ""}</span>
            <span class="muted">{download.percent ?? 0}%</span>
          </div>
          <Progress value={download.percent} paused={download.paused} />
          <div class="line muted small">
            <span>{download.paused ? "Paused" : `${bytes(download.bytes_per_sec)}/s`}</span>
            <span>{download.eta_sec ? `${duration(download.eta_sec)} left` : ""}</span>
          </div>
        </div>
      </div>
    {:else}
      <p class="empty">No active downloads</p>
    {/if}
  </Section>
{/if}

<style>
  .hero {
    display: flex;
    align-items: center;
    gap: 20px;
    padding: 20px var(--gutter);
    background: linear-gradient(135deg, #2a475e, var(--surface));
  }
  .hero-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .big {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 32px;
    font-weight: 600;
  }
  .controllers {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .controller {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px var(--gutter);
    background: var(--surface);
  }
  .spacer {
    flex: 1;
  }
  .small {
    font-size: 13px;
  }
  .card {
    background: var(--surface);
  }
  .card + .card {
    margin-top: 8px;
  }
  .card-body {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px var(--gutter);
  }
  .line {
    display: flex;
    justify-content: space-between;
    gap: 12px;
  }
  .more {
    font-size: 13px;
    color: var(--muted);
  }
</style>
