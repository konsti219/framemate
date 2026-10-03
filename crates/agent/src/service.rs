//! `install-service` / `uninstall-service`: a systemd **user** unit that starts the agent
//! with the session. Flatpaks can't autostart in Game Mode (XDG autostart only runs in
//! Plasma), so the unit runs `flatpak run …` itself. Talks to the user systemd over
//! D-Bus, which works from inside the sandbox.
//!
//! Flatpak permissions: `--filesystem=xdg-config/systemd/user:create`,
//! `--talk-name=org.freedesktop.systemd1`.

use std::path::PathBuf;

use anyhow::Context;
use zbus::Connection;

const UNIT: &str = "framemate-agent.service";

pub async fn install() -> anyhow::Result<()> {
    let exec = match std::env::var("FLATPAK_ID") {
        // `flatpak run` moves the app into its own scope outside this unit's cgroup, so
        // stopping the unit would only kill the launcher; --die-with-parent ties them together.
        Ok(app_id) => {
            let installation = std::fs::read_to_string("/.flatpak-info")
                .ok()
                .and_then(|info| installation_flag(&info));
            if installation.is_none() {
                eprintln!("warning: couldn't tell whether the agent is a --user or --system install");
            }
            let flag = installation.map(|f| format!("{f} ")).unwrap_or_default();
            format!("/usr/bin/flatpak run {flag}--die-with-parent --command=framemate-agent {app_id}")
        }
        Err(_) => std::env::current_exe()?.display().to_string(),
    };
    let unit = format!(
        "# Installed by `framemate-agent install-service`; remove with `uninstall-service`.\n\
         [Unit]\n\
         Description=FrameMate agent (Steam Frame companion)\n\
         \n\
         [Service]\n\
         ExecStart={exec}\n\
         Restart=on-failure\n\
         RestartSec=5\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n"
    );
    let path = unit_path()?;
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, unit).with_context(|| format!("writing {}", path.display()))?;

    // Create the token now, so the starting service and a following `token` call can't race.
    crate::config::load_or_create_token()?;

    let systemd = Systemd::connect().await?;
    systemd.call("Reload", &()).await?;
    systemd.call("EnableUnitFiles", &(&[UNIT][..], false, true)).await?;
    systemd.call("RestartUnit", &(UNIT, "replace")).await?;
    println!("Installed and started {UNIT} ({}).", path.display());
    // `-u {UNIT}` shows nothing: `flatpak run` moves the app into its own scope.
    println!("Logs: journalctl --user -f _COMM=framemate-agent");
    println!();
    crate::check::run().await
}

pub async fn uninstall() -> anyhow::Result<()> {
    let systemd = Systemd::connect().await?;
    // Ignore errors: the unit may not be loaded or enabled.
    let _ = systemd.call("StopUnit", &(UNIT, "replace")).await;
    let _ = systemd.call("DisableUnitFiles", &(&[UNIT][..], false)).await;
    let path = unit_path()?;
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e).with_context(|| format!("removing {}", path.display())),
    }
    systemd.call("Reload", &()).await?;
    println!("Removed {UNIT}.");
    Ok(())
}

/// `--user` or `--system`, from `app-path` in the sandbox's `/.flatpak-info`. Plain `flatpak run`
/// fails when no system installation exists (fresh Frames, Flatpak 1.15.8).
fn installation_flag(flatpak_info: &str) -> Option<&'static str> {
    let path = flatpak_info.lines().find_map(|l| l.trim().strip_prefix("app-path="))?;
    if path.starts_with("/var/lib/flatpak/") {
        Some("--system")
    } else if path.contains("/.local/share/flatpak/") {
        Some("--user")
    } else {
        None // a custom installation (installations.d); plain `flatpak run` finds it
    }
}

/// `~/.config/systemd/user/…` on the host. Deliberately not `$XDG_CONFIG_HOME`, which a
/// Flatpak remaps into its own data directory.
fn unit_path() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME not set")?;
    Ok(PathBuf::from(home).join(".config/systemd/user").join(UNIT))
}

struct Systemd(Connection);

impl Systemd {
    async fn connect() -> anyhow::Result<Self> {
        Ok(Self(Connection::session().await.context("connecting to the session bus")?))
    }

    async fn call<B>(&self, method: &str, body: &B) -> anyhow::Result<()>
    where
        B: serde::Serialize + zbus::zvariant::DynamicType,
    {
        self.0
            .call_method(
                Some("org.freedesktop.systemd1"),
                "/org/freedesktop/systemd1",
                Some("org.freedesktop.systemd1.Manager"),
                method,
                body,
            )
            .await
            .with_context(|| format!("systemd {method}"))?;
        Ok(())
    }
}
