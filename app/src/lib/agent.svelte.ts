// Connection to the agent: latest state from `/api/ws`, reconnects on its own.

import type { AgentState } from "./types";

const STORAGE_KEY = "framemate.connection";
const DEFAULT_PORT = 7380;
const RETRY_MS = 3000;
/** A connect that hasn't opened by then is given up (a sleeping Frame never answers the SYN). */
const CONNECT_TIMEOUT_MS = 8000;
/** The agent pushes at least every ~10 s (power poll); silence beyond this means a dead socket. */
const SILENCE_MS = 25000;

export interface ConnectionSettings {
  /** Hostname or IP, optionally with `:port`. */
  host: string;
  token: string;
}

/** `unauthorized`: the agent answered but rejected the token. */
export type ConnectionStatus = "unconfigured" | "connecting" | "connected" | "offline" | "unauthorized";

function loadSettings(): ConnectionSettings {
  try {
    const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null");
    if (saved?.host !== undefined && saved?.token !== undefined) return saved;
  } catch {}
  return { host: "frame.local", token: "" };
}

class Agent {
  settings = $state<ConnectionSettings>(loadSettings());
  state = $state<AgentState | null>(null);
  status = $state<ConnectionStatus>("unconfigured");

  /** When the last message arrived; tells live data from a snapshot left over from before. */
  receivedAt = $state(0);

  #socket: WebSocket | null = null;
  #retry: ReturnType<typeof setTimeout> | undefined;
  #watchdog: ReturnType<typeof setTimeout> | undefined;

  /** True while `state` is being kept current by an open connection. */
  get live() {
    return this.status === "connected";
  }

  get configured() {
    return this.settings.host.trim() !== "" && this.settings.token.trim() !== "";
  }

  get authority() {
    const host = this.settings.host.trim();
    if (host.startsWith("[")) return /\]:\d+$/.test(host) ? host : `${host}:${DEFAULT_PORT}`;
    // A bare IPv6 address needs brackets before a port can follow.
    if ((host.match(/:/g) ?? []).length > 1) return `[${host}]:${DEFAULT_PORT}`;
    return /:\d+$/.test(host) ? host : `${host}:${DEFAULT_PORT}`;
  }

  socketUrl(path: string) {
    return `ws://${this.authority}${path}?token=${encodeURIComponent(this.settings.token.trim())}`;
  }

  save(settings: ConnectionSettings) {
    this.settings = { host: settings.host.trim(), token: settings.token.trim() };
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(this.settings));
    } catch {}
    this.state = null;
    this.status = "connecting";
    this.connect();
  }

  connect() {
    this.#close();
    if (!this.configured) {
      this.status = "unconfigured";
      return;
    }
    // Retries keep showing the last failure instead of flickering back to "connecting".
    if (this.status !== "offline" && this.status !== "unauthorized") this.status = "connecting";
    const socket = new WebSocket(this.socketUrl("/api/ws"));
    this.#socket = socket;
    let opened = false;
    const lost = () => this.#lost(socket, opened);
    socket.onmessage = event => {
      if (this.#socket !== socket) return;
      this.state = JSON.parse(event.data);
      this.status = "connected";
      this.receivedAt = Date.now();
      this.#arm(socket, SILENCE_MS, lost);
    };
    socket.onopen = () => (opened = true);
    socket.onclose = lost;
    this.#arm(socket, CONNECT_TIMEOUT_MS, lost);
  }

  /**
   * Fresh snapshot after the app was in the background: Android freezes the WebView, so the old
   * socket may be dead without ever having fired `onclose` and would keep showing old data.
   * The current state stays visible (marked stale via `live`) until the new one arrives.
   */
  refresh() {
    if (!this.configured) return;
    if (this.status === "connected") this.status = "connecting";
    this.connect();
  }

  /** Give up on `socket` if nothing arrives within `ms`. */
  #arm(socket: WebSocket, ms: number, lost: () => void) {
    clearTimeout(this.#watchdog);
    this.#watchdog = setTimeout(() => {
      if (this.#socket !== socket) return;
      // On a dead TCP connection onclose can take minutes; don't wait for it.
      socket.onclose = null;
      socket.close();
      lost();
    }, ms);
  }

  async #lost(socket: WebSocket, opened: boolean) {
    if (this.#socket !== socket) return; // replaced by a newer connection
    this.#socket = null;
    clearTimeout(this.#watchdog);
    // A rejected upgrade looks like any other failure to the WebSocket API; ask over HTTP.
    const status = opened ? "offline" : await this.#probe();
    if (this.#socket || this.#retry !== undefined) return; // reconnected meanwhile
    this.status = status;
    this.#retry = setTimeout(() => {
      this.#retry = undefined;
      this.connect();
    }, RETRY_MS);
  }

  /** Wrong token (401) vs. unreachable; needs CORS on /api/state. */
  async #probe(): Promise<ConnectionStatus> {
    try {
      const url = `http://${this.authority}/api/state?token=${encodeURIComponent(this.settings.token.trim())}`;
      const response = await fetch(url, { signal: AbortSignal.timeout(RETRY_MS) });
      return response.status === 401 ? "unauthorized" : "offline";
    } catch {
      return "offline";
    }
  }

  #close() {
    clearTimeout(this.#watchdog);
    clearTimeout(this.#retry);
    this.#retry = undefined;
    const socket = this.#socket;
    this.#socket = null;
    socket?.close();
  }
}

export const agent = new Agent();

/** Android 17+ blocks the LAN until "Nearby devices" is granted; fails like a timeout. */
export function localNetworkBlocked() {
  return window.FrameMateAndroid?.localNetworkAllowed?.() === false;
}
