//! Read-only view of steamos-manager's session-bus properties
//! (performance profile, CPU/GPU tuning). Flatpak: --talk-name=com.steampowered.SteamOSManager1

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value as Json, json};
use zbus::fdo::PropertiesProxy;
use zbus::names::InterfaceName;
use zbus::zvariant::Value;

use crate::hub::Hub;

const POLL_INTERVAL: Duration = Duration::from_secs(30);
const DEST: &str = "com.steampowered.SteamOSManager1";
const PATH: &str = "/com/steampowered/SteamOSManager1";

/// (interface suffix, property)
const PROPERTIES: &[(&str, &str)] = &[
    ("Manager2", "DeviceModel"),
    ("PerformanceProfile1", "PerformanceProfile"),
    ("CpuScaling1", "CpuScalingGovernor"),
    ("CpuScheduler1", "CpuScheduler"),
    ("CpuBoost1", "CpuBoostState"),
    ("GpuPerformanceLevel1", "GpuPerformanceLevel"),
    ("GpuPerformanceLevel1", "ManualGpuClock"),
    ("WifiPowerManagement1", "WifiPowerManagementState"),
    ("SessionManagement1", "DefaultLoginMode"),
];

pub async fn run(hub: Arc<Hub>) {
    loop {
        if let Err(e) = session(&hub).await {
            tracing::warn!("steamos-manager: {e:#}");
            hub.update(|s| {
                s.steamos.available = false;
                s.steamos.error = Some(format!("{e:#}"));
            });
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

async fn session(hub: &Hub) -> anyhow::Result<()> {
    let conn = zbus::Connection::session().await?;
    let proxy = PropertiesProxy::builder(&conn)
        .destination(DEST)?
        .path(PATH)?
        .build()
        .await?;
    let mut last = None;
    loop {
        let mut props = BTreeMap::new();
        for (iface, prop) in PROPERTIES {
            let iface = InterfaceName::try_from(format!("{DEST}.{iface}"))?;
            // Individual properties may be missing on other SteamOS versions.
            if let Ok(value) = proxy.get(iface, prop).await {
                props.insert(prop.to_string(), to_json(&value));
            }
        }
        if props.is_empty() {
            anyhow::bail!("no properties readable from {DEST}");
        }
        if last.as_ref() != Some(&props) {
            let snapshot = props.clone();
            hub.update(|s| {
                s.steamos.available = true;
                s.steamos.error = None;
                s.steamos.properties = snapshot;
            });
            last = Some(props);
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

fn to_json(value: &Value) -> Json {
    match value {
        Value::Bool(v) => json!(v),
        Value::U8(v) => json!(v),
        Value::I16(v) => json!(v),
        Value::U16(v) => json!(v),
        Value::I32(v) => json!(v),
        Value::U32(v) => json!(v),
        Value::I64(v) => json!(v),
        Value::U64(v) => json!(v),
        Value::F64(v) => json!(v),
        Value::Str(v) => json!(v.as_str()),
        Value::Value(v) => to_json(v),
        Value::Array(a) => Json::Array(a.iter().map(to_json).collect()),
        Value::Structure(s) => Json::Array(s.fields().iter().map(to_json).collect()),
        other => json!(format!("{other:?}")),
    }
}
