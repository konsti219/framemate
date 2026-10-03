//! Shared agent state. Collectors mutate it through [`Hub::update`]; the server
//! serializes snapshots and gets notified of changes through a watch channel.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;
use tokio::sync::watch;

use crate::power::PowerState;
use crate::stream::StreamStats;

#[derive(Debug, Default, Serialize)]
pub struct State {
    pub agent: AgentInfo,
    pub steam: SteamState,
    pub power: Option<PowerState>,
    pub steamos: SteamOsState,
    /// Present while the headset-view stream runs (i.e. someone is watching).
    pub stream: Option<StreamStats>,
}

#[derive(Debug, Default, Serialize)]
pub struct AgentInfo {
    pub version: &'static str,
    pub hostname: String,
    pub started_at_ms: u64,
    pub updated_at_ms: u64,
}

/// Reported by `shim.js`. Topics stay loose JSON: the Steam APIs behind them change often.
#[derive(Debug, Default, Serialize)]
pub struct SteamState {
    pub connected: bool,
    pub error: Option<String>,
    pub last_event_ms: Option<u64>,
    pub topics: BTreeMap<String, Value>,
    /// Per-topic errors reported by the shim; cleared when the topic reports again.
    pub topic_errors: BTreeMap<String, String>,
}

#[derive(Debug, Default, Serialize)]
pub struct SteamOsState {
    pub available: bool,
    pub error: Option<String>,
    pub properties: BTreeMap<String, Value>,
}

pub struct Hub {
    state: RwLock<State>,
    version: watch::Sender<u64>,
}

impl Hub {
    pub fn new(hostname: String) -> Arc<Self> {
        let now = now_ms();
        let state = State {
            agent: AgentInfo {
                version: env!("CARGO_PKG_VERSION"),
                hostname,
                started_at_ms: now,
                updated_at_ms: now,
            },
            ..Default::default()
        };
        Arc::new(Self {
            state: RwLock::new(state),
            version: watch::Sender::new(0),
        })
    }

    pub fn update(&self, f: impl FnOnce(&mut State)) {
        {
            let mut state = self.state.write().unwrap();
            f(&mut state);
            state.agent.updated_at_ms = now_ms();
        }
        self.version.send_modify(|v| *v += 1);
    }

    pub fn snapshot_json(&self) -> String {
        serde_json::to_string(&*self.state.read().unwrap()).expect("state serializes")
    }

    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.version.subscribe()
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
