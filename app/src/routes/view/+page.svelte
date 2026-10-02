<script lang="ts">
  // Live headset view. Same protocol as the agent's /stream page: a JSON header with the
  // codec string, an fMP4 init segment, then one moof+mdat per frame, fed into MSE.
  // The agent only captures while a viewer is connected, so leaving this tab stops it.
  // The app is portrait-only; fullscreen switches to landscape (natively on Android).
  import { onDestroy, onMount } from "svelte";
  import { pushState } from "$app/navigation";
  import { page } from "$app/state";
  import { agent } from "$lib/agent.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import Row from "$lib/components/Row.svelte";
  import Section from "$lib/components/Section.svelte";

  let video: HTMLVideoElement;
  let status = $state("Connecting…");
  let socket: WebSocket | null = null;
  let retry: ReturnType<typeof setTimeout> | undefined;
  let closed = false;
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

  function connect() {
    const ws = new WebSocket(agent.socketUrl("/api/stream/ws"));
    socket = ws;
    ws.binaryType = "arraybuffer";
    let buffer: SourceBuffer | null = null;
    const queue: ArrayBuffer[] = [];
    const pump = () => {
      if (buffer && !buffer.updating && queue.length) buffer.appendBuffer(queue.shift()!);
    };

    ws.onmessage = event => {
      if (typeof event.data !== "string") {
        queue.push(event.data);
        pump();
        return;
      }
      const { codec } = JSON.parse(event.data);
      const type = `video/mp4; codecs="${codec}"`;
      if (!MediaSource.isTypeSupported(type)) {
        status = `This device can't play ${codec}`;
        closed = true;
        ws.close();
        return;
      }
      const source = new MediaSource();
      video.src = URL.createObjectURL(source);
      source.addEventListener(
        "sourceopen",
        () => {
          buffer = source.addSourceBuffer(type);
          buffer.mode = "sequence";
          buffer.addEventListener("updateend", () => {
            keepLive(buffer!);
            pump();
          });
          pump();
        },
        { once: true },
      );
      status = "";
    };
    ws.onclose = () => {
      if (closed) return;
      status = "Reconnecting…";
      retry = setTimeout(connect, 2000);
    };
  }

  // Catch up by playing slightly faster; seeking on every frame restarts decoding at the
  // last keyframe and drops the picture to a few fps.
  function keepLive(buffer: SourceBuffer) {
    if (buffer.updating || !buffer.buffered.length) return;
    const start = buffer.buffered.start(0);
    const end = buffer.buffered.end(buffer.buffered.length - 1);
    const behind = end - video.currentTime;
    if (video.currentTime < start || behind > 2) video.currentTime = Math.max(start, end - 0.2);
    video.playbackRate = behind > 0.3 ? 1.1 : 1.0;
    if (video.paused) video.play().catch(() => {});
    if (video.currentTime - start > 10) buffer.remove(start, video.currentTime - 5);
  }

  onMount(connect);
  onDestroy(() => {
    if (fullscreen) applyFullscreen(false);
    closed = true;
    clearTimeout(retry);
    socket?.close();
  });
</script>

<svelte:document onfullscreenchange={onFullscreenChange} />

<div class="player" class:fullscreen>
  <video bind:this={video} autoplay muted playsinline></video>
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

<Section title="Stream">
  <Row label="Resolution" value={stats ? `${stats.width}×${stats.height}` : null} />
  <Row label="Frame rate" value={stats ? `${stats.fps.toFixed(0)} fps (headset ${stats.source_fps.toFixed(0)})` : null} />
  <Row label="Bitrate" value={stats ? `${(stats.kbit_per_sec / 1000).toFixed(1)} Mbit/s` : null} />
  <Row label="Viewers" value={stats?.viewers} />
</Section>
<p class="hint muted">The headset only renders this view while someone watches. A sleeping headset shows black frames.</p>

<style>
  .player {
    position: relative;
    background: #000;
    aspect-ratio: 16 / 9;
  }
  video {
    width: 100%;
    height: 100%;
    display: block;
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
