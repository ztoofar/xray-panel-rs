use std::{
    sync::{
        atomic::{AtomicU64, AtomicU8, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Context};
use serde::Serialize;
use tokio::sync::Mutex;
use tonic::transport::Channel;

pub mod proto {
    tonic::include_proto!("xray.app.stats.command");
}

use proto::{
    stats_service_client::StatsServiceClient, GetUsersStatsRequest, QueryStatsRequest, Stat,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineIp {
    pub ip: String,
    pub last_seen: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineUser {
    pub email: String,
    pub ips: Vec<OnlineIp>,
    // Inbound tags this client is connected through, filled by the traffic job
    // from the per-attachment stat identity. Empty when the core/api cannot
    // report it; the inbound page then falls back to activeInbounds.
    pub inbounds: Vec<String>,
}

const ONLINE_API_UNKNOWN: u8 = 0;
const ONLINE_API_SUPPORTED: u8 = 1;
const ONLINE_API_UNSUPPORTED: u8 = 2;
// 3x-ui replaces gRPC's 4 MiB receive default with 64 MiB because a stats
// reply of about 45k clients reaches the default on its own, and a reply over
// the cap fails wholesale — every traffic number on the panel disappears at
// once. The peer is the panel's own core on loopback, so the cap is bounded.
const MAX_STATS_REPLY_BYTES: usize = 64 << 20;

// 3x-ui polls the three stat families with one QueryStats each: a reply that
// fails for one family (e.g. an outbound tag the core chokes on) must not take
// every client's counters down with it. The core matches the pattern anywhere
// in the stat name, so replies are filtered back to the family prefix.
pub const FAMILY_USER: &str = "user>>>";
pub const FAMILY_INBOUND: &str = "inbound>>>";
pub const FAMILY_OUTBOUND: &str = "outbound>>>";
pub const STATS_FAMILIES: [&str; 3] = [FAMILY_USER, FAMILY_INBOUND, FAMILY_OUTBOUND];
// Old cores answer Unimplemented on every call; remember that instead of
// probing each tick, but re-check occasionally so an upgraded core is noticed
// even when a restart leaves the gRPC channel alive.
const ONLINE_API_RECHECK_SECS: u64 = 600;
// A hung xray API must not stall the traffic tick (and with it the online view).
const STATS_RPC_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
// The API is local by default and each tick may try twice (online + traffic),
// so connect failures get a tighter bound than the RPC itself.
const STATS_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// One stats family's reply. `error` is set when that family could not be
/// queried; the other families still report and keep their counter baselines.
#[derive(Debug)]
pub struct FamilyStats {
    pub family: &'static str,
    pub stats: Vec<Stat>,
    pub error: Option<String>,
}

impl FamilyStats {
    /// The family prefix without the separator (`"user>>>"` → `"user"`).
    pub fn kind(&self) -> &'static str {
        match self.family {
            FAMILY_USER => "user",
            FAMILY_INBOUND => "inbound",
            _ => "outbound",
        }
    }
}

// One QueryStats RPC. Counters are never reset (reset: false, like 3x-ui);
// the panel derives deltas from stored baselines instead, so a failed or
// racing reset can never make the same bytes count twice.
async fn query_family(
    client: &mut StatsServiceClient<Channel>,
    family: &str,
) -> anyhow::Result<Vec<Stat>> {
    let request = QueryStatsRequest {
        pattern: family.to_string(),
        reset: false,
        regexp: false,
    };
    match tokio::time::timeout(STATS_RPC_TIMEOUT, client.query_stats(request)).await {
        Ok(Ok(response)) => Ok(response
            .into_inner()
            .stat
            .into_iter()
            // The core matches the pattern anywhere in the name; the prefix
            // keeps the reply to one family (3x-ui filters the same way).
            .filter(|stat| stat.name.starts_with(family))
            .collect()),
        Ok(Err(status)) => Err(anyhow!("统计查询失败：{status}")),
        Err(_) => Err(anyhow!("统计查询超时")),
    }
}

#[derive(Clone)]
pub struct StatsClient {
    endpoint: String,
    client: Arc<Mutex<Option<StatsServiceClient<Channel>>>>,
    // 3x-ui probes GetUsersStats once per core: an old binary answers
    // Unimplemented for every call, so remember that until xray restarts.
    online_api: Arc<AtomicU8>,
    online_api_checked_at: Arc<AtomicU64>,
}

impl StatsClient {
    pub fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            client: Arc::new(Mutex::new(None)),
            online_api: Arc::new(AtomicU8::new(ONLINE_API_UNKNOWN)),
            online_api_checked_at: Arc::new(AtomicU64::new(0)),
        }
    }

    // Bound the connect as well: hyper's connector can stall for the OS TCP
    // timeout when the API address is unreachable, and the RPC timeout only
    // starts once a channel exists. A stalled connect delays every tick (and
    // with it the online view), which the per-call timeout alone does not fix.
    async fn connect(&self) -> anyhow::Result<StatsServiceClient<Channel>> {
        let client = tokio::time::timeout(
            STATS_CONNECT_TIMEOUT,
            StatsServiceClient::connect(self.endpoint.clone()),
        )
        .await
        .map_err(|_| anyhow!("连接 Xray 统计接口 {} 超时", self.endpoint))?
        .with_context(|| format!("连接 Xray 统计接口 {} 失败", self.endpoint))?;
        Ok(client.max_decoding_message_size(MAX_STATS_REPLY_BYTES))
    }

    /// Forget what we know about the online-stats RPC. Called when Xray
    /// restarts: an upgraded core may support `GetUsersStats` even though the
    /// previous one answered Unimplemented.
    pub fn invalidate_online_api_probe(&self) {
        self.online_api.store(ONLINE_API_UNKNOWN, Ordering::Relaxed);
        self.online_api_checked_at.store(0, Ordering::Relaxed);
    }

    /// Query each stats family separately (3x-ui GetTraffic): one family's
    /// failure leaves the other families live, and a failed family keeps its
    /// counter baselines intact for the next round. Returns `Err` only when
    /// *every* family failed, which is the tick failure path; partial failures
    /// come back per family in `FamilyStats`.
    pub async fn query_families(&self) -> anyhow::Result<Vec<FamilyStats>> {
        let mut guard = self.client.lock().await;
        self.ensure_connected(&mut guard).await?;
        let client = guard.as_mut().expect("client connected");
        let mut out = Vec::with_capacity(STATS_FAMILIES.len());
        let mut failures = 0;
        for family in STATS_FAMILIES {
            match query_family(client, family).await {
                Ok(stats) => out.push(FamilyStats {
                    family,
                    stats,
                    error: None,
                }),
                Err(err) => {
                    failures += 1;
                    out.push(FamilyStats {
                        family,
                        stats: Vec::new(),
                        error: Some(format!("{err:#}")),
                    });
                }
            }
        }
        if failures == STATS_FAMILIES.len() {
            // Nothing answered: the channel is dead (Xray restarted or gone).
            // Reconnect on the next tick instead of retrying a stale one.
            *guard = None;
            return Err(anyhow!("Xray 统计查询全部失败"));
        }
        Ok(out)
    }

    async fn ensure_connected(
        &self,
        guard: &mut Option<StatsServiceClient<Channel>>,
    ) -> anyhow::Result<()> {
        if guard.is_none() {
            let client = self.connect().await?;
            *guard = Some(client);
            // A fresh channel can mean a new xray process (and maybe a new
            // core), so re-probe the online-stats RPC.
            self.online_api.store(ONLINE_API_UNKNOWN, Ordering::Relaxed);
            self.online_api_checked_at.store(0, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Connection-level online users (email + source IPs), like 3x-ui's
    /// GetOnlineUsers. `Ok(None)` means the running core predates the
    /// online-stats API; callers must fall back to traffic/access-log signals.
    /// Requires `statsUserOnline` on every policy level.
    pub async fn online_users(&self) -> anyhow::Result<Option<Vec<OnlineUser>>> {
        if self.online_api.load(Ordering::Relaxed) == ONLINE_API_UNSUPPORTED
            && now_secs().saturating_sub(self.online_api_checked_at.load(Ordering::Relaxed))
                < ONLINE_API_RECHECK_SECS
        {
            return Ok(None);
        }
        let mut guard = self.client.lock().await;
        self.ensure_connected(&mut guard).await?;
        let client = guard.as_mut().expect("client connected");
        let request = GetUsersStatsRequest {
            include_traffic: false,
            reset: false,
        };
        match tokio::time::timeout(STATS_RPC_TIMEOUT, client.get_users_stats(request)).await {
            Ok(Ok(response)) => {
                self.online_api
                    .store(ONLINE_API_SUPPORTED, Ordering::Relaxed);
                self.online_api_checked_at
                    .store(now_secs(), Ordering::Relaxed);
                Ok(Some(
                    response
                        .into_inner()
                        .users
                        .into_iter()
                        .map(|user| OnlineUser {
                            email: user.email,
                            ips: user
                                .ips
                                .into_iter()
                                .map(|entry| OnlineIp {
                                    ip: entry.ip,
                                    last_seen: entry.last_seen,
                                })
                                .collect(),
                            inbounds: Vec::new(),
                        })
                        .collect(),
                ))
            }
            Ok(Err(status)) if status.code() == tonic::Code::Unimplemented => {
                // Keep the channel: dropping it would reconnect (and re-probe)
                // on every tick for the rest of this core's lifetime.
                self.online_api
                    .store(ONLINE_API_UNSUPPORTED, Ordering::Relaxed);
                self.online_api_checked_at
                    .store(now_secs(), Ordering::Relaxed);
                Ok(None)
            }
            Ok(Err(status)) => {
                *guard = None;
                Err(anyhow!("在线用户查询失败：{status}"))
            }
            Err(_) => {
                *guard = None;
                Err(anyhow!("在线用户查询超时"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn invalidating_the_probe_forces_a_fresh_connection_attempt() {
        let client = StatsClient::new("http://127.0.0.1:1".into());
        // Pretend an old core just answered Unimplemented: the cache skips it.
        client
            .online_api
            .store(ONLINE_API_UNSUPPORTED, Ordering::Relaxed);
        client
            .online_api_checked_at
            .store(now_secs(), Ordering::Relaxed);
        assert!(matches!(client.online_users().await, Ok(None)));

        // A new process invalidates the probe; the next call must try the
        // (unreachable) API instead of returning the cached None.
        client.invalidate_online_api_probe();
        assert_eq!(
            client.online_api.load(Ordering::Relaxed),
            ONLINE_API_UNKNOWN
        );
        assert_eq!(client.online_api_checked_at.load(Ordering::Relaxed), 0);
        assert!(client.online_users().await.is_err());
    }
}
