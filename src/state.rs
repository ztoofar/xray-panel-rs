use std::{
    collections::{HashMap, VecDeque},
    net::IpAddr,
    sync::{Arc, RwLock},
};

use serde::Serialize;
use tokio::sync::{broadcast, Mutex, Notify};

use crate::{
    config::Config,
    db::Db,
    outbound_check::OutboundCheckRun,
    panel_log::PanelLogBuffer,
    xray::{LogBuffer, OnlineUser, StatsClient, XrayHandle},
};

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub cfg: Arc<Config>,
    pub xray: XrayHandle,
    pub stats: StatsClient,
    pub xray_logs: LogBuffer,
    pub panel_logs: PanelLogBuffer,
    // Serialized native speed snapshots (`{type,payload,time}`). Broadcast only
    // while a panel page holds the WS open; the payload already contains
    // bytes/second, so the browser never divides deltas itself.
    pub ws_events: broadcast::Sender<String>,
    pub online_clients: Arc<Mutex<HashMap<String, OnlineUser>>>,
    // email -> last sign of life (traffic delta or live connection), seconds.
    // The online set is every entry still inside the grace window.
    pub online_last_seen: Arc<Mutex<HashMap<String, i64>>>,
    pub active_inbounds: Arc<Mutex<Vec<String>>>,
    // inbound tag -> last traffic delta timestamp (seconds), same grace window
    pub active_inbound_last_seen: Arc<Mutex<HashMap<String, i64>>>,
    pub login_limiter: Arc<Mutex<HashMap<IpAddr, Vec<i64>>>>,
    pub traffic_history: Arc<Mutex<VecDeque<TrafficPoint>>>,
    // Rates derived from this sample's deltas over the real elapsed interval.
    // Filled only while a speed page is watching; empty otherwise, so an idle
    // panel never pays for rate computation.
    pub traffic_speeds: Arc<Mutex<TrafficSpeeds>>,
    // Wakes the traffic job early when the admin changes the poll interval.
    pub refresh_notify: Arc<Notify>,
    // Latest periodic outbound connectivity check, for the advanced settings page.
    pub outbound_check: Arc<Mutex<Option<OutboundCheckRun>>>,
    // Wakes the outbound checker after its settings are saved.
    pub outbound_check_notify: Arc<Notify>,
    // Normalized panel URL prefix (`/` or `/secret/`). Read per request, so a
    // settings save takes effect without a restart.
    pub base_path: Arc<RwLock<String>>,
    pub started_at: i64,
}

impl AppState {
    pub fn base_path(&self) -> String {
        match self.base_path.read() {
            Ok(path) => path.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    pub fn set_base_path(&self, value: String) {
        match self.base_path.write() {
            Ok(mut path) => *path = value,
            Err(poisoned) => *poisoned.into_inner() = value,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficPoint {
    pub ts: i64,
    pub up: i64,
    pub down: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedSample {
    pub up: i64,
    pub down: i64,
}

#[derive(Debug, Clone, Default)]
pub struct TrafficSpeeds {
    // Inbound tag -> bytes/second.
    pub inbounds: HashMap<String, SpeedSample>,
    // Panel client email -> bytes/second across every attachment.
    pub clients: HashMap<String, SpeedSample>,
}
