use std::{net::IpAddr, time::Duration};

use axum::http::HeaderMap;
use serde_json::json;

use crate::{db, state::AppState};

const TIMEOUT_SECS: u64 = 10;
const MAX_FIELD_CHARS: usize = 100;

// The panel is normally behind a reverse proxy, so the socket peer is the proxy.
// Trust the conventional client-IP headers; an operator exposing the panel
// directly must not rely on them for security decisions.
pub(crate) fn real_ip(headers: &HeaderMap, peer: IpAddr) -> IpAddr {
    let header_ip = |name: &str| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse().ok())
    };
    header_ip("x-real-ip")
        .or_else(|| header_ip("cf-connecting-ip"))
        .or_else(|| {
            headers
                .get("x-forwarded-for")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.split(',').next())
                .and_then(|value| value.trim().parse().ok())
        })
        .unwrap_or(peer)
}

fn clean_field(value: &str) -> String {
    let cleaned: String = value
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .take(MAX_FIELD_CHARS)
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        "(空)".into()
    } else {
        cleaned.to_string()
    }
}

pub(crate) fn spawn_failed_login(
    state: AppState,
    ip: IpAddr,
    username: &str,
    source: &'static str,
) {
    let username = clean_field(username);
    tokio::spawn(async move {
        if let Err(err) = send_failed_login(&state, ip, &username, source).await {
            tracing::warn!(error = %err, "login failure webhook failed");
        }
    });
}

async fn send_failed_login(
    state: &AppState,
    ip: IpAddr,
    username: &str,
    source: &'static str,
) -> anyhow::Result<()> {
    let Some(url) = webhook_url(state).await else {
        return Ok(());
    };
    let content = format!(
        "【Lumen 面板登录失败】\n账户：{username}\n真实 IP：{ip}\n时间：{}\n来源：{source}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S %:z"),
    );
    let payload = json!({
        "msgtype": "text",
        "text": { "content": content },
    });
    let body = serde_json::to_string(&payload)?;
    let success = tokio::task::spawn_blocking(move || -> Result<bool, ureq::Error> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(TIMEOUT_SECS)))
            .build()
            .into();
        let response = agent
            .post(&url)
            .header("Content-Type", "application/json")
            .send(body)?;
        Ok(response.status().is_success())
    })
    .await
    .map_err(|err| anyhow::anyhow!("webhook task failed: {err}"))??;
    if !success {
        tracing::warn!("login failure webhook returned non-2xx");
    }
    Ok(())
}

async fn webhook_url(state: &AppState) -> Option<String> {
    let values = db::settings::get_all(&state.db).await.ok()?;
    let stored: serde_json::Value = serde_json::from_str(values.get("x3ui_settings")?).ok()?;
    stored
        .get("loginNotifyWebhook")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(*name, value.parse().unwrap());
        }
        map
    }

    #[test]
    fn real_ip_prefers_proxy_headers_in_order() {
        let peer: IpAddr = "127.0.0.1".parse().unwrap();
        assert_eq!(
            real_ip(
                &headers(&[
                    ("x-real-ip", "203.0.113.9"),
                    ("cf-connecting-ip", "198.51.100.4"),
                    ("x-forwarded-for", "192.0.2.7, 10.0.0.1"),
                ]),
                peer
            ),
            "203.0.113.9".parse::<IpAddr>().unwrap()
        );
        assert_eq!(
            real_ip(
                &headers(&[("x-forwarded-for", "192.0.2.7, 10.0.0.1")]),
                peer
            ),
            "192.0.2.7".parse::<IpAddr>().unwrap()
        );
        assert_eq!(real_ip(&headers(&[]), peer), peer);
    }

    #[test]
    fn clean_field_strips_control_characters_and_limits_length() {
        assert_eq!(clean_field("  admin\nsecond line  "), "admin second line");
        assert_eq!(clean_field(""), "(空)");
        assert_eq!(clean_field(&"a".repeat(200)).chars().count(), 100);
    }
}
