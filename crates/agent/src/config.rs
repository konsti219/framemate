//! Configuration from environment variables. XDG paths, so it works on the host and inside
//! the Flatpak (where XDG_CONFIG_HOME points into ~/.var/app/<id>/config).

use std::io::Read;
use std::net::{IpAddr, SocketAddr};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;

use anyhow::Context;

use crate::stream::StreamConfig;

pub struct Config {
    pub listen: SocketAddr,
    pub cdp_url: String,
    pub token: String,
    /// Source networks allowed to connect; empty allows everyone.
    pub allow: Vec<Cidr>,
    pub power_supply_dir: PathBuf,
    pub stream: StreamConfig,
    /// Accept clients from outside the local network (see access.rs).
    pub allow_remote: bool,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        // Dual-stack (see server.rs), so IPv4 clients work too.
        let listen = env_or("FRAMEMATE_LISTEN", "[::]:7380")
            .parse()
            .context("FRAMEMATE_LISTEN must be host:port")?;
        let token = match std::env::var("FRAMEMATE_TOKEN") {
            Ok(token) if !token.is_empty() => token,
            _ => load_or_create_token()?,
        };
        let token = normalize_token(&token);
        let fps: u32 = env_or("FRAMEMATE_STREAM_FPS", "30").parse().context("FRAMEMATE_STREAM_FPS")?;
        anyhow::ensure!((1..=120).contains(&fps), "FRAMEMATE_STREAM_FPS must be 1–120");
        let bitrate: u32 = env_or("FRAMEMATE_STREAM_BITRATE", "6000000").parse().context("FRAMEMATE_STREAM_BITRATE")?;
        anyhow::ensure!(bitrate > 0, "FRAMEMATE_STREAM_BITRATE must be > 0");
        Ok(Self {
            listen,
            cdp_url: env_or("FRAMEMATE_CDP", "http://127.0.0.1:8080"),
            token,
            allow: parse_allow(&env_or("FRAMEMATE_ALLOW", ""))?,
            power_supply_dir: env_or("FRAMEMATE_POWER_SUPPLY_DIR", "/sys/class/power_supply").into(),
            stream: StreamConfig {
                source_device: env_or("FRAMEMATE_STREAM_SOURCE", "/dev/video99").into(),
                encoder_device: std::env::var("FRAMEMATE_STREAM_ENCODER").unwrap_or_else(|_| default_encoder().into()),
                fps,
                bitrate,
            },
            allow_remote: matches!(env_or("FRAMEMATE_ALLOW_REMOTE", "").as_str(), "1" | "true" | "yes"),
        })
    }
}

/// videoN numbers depend on driver probe order; the udev symlink is stable.
fn default_encoder() -> &'static str {
    if std::path::Path::new("/dev/video-enc0").exists() { "/dev/video-enc0" } else { "/dev/video23" }
}

/// `FRAMEMATE_ALLOW`: comma-separated networks (`100.64.0.0/10`, `fd7a::/16`, a bare address), plus the
/// shortcuts `loopback` and `tailscale`.
fn parse_allow(value: &str) -> anyhow::Result<Vec<Cidr>> {
    let mut out = Vec::new();
    for item in value.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let expanded: &[&str] = match item {
            "loopback" => &["127.0.0.0/8", "::1/128"],
            "tailscale" => &["100.64.0.0/10", "fd7a:115c:a1e0::/48"],
            other => &[other],
        };
        for net in expanded {
            out.push(Cidr::parse(net).with_context(|| format!("FRAMEMATE_ALLOW: bad network {net:?}"))?);
        }
    }
    Ok(out)
}

/// A network prefix such as `100.64.0.0/10`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cidr {
    addr: IpAddr,
    prefix: u8,
}

impl std::fmt::Display for Cidr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.addr, self.prefix)
    }
}

impl Cidr {
    pub fn parse(s: &str) -> Option<Self> {
        let (addr, prefix) = match s.split_once('/') {
            Some((a, p)) => (a.parse::<IpAddr>().ok()?, Some(p.parse::<u8>().ok()?)),
            None => (s.parse::<IpAddr>().ok()?, None),
        };
        let max = if addr.is_ipv4() { 32 } else { 128 };
        let prefix = prefix.unwrap_or(max);
        (prefix <= max).then_some(Self { addr, prefix })
    }

