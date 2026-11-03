use serde::Serialize;
use serde_json::Value;
use sqlx::types::Json;
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Inbound {
    pub id: i64,
    pub user_id: i64,
    pub up: i64,
    pub down: i64,
    #[serde(skip_serializing)]
    pub last_up: i64,
    #[serde(skip_serializing)]
    pub last_down: i64,
    pub total: i64,
    pub remark: String,
    pub sub_sort_index: i64,
    pub exclude_from_sub: bool,
    pub enable: bool,
    pub expiry_time: i64,
    pub traffic_reset: String,
    pub traffic_reset_day: i64,
    pub last_traffic_reset_time: i64,
    pub listen: String,
    pub port: i64,
    pub protocol: String,
    pub settings: Json<Value>,
    pub stream_settings: Json<Value>,
    pub sniffing: Json<Value>,
    pub tag: String,
    pub share_addr_strategy: String,
    pub share_addr: String,
    pub disable_flow: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Client {
    pub id: i64,
    pub inbound_id: i64,
    pub email: String,
    pub enable: bool,
    pub up: i64,
    pub down: i64,
    #[serde(skip_serializing)]
    pub last_up: i64,
    #[serde(skip_serializing)]
    pub last_down: i64,
    pub total: i64,
    pub expiry_time: i64,
    pub settings: Json<Value>,
    pub comment: String,
    pub group_name: String,
    pub limit_ip: i64,
    pub limit_hwid: i64,
    pub reset_days: i64,
    pub last_reset_time: i64,
    pub last_online: i64,
    #[sqlx(skip)]
    pub inbound_ids: Vec<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct SessionUser {
    pub user_id: i64,
    pub csrf: String,
    pub expires_at: i64,
    pub username: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboundCount {
    pub inbound_id: i64,
    pub clients: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct OutboundTraffic {
    pub tag: String,
    pub up: i64,
    pub down: i64,
    #[serde(skip_serializing)]
    pub last_up: i64,
    #[serde(skip_serializing)]
    pub last_down: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct OutboundSubscription {
    pub id: i64,
    pub remark: String,
    pub url: String,
    pub enabled: bool,
    pub allow_private: bool,
    pub allow_insecure: bool,
    pub user_agent: String,
    pub proxy_url: String,
    pub tag_prefix: String,
    pub update_interval: i64,
    pub priority: i64,
    pub prepend: bool,
    pub last_updated: i64,
    pub last_error: String,
    pub last_fetched_outbounds: String,
    pub link_identities: String,
    pub created_at: i64,
    pub updated_at: i64,
}

// List payload: the stored raw outbound blob stays in SQLite, like 3x-ui,
// while the UI only needs the count.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboundSubscriptionView {
    pub id: i64,
    pub remark: String,
    pub url: String,
    pub enabled: bool,
    pub allow_private: bool,
    pub allow_insecure: bool,
    pub user_agent: String,
    pub proxy_url: String,
    pub tag_prefix: String,
    pub update_interval: i64,
    pub priority: i64,
    pub prepend: bool,
    pub last_updated: i64,
    pub last_error: String,
    pub outbound_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}
