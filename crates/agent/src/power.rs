//! Battery and charger data from the kernel's power_supply class (works without Steam).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;

use crate::hub::Hub;

const POLL_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct PowerState {
    pub battery: Option<Battery>,
    /// True if any non-battery supply (charger, USB-C) reports ONLINE=1.
    pub external_power: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Battery {
    pub name: String,
    pub status: Option<String>,
    pub health: Option<String>,
    pub capacity_percent: Option<u8>,
    pub voltage_v: Option<f64>,
    /// Negative while discharging.
    pub current_a: Option<f64>,
    pub power_w: Option<f64>,
    pub temp_c: Option<f64>,
    pub cycle_count: Option<u32>,
    pub charge_now_mah: Option<u32>,
    pub charge_full_mah: Option<u32>,
    pub charge_full_design_mah: Option<u32>,
    pub time_to_empty_s: Option<u64>,
    pub time_to_full_s: Option<u64>,
}

pub async fn run(hub: Arc<Hub>, dir: PathBuf) {
    let mut last = None;
    loop {
        let state = read(&dir);
        if last.as_ref() != Some(&state) {
            let snapshot = state.clone();
            hub.update(|s| s.power = Some(snapshot));
            last = Some(state);
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

fn read(dir: &Path) -> PowerState {
    let mut state = PowerState::default();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return state;
    };
    for entry in entries.flatten() {
        let Ok(uevent) = std::fs::read_to_string(entry.path().join("uevent")) else {
            continue;
        };
        let props: HashMap<&str, &str> = uevent
            .lines()
            .filter_map(|l| l.strip_prefix("POWER_SUPPLY_")?.split_once('='))
            .collect();
        if props.get("TYPE") == Some(&"Battery") {
            if state.battery.is_none() {
                state.battery = Some(battery(&entry.file_name().to_string_lossy(), &props));
            }
        } else if let Some(online) = props.get("ONLINE") {
            let online = *online == "1";
            state.external_power = Some(state.external_power.unwrap_or(false) || online);
        }
    }
    state
}

fn battery(name: &str, p: &HashMap<&str, &str>) -> Battery {
    let int = |k: &str| p.get(k).and_then(|v| v.parse::<i64>().ok());
    // power_supply units: µV, µA, µAh, tenths of °C, seconds.
    let micro = |k: &str| int(k).map(|v| v as f64 / 1e6);
    let mah = |k: &str| int(k).map(|v| (v / 1000) as u32);
    let status = p.get("STATUS").map(|s| s.to_string());
    let voltage_v = micro("VOLTAGE_NOW");
    let current_a = micro("CURRENT_NOW");
    let charging = status.as_deref() == Some("Charging");
    let discharging = status.as_deref() == Some("Discharging");
    Battery {
        name: name.to_owned(),
        health: p.get("HEALTH").map(|s| s.to_string()),
        capacity_percent: int("CAPACITY").map(|v| v.clamp(0, 100) as u8),
        voltage_v,
        current_a,
        power_w: voltage_v.zip(current_a).map(|(v, a)| v * a),
        temp_c: int("TEMP").map(|v| v as f64 / 10.0),
        cycle_count: int("CYCLE_COUNT").map(|v| v as u32),
        charge_now_mah: mah("CHARGE_NOW"),
        charge_full_mah: mah("CHARGE_FULL"),
        charge_full_design_mah: mah("CHARGE_FULL_DESIGN"),
        // The fuel gauge reports both estimates at all times; only one is meaningful.
        time_to_empty_s: int("TIME_TO_EMPTY_AVG").filter(|_| discharging).map(|v| v as u64),
        time_to_full_s: int("TIME_TO_FULL_AVG").filter(|_| charging).map(|v| v as u64),
        status,
    }
}