    pub fn contains(&self, ip: IpAddr) -> bool {
        // Dual-stack sockets report IPv4 clients as ::ffff:a.b.c.d
        let ip = ip.to_canonical();
        match (self.addr, ip) {
            (IpAddr::V4(net), IpAddr::V4(ip)) => {
                let mask = u32::MAX.checked_shl(32 - u32::from(self.prefix)).unwrap_or(0);
                u32::from(net) & mask == u32::from(ip) & mask
            }
            (IpAddr::V6(net), IpAddr::V6(ip)) => {
                let mask = u128::MAX.checked_shl(128 - u32::from(self.prefix)).unwrap_or(0);
                u128::from(net) & mask == u128::from(ip) & mask
            }
            _ => false,
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_owned())
}

fn config_dir() -> anyhow::Result<PathBuf> {
    xdg_dir("XDG_CONFIG_HOME", ".config")
}

pub fn state_dir() -> anyhow::Result<PathBuf> {
    xdg_dir("XDG_STATE_HOME", ".local/state")
}

fn xdg_dir(var: &str, fallback: &str) -> anyhow::Result<PathBuf> {
    let base = match std::env::var_os(var) {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => PathBuf::from(std::env::var_os("HOME").context("HOME not set")?).join(fallback),
    };
    Ok(base.join("framemate"))
}

/// Crockford base32: no I, L, O, U, so tokens survive being read aloud or typed on a phone.
const TOKEN_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const TOKEN_LEN: usize = 10; // 50 bits

/// Canonical form for comparison: case, dashes and spaces don't matter, and the
/// look-alikes O/I/L read as 0/1 (Crockford decoding).
pub fn normalize_token(token: &str) -> String {
    token
        .chars()
        .filter(|c| !matches!(c, '-' | ' '))
        .map(|c| match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            c => c,
        })
        .collect()
}

/// `ABCDE-FGHJK` for display.
pub fn format_token(token: &str) -> String {
    let (a, b) = token.split_at(token.len() / 2);
    format!("{a}-{b}")
}

fn is_current_format(token: &str) -> bool {
    token.len() == TOKEN_LEN && token.bytes().all(|b| TOKEN_ALPHABET.contains(&b))
}

/// `$XDG_CONFIG_HOME/framemate/token`, created on first run; other formats are replaced.
pub fn load_or_create_token() -> anyhow::Result<String> {
    let path = config_dir()?.join("token");
    if let Ok(token) = std::fs::read_to_string(&path) {
        let token = normalize_token(token.trim());
        if is_current_format(&token) {
            return Ok(token);
        }
    }
    write_new_token(&path)
}

/// Replaces the token; the running agent only reads it at startup.
pub fn rotate_token() -> anyhow::Result<String> {
    write_new_token(&config_dir()?.join("token"))
}

fn write_new_token(path: &std::path::Path) -> anyhow::Result<String> {
    let mut bytes = [0u8; TOKEN_LEN];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    // 256 is a multiple of 32, so `% 32` is unbiased.
    let token: String = bytes.iter().map(|b| TOKEN_ALPHABET[(b % 32) as usize] as char).collect();

    let dir = path.parent().unwrap();
    std::fs::create_dir_all(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("writing {}", path.display()))?;
    std::io::Write::write_all(&mut file, format_token(&token).as_bytes())?;
    tracing::info!("generated a new API token");
    Ok(token)
}

pub fn hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|h| h.trim().to_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_compare_leniently() {
        assert_eq!(normalize_token("abcde-fghjk"), "ABCDEFGHJK");
        assert_eq!(normalize_token("0O1Il-ab cd"), "00111ABCD");
        assert_eq!(format_token("ABCDEFGHJK"), "ABCDE-FGHJK");
        assert!(is_current_format("ABCDEFGHJK"));
        assert!(!is_current_format("cbad37e74cbd250279126bbbca61a0a9"));
    }

    #[test]
    fn allow_list_matches_networks() {
        let allow = parse_allow("loopback, tailscale, 192.168.0.13").unwrap();
        let ok = |s: &str| allow.iter().any(|c| c.contains(s.parse().unwrap()));
        assert!(ok("127.0.0.1"));
        assert!(ok("::1"));
        assert!(ok("100.64.0.13"));
        assert!(ok("100.127.255.254"));
        assert!(ok("::ffff:100.64.0.13"));
        assert!(ok("fd7a:115c:a1e0::353a:dc01"));
        assert!(ok("192.168.0.13"));
        assert!(!ok("192.168.0.14"));
        assert!(!ok("100.128.0.1"));
        assert!(!ok("10.0.0.1"));
        assert!(parse_allow("").unwrap().is_empty());
        assert!(parse_allow("10.0.0.0/33").is_err());
        assert!(Cidr::parse("0.0.0.0/0").unwrap().contains("8.8.8.8".parse().unwrap()));
    }
}
