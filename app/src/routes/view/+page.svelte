<script lang="ts">
  // Live headset view through the agent's player (crates/agent/src/player.js), shared with its
  // /stream page: WebCodecs when available (low latency), MSE otherwise. The agent only captures
  // while someone watches, so leaving this page stops it.
  // The agent only captures while a viewer is connected, so leaving this tab stops it.
  // The app is portrait-only; fullscreen switches to landscape (natively on Android).
  import { onDestroy, onMount } from "svelte";
  import { pushState } from "$app/navigation";
  import { page } from "$app/state";
  import { agent } from "$lib/agent.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import Row from "$lib/components/Row.svelte";
  import Section from "$lib/components/Section.svelte";
  import { startPlayer } from "../../../../crates/agent/src/player.js";

  let video: HTMLVideoElement;
  let canvas: HTMLCanvasElement;
  let status = $state("Connecting…");
  let player: { close(): void } | undefined;
  const stats = $derived(agent.state?.stream);
  const fullscreen = $derived(!!page.state.fullscreen);

  // Fullscreen is a shallow history entry: Android's back button (and Esc on desktop)
  // leave it like any other navigation.
  function enterFullscreen() {
    pushState("", { fullscreen: true });
  }

  function applyFullscreen(enabled: boolean) {
    if (window.FrameMateAndroid) {
      window.FrameMateAndroid.setFullscreen(enabled);
    } else if (enabled && !document.fullscreenElement) {
      document.documentElement.requestFullscreen?.().catch(() => {});
    } else if (!enabled && document.fullscreenElement) {
      document.exitFullscreen().catch(() => {});
    }
  }

  $effect(() => applyFullscreen(fullscreen));

  // Leaving browser fullscreen via Esc: drop our history entry too.
  function onFullscreenChange() {
    if (!document.fullscreenElement && page.state.fullscreen) history.back();
  }

  function play() {
    player?.close();
    player = startPlayer({
      url: agent.socketUrl("/api/stream/ws"),
      video,
      canvas,
      onStatus: (text: string) => (status = text),
    });
  }

  // In the background the socket dies or, worse, keeps the Frame capturing for nobody:
  // stop it, and start fresh (new keyframe, no stale buffer) when the app comes back.
  function onVisibilityChange() {
    player?.close();
    player = undefined;
    if (document.visibilityState === "visible") {
      status = "Connecting…";
      play();
    }
  }

  onMount(play);
  onDestroy(() => {
    if (fullscreen) applyFullscreen(false);
    player?.close();
  });
</script>

<svelte:document onfullscreenchange={onFullscreenChange} onvisibilitychange={onVisibilityChange} />

<div class="player" class:fullscreen>
  <video bind:this={video} autoplay muted playsinline hidden></video>
  <canvas bind:this={canvas} hidden></canvas>
  {#if status}<div class="overlay">{status}</div>{/if}
  {#if fullscreen}
    <button class="control exit" onclick={() => history.back()} aria-label="Exit fullscreen">
      <Icon name="back" size={22} />
    </button>
  {:else}
    <button class="control enter" onclick={enterFullscreen} aria-label="Fullscreen">
      <svg viewBox="0 0 36 36" width="22" height="22" fill="currentColor" aria-hidden="true">
        <path d="M4 4h11v4H8v7H4zM21 4h11v11h-4V8h-7zM4 21h4v7h7v4H4zM28 21h4v11H21v-4h7z" />
      </svg>
    </button>
  {/if}
</div>

<Section title="Mirroring">
  <Row label="Resolution" value={stats ? `${stats.width}×${stats.height}` : null} />
  <Row label="Frame rate" value={stats ? `${stats.fps.toFixed(0)} fps (headset ${stats.source_fps.toFixed(0)})` : null} />
  <Row label="Bitrate" value={stats ? `${(stats.kbit_per_sec / 1000).toFixed(1)} Mbit/s` : null} />
  <Row label="Viewers" value={stats?.viewers} />
</Section>
<p class="hint muted">Mirroring only runs while someone watches. A sleeping headset shows black frames.</p>

<style>
  .player {
    position: relative;
    background: #000;
    aspect-ratio: 16 / 9;
  }
  video,
  canvas {
    width: 100%;
    height: 100%;
    display: block;
    object-fit: contain;
  }
  [hidden] {
    display: none;
  }
  .player.fullscreen {
    position: fixed;
    inset: 0;
    z-index: 10;
    aspect-ratio: auto;
  }
  .control {
    position: absolute;
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border: 0;
    border-radius: 2px;
    background: rgb(0 0 0 / 0.5);
    color: #fff;
  }
  .enter {
    right: 8px;
    bottom: 8px;
  }
  .exit {
    left: 12px;
    top: 12px;
  }
  .overlay {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: var(--muted);
  }
  .hint {
    padding: 12px var(--gutter);
    font-size: 13px;
  }
</style>
