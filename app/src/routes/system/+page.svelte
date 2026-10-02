<script lang="ts">
  import { agent } from "$lib/agent.svelte";
  import Row from "$lib/components/Row.svelte";
  import Section from "$lib/components/Section.svelte";
  import { bytes, hmdActivity, since } from "$lib/format";

  const s = $derived(agent.state);
  const topics = $derived(s?.steam.topics);
  const props = $derived(s?.steamos.properties ?? {});
  const kernel = $derived(s?.power?.battery);
  const info = $derived(topics?.system_info);
  const net = $derived(topics?.network);
  const perf = $derived(topics?.perf);
  const yesNo = (v: boolean | null | undefined) => (v == null ? null : v ? "Yes" : "No");
</script>

{#if s}
  <Section title="Headset">
    <Row label="Status" value={hmdActivity(topics?.vr_state?.hmd_activity)} />
    <Row label="VR app" value={topics?.vr_state?.scene_app_name ?? "None"} />
    <Row label="Power draw" value={kernel?.power_w != null ? `${Math.abs(kernel.power_w).toFixed(1)} W` : null} />
    <Row label="Battery temperature" value={kernel?.temp_c != null ? `${kernel.temp_c.toFixed(1)} °C` : null} />
    <Row label="Battery health" value={kernel ? `${kernel.health ?? "–"} · ${kernel.cycle_count ?? "–"} cycles` : null} />
  </Section>

  <Section title="Performance">
    <Row label="Profile" value={props.PerformanceProfile} />
    <Row label="CPU governor" value={props.CpuScalingGovernor} />
    <Row label="CPU scheduler" value={props.CpuScheduler} />
    <Row label="GPU performance" value={props.GpuPerformanceLevel} />
    <Row label="TDP limit" value={perf?.tdp_limit_w != null ? `${perf.tdp_limit_w} W` : "Off"} />
    <Row label="Frame limit" value={perf?.fps_limit ?? "Off"} />
  </Section>

  <Section title="Internet">
    <Row label="Internet" value={yesNo(net?.internet)} />
    <Row label="Steam" value={yesNo(net?.steam)} />
    {#each net?.interfaces ?? [] as i (i.name)}
      <Row label={i.name} value={`↓ ${bytes(i.rx_bytes_per_sec)}/s · ↑ ${bytes(i.tx_bytes_per_sec)}/s`} />
    {/each}
  </Section>

  <Section title="Software">
    <Row label="SteamOS" value={info ? `${info.os_version} (${info.os_variant})` : null} />
    <Row label="OS build" value={info?.os_build} />
    <Row label="Steam client" value={info?.steam_version} />
    <Row label="Kernel" value={info?.kernel} />
    <Row label="Graphics" value={info ? `${info.gpu} · ${info.gpu_driver}` : null} />
    <Row label="Memory" value={info ? `${(info.ram_mb / 1024).toFixed(0)} GB` : null} />
  </Section>

  <Section title="Account">
    <Row label="Signed in as" value={topics?.user?.persona_name} />
    <Row label="Account" value={topics?.user?.account_name} />
  </Section>

  <Section title="FrameMate agent">
    <Row label="Version" value={s.agent.version} />
    <Row label="Running for" value={since(s.agent.started_at_ms)} />
    <Row label="Steam connection" value={s.steam.connected ? "Connected" : (s.steam.error ?? "Not connected")} />
  </Section>
{:else}
  <p class="empty">Not connected</p>
{/if}
