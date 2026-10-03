// Update check against the latest GitHub release tag

import tauriConf from "../../src-tauri/tauri.conf.json";
import { agent } from "./agent.svelte";

const LATEST_URL = "https://api.github.com/repos/nailuj05/framemate/releases/latest";
const STORAGE_KEY = "framemate.updates";
const TIMEOUT_MS = 10_000;

/** Same source as the APK's versionName. */
export const APP_VERSION: string = tauriConf.version;

export interface Release {
  version: string;
  /** Release page on GitHub (APK + Flatpak downloads). */
  url: string;
}

function parse(version: string): number[] | null {
  const m = /^v?(\d+)\.(\d+)\.(\d+)/.exec(version.trim());
  return m ? m.slice(1).map(Number) : null;
}

/** True if `a` is a higher x.y.z than `b`; false if either doesn't parse. */
export function isNewer(a: string, b: string): boolean {
  const [x, y] = [parse(a), parse(b)];
  if (!x || !y) return false;
  for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return x[i] > y[i];
  return false;
}

function loadAutoCheck(): boolean {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}").autoCheck !== false;
  } catch {
    return true;
  }
}

class Updates {
  autoCheck = $state(loadAutoCheck());
  latest = $state<Release | null>(null);
  checking = $state(false);
  error = $state<string | null>(null);
  /** The startup notice was closed for this session. */
  dismissed = $state(false);

  appOutdated = $derived(!!this.latest && isNewer(this.latest.version, APP_VERSION));
  /** Only known while connected. */
  agentOutdated = $derived(
    !!this.latest && !!agent.state && isNewer(this.latest.version, agent.state.agent.version),
  );
  available = $derived(this.appOutdated || this.agentOutdated);

  setAutoCheck(enabled: boolean) {
    this.autoCheck = enabled;
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify({ autoCheck: enabled }));
    } catch {}
  }

  async check() {
    if (this.checking) return;
    this.checking = true;
    this.error = null;
    try {
      const response = await fetch(LATEST_URL, {
        headers: { Accept: "application/vnd.github+json" },
        signal: AbortSignal.timeout(TIMEOUT_MS),
      });
      if (!response.ok) throw new Error(`GitHub answered ${response.status}`);
      const release = await response.json();
      const version = String(release.tag_name ?? "").replace(/^v/, "");
      if (!parse(version)) throw new Error(`unexpected release tag "${release.tag_name}"`);
      this.latest = { version, url: release.html_url };
    } catch (e) {
      // TypeError = network failure; TimeoutError = no answer within TIMEOUT_MS.
      const unreachable = e instanceof TypeError || (e instanceof Error && e.name === "TimeoutError");
      this.error = unreachable ? "Couldn't reach GitHub" : e instanceof Error ? e.message : String(e);
    } finally {
      this.checking = false;
    }
  }
}

export const updates = new Updates();
