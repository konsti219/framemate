//! Runtime configuration from environment variables. Paths follow XDG so the same
//! binary works on the host and inside a Flatpak sandbox (where XDG_CONFIG_HOME
//! points into ~/.var/app/<id>/config).

use std::io::Read;
use std::net::SocketAddr;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;

use anyhow::Context;

use crate::stream::StreamConfig;

pub struct Config {
    pub listen: SocketAddr,
    /// Base URL of Steam's CEF remote debugging HTTP endpoint.
    pub cdp_url: String,
    pub token: String,
    pub power_supply_dir: PathBuf,
    pub stream: StreamConfig,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let listen = env_or("FRAMEMATE_LISTEN", "0.0.0.0:7380")
            .parse()
            .context("FRAMEMATE_LISTEN must be host:port")?;
        let token = match std::env::var("FRAMEMATE_TOKEN") {
            Ok(token) if !token.is_empty() => token,
            _ => load_or_create_token()?,
        };
        Ok(Self {
            listen,
            cdp_url: env_or("FRAMEMATE_CDP", "http://127.0.0.1:8080"),
            token,
            power_supply_dir: env_or("FRAMEMATE_POWER_SUPPLY_DIR", "/sys/class/power_supply").into(),
            stream: StreamConfig {
                source_device: env_or("FRAMEMATE_STREAM_SOURCE", "/dev/video99").into(),
                encoder_device: env_or("FRAMEMATE_STREAM_ENCODER", "/dev/video23"),
                fps: env_or("FRAMEMATE_STREAM_FPS", "30").parse().context("FRAMEMATE_STREAM_FPS")?,
                bitrate: env_or("FRAMEMATE_STREAM_BITRATE", "6000000").parse().context("FRAMEMATE_STREAM_BITRATE")?,
            },
        })
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_owned())
}

fn config_dir() -> anyhow::Result<PathBuf> {
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => PathBuf::from(std::env::var_os("HOME").context("HOME not set")?).join(".config"),
    };
    Ok(base.join("framemate"))
}

/// The API token lives in `$XDG_CONFIG_HOME/framemate/token` and is created on first run.
fn load_or_create_token() -> anyhow::Result<String> {
    let path = config_dir()?.join("token");
    if let Ok(token) = std::fs::read_to_string(&path) {
        let token = token.trim().to_owned();
        if !token.is_empty() {
            return Ok(token);
        }
    }
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();

    let dir = path.parent().unwrap();
    std::fs::create_dir_all(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&path)
        .with_context(|| format!("writing {}", path.display()))?;
    std::io::Write::write_all(&mut file, token.as_bytes())?;
    Ok(token)
}

pub fn hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|h| h.trim().to_owned())
        .unwrap_or_default()
}
