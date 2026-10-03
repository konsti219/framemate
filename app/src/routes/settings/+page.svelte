<script lang="ts">
  import { agent } from "$lib/agent.svelte";
  import Row from "$lib/components/Row.svelte";
  import Section from "$lib/components/Section.svelte";
  import { openExternal } from "$lib/external";
  import { APP_VERSION, updates } from "$lib/updates.svelte";

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

<Section title="Updates">
  <Row label="App version" value={APP_VERSION} />
  <Row label="Agent version" value={agent.state?.agent.version ?? "not connected"} />
  <Row label="Latest release">
    {#if updates.checking}
      Checking…
    {:else if updates.error}
      <span class="error">{updates.error}</span>
    {:else if updates.latest}
      v{updates.latest.version}
      {#if updates.available}<span class="badge">new</span>{:else}· up to date{/if}
    {:else}
      not checked
    {/if}
  </Row>
  <label class="row toggle">
    <span>Check for updates on start</span>
    <input
      type="checkbox"
      role="switch"
      checked={updates.autoCheck}
      onchange={event => updates.setAutoCheck(event.currentTarget.checked)}
    />
  </label>
  <div class="actions">
    {#if updates.available && updates.latest}
      <p class="hint">
        {#if updates.appOutdated}Download the new <code>framemate.apk</code> from the release page.{/if}
        {#if updates.agentOutdated}Update the agent on the Frame: download the new
          <code>framemate-agent.flatpak</code> and run the install commands again (see Help).{/if}
      </p>
      <button class="button" onclick={() => openExternal(updates.latest!.url)}>Open release page</button>
    {/if}
    <button class="button secondary" onclick={() => updates.check()} disabled={updates.checking}>
      {updates.checking ? "Checking…" : "Check for updates"}
    </button>
  </div>
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
  .error {
    color: var(--red);
  }
  .badge {
    margin-left: 6px;
    padding: 1px 6px;
    border-radius: 2px;
    background: var(--accent);
    color: #fff;
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
  }
  /* Same look as Row, with a SteamOS-style switch on the right. */
  .toggle {
    display: flex;
    flex-direction: row;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    min-height: 48px;
    padding: 0 var(--gutter);
    background: var(--surface);
    border-bottom: 1px solid var(--divider);
  }
  .toggle span {
    font-size: inherit;
    color: var(--text);
  }
  .toggle input {
    appearance: none;
    position: relative;
    flex: none;
    width: 44px;
    min-height: 0;
    height: 24px;
    padding: 0;
    border: 0;
    border-radius: 12px;
    background: var(--track);
    cursor: pointer;
    transition: background 0.15s;
  }
  .toggle input::after {
    content: "";
    position: absolute;
    top: 3px;
    left: 3px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: #fff;
    transition: transform 0.15s;
  }
  .toggle input:checked {
    background: var(--accent);
  }
  .toggle input:checked::after {
    transform: translateX(20px);
  }
  .actions {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px var(--gutter);
    background: var(--surface);
  }
  .button.secondary {
    background: var(--surface-2);
  }
  .button:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
