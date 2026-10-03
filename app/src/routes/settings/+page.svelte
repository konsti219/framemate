<script lang="ts">
  import { agent } from "$lib/agent.svelte";
  import Row from "$lib/components/Row.svelte";
  import Section from "$lib/components/Section.svelte";

  let host = $state(agent.settings.host);
  let token = $state(agent.settings.token);

  const statusText = $derived(
    {
      unconfigured: "Enter host and token",
      connecting: "Connecting…",
      connected: `Connected to ${agent.state?.agent.hostname ?? agent.authority}`,
      offline: `Can't reach ${agent.authority}`,
      unauthorized: "Wrong token. Check it on the Frame (see below).",
    }[agent.status],
  );

  function save(event: SubmitEvent) {
    event.preventDefault();
    agent.save({ host, token });
  }
</script>

<Section title="Connection">
  <form onsubmit={save}>
    <label>
      <span>Frame address</span>
      <input bind:value={host} placeholder="frame.local or 192.168.x.x" autocapitalize="off" autocorrect="off" spellcheck="false" />
    </label>
    <label>
      <span>Token</span>
      <input bind:value={token} placeholder="XXXXX-XXXXX" autocapitalize="characters" autocorrect="off" spellcheck="false" />
    </label>
    <button class="button" type="submit">Save &amp; connect</button>
    <p class="hint">Show the token on the Frame with <code>flatpak run dev.framemate.Agent token</code>.</p>
    <p class="status {agent.status}">{statusText}</p>
  </form>
</Section>

<Section title="About">
  <Row label="Agent version" value={agent.state?.agent.version} />
</Section>

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 16px var(--gutter);
    background: var(--surface);
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  label span {
    font-size: 13px;
    color: var(--muted);
  }
  input {
    min-height: 44px;
    padding: 0 12px;
    background: var(--surface-2);
    border: 1px solid transparent;
    border-radius: 2px;
  }
  input:focus {
    outline: none;
    border-color: var(--accent);
  }
  .hint {
    margin: 0;
    font-size: 13px;
    color: var(--muted);
  }
  .status {
    margin: 0;
    color: var(--muted);
  }
  .status.connected {
    color: var(--green);
  }
  .status.offline,
  .status.unauthorized {
    color: var(--red);
  }
</style>
