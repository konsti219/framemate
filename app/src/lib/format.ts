export function percent(fraction: number | null | undefined): string {
  return fraction == null ? "–" : `${Math.round(fraction * 100)}%`;
}

export function bytes(n: number | null | undefined): string {
  if (n == null) return "–";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  while (n >= 1000 && i < units.length - 1) {
    n /= 1000;
    i++;
  }
  return `${n.toFixed(i && n < 100 ? 1 : 0)} ${units[i]}`;
}

export function duration(seconds: number | null | undefined): string {
  if (seconds == null || seconds < 0) return "–";
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (h) return `${h}h ${m}m`;
  if (m) return `${m}m`;
  return `${Math.floor(seconds)}s`;
}

export function since(ms: number | null | undefined): string {
  return ms ? duration((Date.now() - ms) / 1000) : "–";
}

/** Steam CDN artwork for an app; tools and runtimes have none (handle onerror). */
export function appArt(appid: number, kind: "header" | "capsule" = "header"): string {
  const file = kind === "header" ? "header.jpg" : "capsule_184x69.jpg";
  return `https://cdn.cloudflare.steamstatic.com/steam/apps/${appid}/${file}`;
}

/** OpenVR EDeviceActivityLevel. */
export function hmdActivity(level: number | undefined): string {
  return (
    { [-1]: "Unknown", 0: "Idle", 1: "In use", 2: "In use", 3: "Standby", 4: "Idle" } as Record<number, string>
  )[level ?? -1] ?? "Unknown";
}
