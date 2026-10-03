//! `check`: self check of the running agent, then prints what the app needs. Also run by
//! `install-service`. Same sandbox as the service, so permissions are checked too.

use std::ffi::CString;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::config::{self, Config};

/// The service needs a moment after `RestartUnit` (`flatpak run` startup).
const STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
/// Steam data follows shortly after the agent is up (the shim injects, then reports).
const STEAM_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn run() -> anyhow::Result<()> {
    let config = Config::from_env()?;
    let addr = match config.listen.ip() {
        ip if ip.is_unspecified() => SocketAddr::new(Ipv4Addr::LOCALHOST.into(), config.listen.port()),
        _ => config.listen,
    };
    let authority = addr.to_string();
    println!("Checking the agent on {authority}…");

    let started = Instant::now();
    loop {
        match crate::cdp::http_get(&authority, "/healthz").await {
            Ok(_) => break,
            Err(e) if started.elapsed() > STARTUP_TIMEOUT => {
                fail(&format!("agent not answering ({e:#})"));
                println!("  Logs: journalctl --user -n 50 _COMM=framemate-agent");
                anyhow::bail!("agent not running");
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(500)).await,
        }
    }
    ok(&format!("agent answers on port {}", config.listen.port()));

    let state_path = format!("/api/state?token={}", config.token);
    let mut state = match fetch_state(&authority, &state_path).await {
        Ok(state) => state,
        Err(e) => {
            // Usually a token file that changed after the service started.
            fail(&format!("token rejected or state unreadable ({e:#}); restart the service"));
            anyhow::bail!("agent unusable");
        }
    };
    ok("token accepted");

    let started = Instant::now();
    while state["steam"]["connected"] != true && started.elapsed() < STEAM_TIMEOUT {
        tokio::time::sleep(Duration::from_secs(1)).await;
        state = fetch_state(&authority, &state_path).await.unwrap_or(state);
    }
    if state["steam"]["connected"] == true {
        ok("connected to Steam");
    } else {
        let error = state["steam"]["error"].as_str().unwrap_or("not connected yet");
        warn(&format!(
            "no Steam data: {error}. Normal for a few seconds after boot or while Steam isn't \
             running; battery and Mirroring work without it"
        ));
    }

    if state["power"]["battery"].is_object() {
        ok("battery readings");
    } else {
        warn("no battery found in /sys/class/power_supply");
    }
    if state["steamos"]["available"] == true {
        ok("SteamOS performance settings");
    } else {
        let error = state["steamos"]["error"].as_str().unwrap_or("unavailable");
        warn(&format!("SteamOS performance settings: {error}"));
    }

    // Only check access: opening the source would wake SteamVR's v4l2cam.
    let devices = [
        (config.stream.source_device.as_path(), "headset view"),
        (Path::new(&config.stream.encoder_device), "H.264 encoder"),
    ];
    let missing: Vec<String> = devices
        .iter()
        .filter(|(path, _)| !accessible(path))
        .map(|(path, what)| format!("{what} {}", path.display()))
        .collect();
    if missing.is_empty() {
        ok("Mirroring devices");
    } else {
        warn(&format!("Mirroring unavailable, can't access {}", missing.join(", ")));
    }

    let ip = lan_ip().map(|ip| format!(" (or {ip})")).unwrap_or_default();
    println!();
    println!("Connect the app to: {}.local{ip}", config::hostname());
    println!("Token: {}", config::format_token(&config.token));
    Ok(())
}

async fn fetch_state(authority: &str, path: &str) -> anyhow::Result<Value> {
    Ok(serde_json::from_slice(&crate::cdp::http_get(authority, path).await?)?)
}

fn accessible(path: &Path) -> bool {
    CString::new(path.as_os_str().as_encoded_bytes())
        .is_ok_and(|p| unsafe { libc::access(p.as_ptr(), libc::R_OK | libc::W_OK) } == 0)
}

/// The address of the interface the default route uses. `connect` on UDP sends nothing.
fn lan_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?; // TEST-NET-1, never actually contacted
    Some(socket.local_addr().ok()?.ip())
}

fn ok(msg: &str) {
    println!("  [ok]   {msg}");
}

fn warn(msg: &str) {
    println!("  [warn] {msg}");
}

fn fail(msg: &str) {
    println!("  [err]  {msg}");
}
