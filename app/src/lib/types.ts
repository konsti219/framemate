// Shape of the agent's `/api/ws` / `/api/state` JSON (see crates/agent/src/hub.rs and
// shim.js). Steam topics are loosely typed on the agent side; every field may be
// missing when Steam isn't connected or an API changed, hence the many optionals.

export interface AgentState {
  agent: { version: string; hostname: string; started_at_ms: number; updated_at_ms: number };
  steam: {
    connected: boolean;
    error: string | null;
    last_event_ms: number | null;
    topics: SteamTopics;
    topic_errors: Record<string, string>;
  };
  power: { battery: KernelBattery | null; external_power: boolean | null } | null;
  steamos: { available: boolean; error: string | null; properties: Record<string, unknown> };
  stream: StreamStats | null;
}

export interface SteamTopics {
  battery?: {
    has_battery: boolean;
    level: number;
    seconds_remaining: number;
    /** 1 = on battery, 2 = on AC */
    ac_state: number;
    battery_state: number;
  };
  vr_devices?: VrDevice[];
  vr_state?: { hmd_activity?: number; scene_appid?: number | null; scene_app_name?: string | null };
  running_apps?: { appid: number; name: string | null }[];
  last_app_event?: { appid: number; name: string | null; running: boolean; at_ms: number };
  downloads?: DownloadItem[];
  download_overview?: DownloadOverview;
  network?: {
    connected: boolean | null;
    internet: boolean | null;
    steam: boolean | null;
    wifi_enabled: boolean | null;
    interfaces: { name: string; rx_bytes_per_sec: number; tx_bytes_per_sec: number }[];
  };
  perf?: {
    battery_temp_c: number | null;
    tdp_limit_w: number | null;
    fps_limit: number | null;
    low_latency_mode: boolean | null;
  };
  system_info?: {
    os: string;
    os_version: string;
    os_variant: string;
    os_build: string;
    kernel: string;
    hostname: string;
    steam_version: number;
    steam_build_date: string;
    gpu: string;
    gpu_driver: string;
    cpu_cores: number;
    ram_mb: number;
  };
  user?: { account_name: string | null; persona_name: string | null; persona_state: number | null };
}

export interface VrDevice {
  path: string;
  model: string | null;
  serial: string | null;
  /** 1 = HMD, 2 = controller */
  device_class: number | null;
  /** 0..1 */
  battery: number | null;
  charging: boolean | null;
  connected: boolean | null;
  /** Last time SteamVR reported it connected (agent-side memory). */
  last_seen_ms?: number | null;
  /** Not currently known to SteamVR (e.g. asleep since a reboot); last known state. */
  remembered?: boolean;
}

export interface DownloadItem {
  appid: number;
  name: string | null;
  queue_index: number;
  active: boolean;
  paused: boolean;
  completed: boolean;
  completed_time: number;
  percent: number | null;
  error: string | null;
}

export interface DownloadOverview {
  appid: number | null;
  name?: string | null;
  state?: string | null;
  paused?: boolean;
  percent?: number;
  bytes_per_sec?: number;
  eta_sec?: number;
  is_workshop?: boolean;
  is_shader?: boolean;
}

export interface KernelBattery {
  status: string | null;
  health: string | null;
  capacity_percent: number | null;
  power_w: number | null;
  temp_c: number | null;
  cycle_count: number | null;
}

export interface StreamStats {
  viewers: number;
  width: number;
  height: number;
  source_fps: number;
  fps: number;
  kbit_per_sec: number;
  encode_ms: number;
}
