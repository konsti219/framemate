//! Tells clients the headset is going to sleep before it does. A suspend stops everything including
//! Wi-Fi, so without this clients only see the agent vanish. Sleeps started by Steam (power button, idle
//! timer) are marked earlier through the shim's `suspend` topic (see cdp.rs), while Wi-Fi is still up.
//! For anything else this holds a logind "delay" inhibitor, and on `PrepareForSleep(true)` marks the
//! state asleep, gives the push a moment to go out, then releases it. NetworkManager drops Wi-Fi on the
//! same signal, so that push may not make it.
//! Flatpak: --system-talk-name=org.freedesktop.login1

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use serde::Serialize;
use zbus::message::Type;
use zbus::zvariant::OwnedFd;
use zbus::{Connection, MatchRule, MessageStream};

use crate::hub::{Hub, now_ms};

const LOGIND: &str = "org.freedesktop.login1";
const MANAGER: &str = "org.freedesktop.login1.Manager";
/// Covers the push throttle in server.rs plus sending the update.
const PUSH_GRACE: Duration = Duration::from_millis(600);
const RETRY: Duration = Duration::from_secs(30);

#[derive(Debug, Default, Clone, Serialize)]
pub struct SleepState {
    /// True between the sleep signal and the resume. Clients mostly see this in the last update
    /// before the headset goes quiet.
    pub asleep: bool,
    pub asleep_since_ms: Option<u64>,
    pub resumed_at_ms: Option<u64>,
}

pub async fn run(hub: Arc<Hub>) {
    loop {
        if let Err(e) = session(&hub).await {
            tracing::warn!("logind: {e:#}");
        }
        tokio::time::sleep(RETRY).await;
    }
}

async fn inhibit(conn: &Connection) -> zbus::Result<OwnedFd> {
    let reply = conn
        .call_method(
            Some(LOGIND),
            "/org/freedesktop/login1",
            Some(MANAGER),
            "Inhibit",
            &("sleep", "framemate", "Telling clients the headset goes to sleep", "delay"),
        )
        .await?;
    reply.body().deserialize::<OwnedFd>()
}

async fn session(hub: &Hub) -> anyhow::Result<()> {
    let conn = Connection::system().await?;
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(LOGIND)?
        .interface(MANAGER)?
        .member("PrepareForSleep")?
        .build();
    let mut signals = MessageStream::for_match_rule(rule, &conn, Some(8)).await?;
    let mut lock = Some(inhibit(&conn).await?);
    tracing::info!("logind: holding a sleep delay lock");

    while let Some(msg) = signals.next().await {
        let starting: bool = msg?.body().deserialize()?;
        if starting {
            tracing::info!("going to sleep");
            hub.update(|s| {
                if !s.sleep.asleep {
                    s.sleep.asleep = true;
                    s.sleep.asleep_since_ms = Some(now_ms());
                }
            });
            tokio::time::sleep(PUSH_GRACE).await;
            lock = None;
        } else {
            tracing::info!("resumed");
            hub.update(|s| {
                s.sleep.asleep = false;
                s.sleep.resumed_at_ms = Some(now_ms());
            });
            lock = Some(inhibit(&conn).await?);
        }
    }
    drop(lock);
    anyhow::bail!("signal stream ended")
}
