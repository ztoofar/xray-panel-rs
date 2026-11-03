use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use once_cell::sync::Lazy;
use serde_json::{json, Value};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::RwLock,
    time::timeout,
};

const CACHE_TTL: Duration = Duration::from_secs(600);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

type IpPair = (String, String);
type CacheEntry = (Instant, IpPair);

static CACHE: Lazy<RwLock<Option<CacheEntry>>> = Lazy::new(|| RwLock::new(None));
static REFRESHING: AtomicBool = AtomicBool::new(false);

// The dashboard polls status every 2s, so the lookup runs in the background and
// a stale answer is served while it refreshes.
pub async fn public_ip_json() -> Value {
    match snapshot() {
        Some((true, value)) => value,
        Some((false, value)) => {
            spawn_refresh();
            value
        }
        None => {
            spawn_refresh();
            json!({ "ipv4": "", "ipv6": "" })
        }
    }
}

fn snapshot() -> Option<(bool, Value)> {
    let guard = CACHE.try_read().ok()?;
    let (at, (ipv4, ipv6)) = guard.as_ref()?;
    Some((
        at.elapsed() < CACHE_TTL,
        json!({ "ipv4": ipv4, "ipv6": ipv6 }),
    ))
}

fn spawn_refresh() {
    if REFRESHING.swap(true, Ordering::SeqCst) {
        return;
    }
    tokio::spawn(async {
        let ipv4 = fetch_plain_ip("ipv4.icanhazip.com")
            .await
            .unwrap_or_default();
        let ipv6 = fetch_plain_ip("ipv6.icanhazip.com")
            .await
            .unwrap_or_default();
        *CACHE.write().await = Some((Instant::now(), (ipv4, ipv6)));
        REFRESHING.store(false, Ordering::SeqCst);
    });
}

async fn fetch_plain_ip(host: &str) -> Option<String> {
    let mut stream = timeout(REQUEST_TIMEOUT, TcpStream::connect((host, 80)))
        .await
        .ok()?
        .ok()?;
    let request = format!(
        "GET / HTTP/1.1\r\nHost: {host}\r\nUser-Agent: lumen-panel\r\nConnection: close\r\n\r\n"
    );
    timeout(REQUEST_TIMEOUT, stream.write_all(request.as_bytes()))
        .await
        .ok()?
        .ok()?;
    let mut buffer = Vec::new();
    timeout(REQUEST_TIMEOUT, stream.read_to_end(&mut buffer))
        .await
        .ok()?
        .ok()?;
    let text = String::from_utf8_lossy(&buffer);
    let body = text.split("\r\n\r\n").nth(1)?.trim();
    let ip: std::net::IpAddr = body.parse().ok()?;
    if ip.is_ipv4() != host.starts_with("ipv4") {
        return None;
    }
    Some(ip.to_string())
}
