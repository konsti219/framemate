<script lang="ts">
  import "../app.css";
  import { onMount, type Snippet } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { agent } from "$lib/agent.svelte";
  import Battery from "$lib/components/Battery.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import UpdateNotice from "$lib/components/UpdateNotice.svelte";
  import { updates } from "$lib/updates.svelte";

  let { children }: { children: Snippet } = $props();

  const tabs = [
    { href: "/", icon: "home", label: "Home" },
    { href: "/downloads", icon: "download", label: "Downloads" },
    { href: "/view", icon: "cast", label: "Mirroring" },
    { href: "/system", icon: "performance", label: "System" },
    { href: "/settings", icon: "settings", label: "Settings" },
  ];

  const battery = $derived(agent.state?.steam.topics.battery);

  onMount(() => {
    agent.connect();
    if (updates.autoCheck) updates.check();
    if (!agent.configured) goto("/settings");
    // Mobile WebViews drop sockets in the background; reconnect when we come back.
    const onVisible = () => {
      if (document.visibilityState === "visible" && agent.status !== "connected") agent.connect();
    };
    document.addEventListener("visibilitychange", onVisible);
    return () => document.removeEventListener("visibilitychange", onVisible);
  });
</script>

<div class="app">
  <header class="topbar">
    <img src="/favicon.svg" alt="" class="logo" />
    <a href="/" class="title">FrameMate</a>
    <span class="spacer"></span>
    {#if battery}
      <span class="battery">
        {Math.round(battery.level * 100)}%
        <Battery level={battery.level} charging={battery.ac_state === 2} size={18} />
      </span>
    {/if}
    <span class="status {agent.status}" title={agent.status}></span>
    <a href="/help" class="help" class:active={true} aria-label="Help" >
      <Icon name="help" size={20} />
    </a>
  </header>

  <main>
    {@render children()}
  </main>

  {#if updates.available && !updates.dismissed && page.url.pathname !== "/settings"}
    <UpdateNotice />
  {/if}

  <nav class="tabs">
    {#each tabs as tab}
      <a href={tab.href} class:active={page.url.pathname === tab.href} aria-label={tab.label}>
        <Icon name={tab.icon} size={26} />
      </a>
    {/each}
  </nav>
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100dvh;
  }
  .topbar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: calc(env(safe-area-inset-top) + 10px) var(--gutter) 10px;
    background: var(--topbar);
  }
  .logo {
    width: 30px;
    height: 30px;
  }
  .title {
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }
  .spacer {
    flex: 1;
  }
  .battery {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 14px;
  }
  .status {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--muted);
  }
  .status.connected {
    background: var(--green);
  }
  .status.offline,
  .status.unauthorized {
    background: var(--red);
  }
  /* Flex, so the icon isn't placed on a text line (line-height would shift it off-center). */
  .help {
    display: flex;
  }
  main {
    flex: 1;
    overflow-y: auto;
    padding-bottom: 24px;
  }
  .tabs {
    display: flex;
    background: var(--topbar);
    padding-bottom: env(safe-area-inset-bottom);
  }
  .tabs a {
    flex: 1;
    display: flex;
    justify-content: center;
    padding: 12px 0;
    color: var(--text);
    border-top: 2px solid transparent;
  }
  .tabs a.active {
    color: var(--accent);
    border-top-color: var(--accent);
  }
</style>
