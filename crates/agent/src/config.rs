//! Configuration from environment variables. XDG paths, so it works on the host and inside
//! the Flatpak (where XDG_CONFIG_HOME points into ~/.var/app/<id>/config).

use std::io::Read;
use std::net::SocketAddr;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;

use anyhow::Context;

use crate::stream::StreamConfig;

pub struct Config {
    pub listen: SocketAddr,
    pub cdp_url: String,
    pub token: String,
    pub power_supply_dir: PathBuf,
    pub stream: StreamConfig,
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
            power_supply_dir: env_or("FRAMEMATE_POWER_SUPPLY_DIR", "/sys/class/power_supply").into(),
            stream: StreamConfig {
                source_device: env_or("FRAMEMATE_STREAM_SOURCE", "/dev/video99").into(),
                encoder_device: std::env::var("FRAMEMATE_STREAM_ENCODER").unwrap_or_else(|_| default_encoder().into()),
                fps,
                bitrate,
            },
        })
    }
}

/// videoN numbers depend on driver probe order; the udev symlink is stable.
fn default_encoder() -> &'static str {
    if std::path::Path::new("/dev/video-enc0").exists() { "/dev/video-enc0" } else { "/dev/video23" }
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
        .open(&path)
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
}
