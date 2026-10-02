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
        Ok(app_id) => format!("/usr/bin/flatpak run --command=framemate-agent {app_id}"),
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
    // `flatpak run` moves the app into its own app-flatpak-*.scope, so `-u {UNIT}` shows
    // nothing; match the process name instead.
    println!("Logs: journalctl --user -f _COMM=framemate-agent");
    Ok(())
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
