//! Remembers VR devices across restarts: SteamVR only lists a controller once it has connected
//! since SteamVR started, so sleeping controllers would vanish after a reboot. Missing ones are
//! merged into `vr_devices` as `connected: false, remembered: true`. Never forgotten; delete
//! `$XDG_STATE_HOME/framemate/devices.json` to reset.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::hub::now_ms;

/// `last_seen_ms` alone only forces a write this often (it changes on every poll).
const LAST_SEEN_SAVE_INTERVAL_MS: u64 = 5 * 60 * 1000;

pub struct DeviceMemory {
    file: Option<PathBuf>,
    /// Keyed by serial; devices without one aren't remembered (no phantom duplicates).
    known: BTreeMap<String, Value>,
    last_save_ms: u64,
}

impl DeviceMemory {
    pub fn load() -> Self {
        let file = crate::config::state_dir().ok().map(|dir| dir.join("devices.json"));
        let known = file
            .as_ref()
            .and_then(|f| std::fs::read(f).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self { file, known, last_save_ms: now_ms() }
    }

    pub fn merge(&mut self, live: Value) -> Value {
        let Value::Array(live) = live else { return live };
        let now = now_ms();
        let before = self.known.clone();

        let mut out = Vec::with_capacity(live.len());
        let mut present = BTreeSet::new();
        for mut device in live {
            let Some(key) = serial(&device) else {
                out.push(device);
                continue;
            };
            // Older versions keyed serial-less devices by OpenVR path; drop such leftovers.
            if let Some(path) = device["path"].as_str().filter(|p| *p != key) {
                self.known.remove(path);
            }
            device["last_seen_ms"] = if device["connected"] == true {
                json!(now)
            } else {
                self.known.get(&key).map_or(Value::Null, |d| d["last_seen_ms"].clone())
            };
            self.known.insert(key.clone(), device.clone());
            present.insert(key);
            out.push(device);
        }
        for (key, device) in &self.known {
            if !present.contains(key) {
                let mut device = device.clone();
                device["connected"] = json!(false);
                device["charging"] = json!(false);
                device["remembered"] = json!(true);
                out.push(device);
            }
        }

        if self.known != before
            && (without_last_seen(&self.known) != without_last_seen(&before)
                || now - self.last_save_ms > LAST_SEEN_SAVE_INTERVAL_MS)
        {
            self.save(now);
        }
        Value::Array(out)
    }

    fn save(&mut self, now: u64) {
        let Some(file) = &self.file else { return };
        // Write + rename: a truncated file would load as empty.
        let tmp = file.with_extension("json.tmp");
        let result = std::fs::create_dir_all(file.parent().unwrap())
            .and_then(|()| std::fs::write(&tmp, serde_json::to_vec_pretty(&self.known).unwrap()))
            .and_then(|()| std::fs::rename(&tmp, file));
        match result {
            Ok(()) => self.last_save_ms = now,
            Err(e) => tracing::warn!("saving {}: {e}", file.display()),
        }
    }
}

fn serial(device: &Value) -> Option<String> {
    device["serial"].as_str().filter(|s| !s.is_empty()).map(str::to_owned)
}

fn without_last_seen(known: &BTreeMap<String, Value>) -> BTreeMap<&String, Value> {
    known
        .iter()
        .map(|(k, v)| {
            let mut v = v.clone();
            if let Some(obj) = v.as_object_mut() {
                obj.remove("last_seen_ms");
            }
            (k, v)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(serial: &str, connected: bool, battery: f64) -> Value {
        json!({ "path": format!("/devices/{serial}"), "serial": serial, "connected": connected, "battery": battery })
    }

    #[test]
    fn remembers_devices_missing_after_restart() {
        let mut memory = DeviceMemory { file: None, known: BTreeMap::new(), last_save_ms: 0 };
        memory.merge(json!([device("hmd", true, 1.0), device("left", true, 0.5)]));

        // After a restart SteamVR only lists the headset.
        let merged = memory.merge(json!([device("hmd", true, 0.9)]));
        let merged = merged.as_array().unwrap();
        assert_eq!(merged.len(), 2);
        let left = merged.iter().find(|d| d["serial"] == "left").unwrap();
        assert_eq!(left["remembered"], true);
        assert_eq!(left["connected"], false);
        assert_eq!(left["battery"], 0.5);
        assert!(left["last_seen_ms"].is_u64());
        assert!(merged.iter().find(|d| d["serial"] == "hmd").unwrap().get("remembered").is_none());
    }

    #[test]
    fn serial_less_devices_are_not_remembered() {
        let mut memory = DeviceMemory { file: None, known: BTreeMap::new(), last_save_ms: 0 };
        // Serial read failed while the controller was connecting.
        let mut connecting = device("right", true, 0.7);
        connecting["serial"] = Value::Null;
        memory.merge(json!([connecting]));
        // Next poll has the serial: exactly one device, nothing remembered by path.
        let merged = memory.merge(json!([device("right", true, 0.7)]));
        assert_eq!(merged.as_array().unwrap().len(), 1);
        assert_eq!(memory.known.len(), 1);
    }

    #[test]
    fn drops_path_keyed_leftovers() {
        let mut memory = DeviceMemory { file: None, known: BTreeMap::new(), last_save_ms: 0 };
        memory.known.insert("/devices/right".into(), device("right", false, 0.5));
        let merged = memory.merge(json!([device("right", true, 0.7)]));
        assert_eq!(merged.as_array().unwrap().len(), 1);
        assert!(!memory.known.contains_key("/devices/right"));
    }
}
