use std::{
    collections::{HashMap, VecDeque},
    net::IpAddr,
    sync::{Arc, RwLock},
};

use serde::Serialize;
use tokio::sync::{Mutex, Notify};

use crate::{
    config::Config,
    db::Db,
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
    pub online_clients: Arc<Mutex<HashMap<String, OnlineUser>>>,
    // email -> last sign of life (traffic delta or live connection), seconds.
    // The online set is every entry still inside the grace window.
    pub online_last_seen: Arc<Mutex<HashMap<String, i64>>>,
    pub active_inbounds: Arc<Mutex<Vec<String>>>,
    // inbound tag -> last traffic delta timestamp (seconds), same grace window
    pub active_inbound_last_seen: Arc<Mutex<HashMap<String, i64>>>,
    pub login_limiter: Arc<Mutex<HashMap<IpAddr, Vec<i64>>>>,
    pub traffic_history: Arc<Mutex<VecDeque<TrafficPoint>>>,
    // Wakes the traffic job early when the admin changes the poll interval.
    pub refresh_notify: Arc<Notify>,
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
