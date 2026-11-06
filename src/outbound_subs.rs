use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
    time::Duration,
};

use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};
use url::Url;

use crate::{
    db::{
        self,
        models::{now, OutboundSubscription, OutboundSubscriptionView},
        outbound_subs::NewSubscription,
    },
    service,
    state::AppState,
    xray::subscription::{self, ParsedOutbound},
};

pub const DEFAULT_USER_AGENT: &str = "3x-ui-outbound-sub/1.0";
const MAX_BODY_BYTES: u64 = 8 << 20;
const FETCH_TIMEOUT: Duration = Duration::from_secs(30);
const JOB_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Default)]
pub struct SubscriptionInput {
    pub remark: String,
    pub url: String,
    pub tag_prefix: String,
    pub user_agent: String,
    pub proxy_url: String,
    pub update_interval: i64,
    pub enabled: bool,
    pub allow_private: bool,
    pub allow_insecure: bool,
    pub prepend: bool,
}

pub async fn list_views(state: &AppState) -> Result<Vec<OutboundSubscriptionView>> {
    let subs = db::outbound_subs::list(&state.db).await?;
    Ok(subs.iter().map(view).collect())
}

pub fn view(sub: &OutboundSubscription) -> OutboundSubscriptionView {
    OutboundSubscriptionView {
        id: sub.id,
        remark: sub.remark.clone(),
        url: sub.url.clone(),
        enabled: sub.enabled,
        allow_private: sub.allow_private,
        allow_insecure: sub.allow_insecure,
        user_agent: sub.user_agent.clone(),
        proxy_url: sub.proxy_url.clone(),
        tag_prefix: sub.tag_prefix.clone(),
        update_interval: sub.update_interval,
        priority: sub.priority,
        prepend: sub.prepend,
        last_updated: sub.last_updated,
        last_error: sub.last_error.clone(),
        outbound_count: count_outbounds(&sub.last_fetched_outbounds),
        created_at: sub.created_at,
        updated_at: sub.updated_at,
    }
}

fn count_outbounds(raw: &str) -> i64 {
    serde_json::from_str::<Vec<Value>>(raw)
        .map(|items| items.len() as i64)
        .unwrap_or(0)
}

pub async fn create(state: &AppState, input: SubscriptionInput) -> Result<OutboundSubscription> {
    let proxy_url = validate_proxy(&input.proxy_url)?;
    let url = sanitize_public_url(&input.url, input.allow_private, !proxy_url.is_empty()).await?;
    let prefix = if input.tag_prefix.trim().is_empty() {
        next_default_prefix(state, 0).await?
    } else {
        input.tag_prefix.trim().to_string()
    };
    let priority = db::outbound_subs::count(&state.db).await?;
    let id = db::outbound_subs::insert(
        &state.db,
        &NewSubscription {
            remark: input.remark.trim().to_string(),
            url,
            enabled: input.enabled,
            allow_private: input.allow_private,
            allow_insecure: input.allow_insecure,
            user_agent: input.user_agent.trim().to_string(),
            proxy_url,
            tag_prefix: prefix,
            update_interval: normalize_interval(input.update_interval),
            priority,
            prepend: input.prepend,
        },
    )
    .await?;
    db::outbound_subs::get(&state.db, id)
        .await?
        .context("创建后的订阅记录丢失")
}

pub async fn update(state: &AppState, id: i64, input: SubscriptionInput) -> Result<()> {
    let sub = db::outbound_subs::get(&state.db, id)
        .await?
        .context("订阅不存在")?;
    let proxy_url = validate_proxy(&input.proxy_url)?;
    let url = sanitize_public_url(&input.url, input.allow_private, !proxy_url.is_empty()).await?;
    let prefix = if input.tag_prefix.trim().is_empty() {
        next_default_prefix(state, sub.id).await?
    } else {
        input.tag_prefix.trim().to_string()
    };
    db::outbound_subs::update(
        &state.db,
        id,
        input.remark.trim(),
        &url,
        input.enabled,
        input.allow_private,
        input.allow_insecure,
        input.user_agent.trim(),
        &proxy_url,
        &prefix,
        normalize_interval(input.update_interval),
        input.prepend,
    )
    .await?;
    Ok(())
}

pub async fn delete(state: &AppState, id: i64) -> Result<()> {
    db::outbound_subs::get(&state.db, id)
        .await?
        .context("订阅不存在")?;
    db::outbound_subs::delete(&state.db, id).await?;
    if let Err(err) = service::apply(state).await {
        tracing::warn!(id, error = %err, "apply after deleting outbound subscription failed");
    }
    Ok(())
}

pub async fn move_item(state: &AppState, id: i64, up: bool) -> Result<()> {
    let subs = db::outbound_subs::list(&state.db).await?;
    let index = subs
        .iter()
        .position(|sub| sub.id == id)
        .context("订阅不存在")?;
    let target = if up {
        index.checked_sub(1)
    } else {
        (index + 1 < subs.len()).then_some(index + 1)
    };
    if let Some(target) = target {
        let mut ordered: Vec<i64> = subs.iter().map(|sub| sub.id).collect();
        ordered.swap(index, target);
        db::outbound_subs::set_priorities(&state.db, &ordered).await?;
        if let Err(err) = service::apply(state).await {
            tracing::warn!(id, error = %err, "apply after reordering outbound subscriptions failed");
        }
    }
    Ok(())
}

pub async fn apply_after_change(state: &AppState) {
    if let Err(err) = service::apply(state).await {
        tracing::warn!(error = %err, "apply after outbound subscription change failed");
    }
}

fn normalize_interval(interval: i64) -> i64 {
    if interval <= 0 {
        600
    } else {
        interval.clamp(60, 7 * 86_400)
    }
}

fn parse_default_prefix(prefix: &str) -> Option<i64> {
    prefix.strip_prefix("sub")?.strip_suffix('-')?.parse().ok()
}

fn default_prefix_number(subs: &[OutboundSubscription], exclude_id: i64) -> i64 {
    let mut used = HashSet::new();
    for sub in subs {
        if sub.id == exclude_id {
            continue;
        }
        if sub.tag_prefix.trim().is_empty() {
            // A blank prefix falls back to its own id; reserve that number.
            used.insert(sub.id);
        } else if let Some(number) = parse_default_prefix(&sub.tag_prefix) {
            used.insert(number);
        }
    }
    let mut number = 1;
    while used.contains(&number) {
        number += 1;
    }
    number
}

async fn next_default_prefix(state: &AppState, exclude_id: i64) -> Result<String> {
    let subs = db::outbound_subs::list(&state.db).await?;
    Ok(format!("sub{}-", default_prefix_number(&subs, exclude_id)))
}

/// Preview a URL without storing anything. Tags use the prefix the next saved
/// subscription would receive, so the preview matches what a save will show.
pub async fn preview(
    state: &AppState,
    url: &str,
    user_agent: &str,
    proxy_url: &str,
    allow_private: bool,
    allow_insecure: bool,
) -> Result<Vec<Value>> {
    let proxy_url = validate_proxy(proxy_url)?;
    let url = sanitize_public_url(url, allow_private, !proxy_url.is_empty()).await?;
    let body = fetch_body(url, user_agent.to_string(), proxy_url, allow_insecure).await?;
    let parsed = subscription::parse_subscription_body(&body);
    let prefix = next_default_prefix(state, 0).await?;
    let (mut outbounds, identities) = split_parsed(parsed);
    subscription::assign_stable_tags(&mut outbounds, &identities, &HashMap::new(), &[], &prefix);
    Ok(outbounds)
}

async fn fetch_body(
    url: String,
    user_agent: String,
    proxy_url: String,
    allow_insecure: bool,
) -> Result<Vec<u8>> {
    let via_proxy = !proxy_url.trim().is_empty();
    let proxy_hint = proxy_url.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        let user_agent = if user_agent.trim().is_empty() {
            DEFAULT_USER_AGENT.to_string()
        } else {
            user_agent
        };
        let tls = ureq::tls::TlsConfig::builder()
            .disable_verification(allow_insecure)
            .build();
        let mut builder = ureq::Agent::config_builder()
            .timeout_global(Some(FETCH_TIMEOUT))
            .max_redirects(10)
            .user_agent(user_agent)
            .tls_config(tls)
            .http_status_as_error(false);
        if !proxy_url.trim().is_empty() {
            let proxy = ureq::Proxy::new(&proxy_url)
                .with_context(|| format!("代理地址无效：{proxy_url}"))?;
            builder = builder.proxy(Some(proxy));
        }
        let config = builder.build();
        let agent: ureq::Agent = config.into();
        let mut response = agent
            .get(&url)
            .call()
            .with_context(|| format!("请求 {url} 失败"))?;
        let status = response.status().as_u16();
        if status != 200 {
            bail!("订阅地址返回 HTTP {status}");
        }
        let body = response
            .body_mut()
            .with_config()
            .limit(MAX_BODY_BYTES)
            .read_to_vec()
            .context("读取订阅响应失败")?;
        Ok(body)
    })
    .await
    .context("订阅请求任务失败")?;
    result.map_err(|err| subscription_fetch_error(err, &proxy_hint, via_proxy))
}

fn subscription_fetch_error(err: anyhow::Error, proxy: &str, via_proxy: bool) -> anyhow::Error {
    if !crate::updates::is_transport_failure(&err) {
        return err;
    }
    tracing::warn!(error = %format!("{err:#}"), "subscription fetch failed");
    if via_proxy {
        anyhow::anyhow!(crate::updates::proxy_unavailable_message(proxy))
    } else {
        anyhow::anyhow!("无法连接订阅地址：请检查面板服务器的网络和 DNS，或为该订阅配置可用代理。")
    }
}

pub async fn sanitize_public_url(
    raw: &str,
    allow_private: bool,
    via_proxy: bool,
) -> Result<String> {
    let clean = sanitize_http_url(raw)?;
    if allow_private {
        return Ok(clean);
    }
    let url = Url::parse(&clean).context("解析订阅地址失败")?;
    let host = match url.host().context("订阅地址缺少主机名")? {
        url::Host::Domain(domain) => domain.to_string(),
        url::Host::Ipv4(ip) => {
            if blocked_ip(IpAddr::V4(ip)) {
                bail!("已阻止私有或内网地址 {ip}");
            }
            return Ok(clean);
        }
        url::Host::Ipv6(ip) => {
            if blocked_ip(IpAddr::V6(ip)) {
                bail!("已阻止私有或内网地址 {ip}");
            }
            return Ok(clean);
        }
    };
    let port = url.port_or_known_default().unwrap_or(80);
    let addresses = match tokio::net::lookup_host((host.as_str(), port)).await {
        Ok(addresses) => addresses.map(|address| address.ip()).collect::<Vec<_>>(),
        // A proxy (socks5h in particular) may resolve the name on its side;
        // the panel cannot judge a name it cannot resolve itself.
        Err(err) if via_proxy => {
            tracing::warn!(host = %host, error = %err, "cannot resolve subscription host locally; leaving lookup to proxy");
            return Ok(clean);
        }
        Err(err) => return Err(err).with_context(|| format!("cannot resolve host {host}")),
    };
    if addresses.is_empty() {
        bail!("主机 {host} 没有可用的 IP 地址");
    }
    // One public address is enough: a poisoned AAAA record must not veto a
    // healthy hostname, matching 3x-ui's resolver.
    if addresses.iter().all(|ip| blocked_ip(*ip)) {
        bail!(
            "host {host} resolves to a blocked private/internal address {}",
            addresses[0]
        );
    }
    Ok(clean)
}

pub fn validate_proxy(raw: &str) -> Result<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(String::new());
    }
    let url = Url::parse(raw).with_context(|| format!("代理地址无效：{raw}"))?;
    match url.scheme() {
        "http" | "https" | "socks4" | "socks4a" | "socks5" | "socks5h" | "socks" => {}
        other => bail!("不支持的代理协议：{other}（仅支持 http/https/socks4/socks5）"),
    }
    if url.host_str().is_none_or(str::is_empty) {
        bail!("代理地址缺少主机名");
    }
    ureq::Proxy::new(raw).with_context(|| format!("代理地址无效：{raw}"))?;
    Ok(raw.to_string())
}

pub fn sanitize_http_url(raw: &str) -> Result<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        bail!("缺少订阅地址");
    }
    let url = Url::parse(raw).with_context(|| format!("invalid URL: {raw}"))?;
    if url.scheme() != "http" && url.scheme() != "https" {
        bail!("不支持的 URL 协议 \"{}\"", url.scheme());
    }
    if url.host_str().is_none_or(str::is_empty) {
        bail!("订阅地址缺少主机名");
    }
    let mut clean = url;
    let _ = clean.set_username("");
    let _ = clean.set_password(None);
    Ok(clean.to_string())
}

pub(crate) fn blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || octets[0] == 0
                || octets[0] >= 240
                || (octets[0] == 100 && (octets[1] & 0xc0) == 64)
                || (octets[0] == 198 && (octets[1] & 0xfe) == 18)
                || (octets[0] == 192 && octets[1] == 0 && octets[2] == 2)
                || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
                || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113)
        }
        IpAddr::V6(v6) => {
            let segments = v6.segments();
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (segments[0] & 0xfe00) == 0xfc00
                || (segments[0] & 0xffc0) == 0xfe80
                || (segments[0] == 0x2001 && segments[1] == 0x0db8)
                || v6
                    .to_ipv4_mapped()
                    .is_some_and(|v4| blocked_ip(IpAddr::V4(v4)))
        }
    }
}

fn split_parsed(parsed: Vec<ParsedOutbound>) -> (Vec<Value>, Vec<String>) {
    let mut outbounds = Vec::with_capacity(parsed.len());
    let mut identities = Vec::with_capacity(parsed.len());
    for item in parsed {
        outbounds.push(item.outbound);
        identities.push(item.identity);
    }
    (outbounds, identities)
}

struct RefreshResult {
    outbounds: Vec<Value>,
}

async fn refresh_record(
    state: &AppState,
    sub: OutboundSubscription,
    apply: bool,
) -> Result<Vec<Value>> {
    let url = match sanitize_public_url(
        &sub.url,
        sub.allow_private,
        !sub.proxy_url.trim().is_empty(),
    )
    .await
    {
        Ok(url) => url,
        Err(err) => {
            record_error(state, sub.id, &err).await;
            return Err(err);
        }
    };
    let body = match fetch_body(
        url,
        sub.user_agent.clone(),
        sub.proxy_url.clone(),
        sub.allow_insecure,
    )
    .await
    {
        Ok(body) => body,
        Err(err) => {
            record_error(state, sub.id, &err).await;
            return Err(err);
        }
    };
    let parsed = subscription::parse_subscription_body(&body);
    let result = store_refresh(state, &sub, parsed, apply).await;
    if let Err(err) = &result {
        record_error(state, sub.id, err).await;
    }
    result.map(|result| result.outbounds)
}

async fn store_refresh(
    state: &AppState,
    sub: &OutboundSubscription,
    parsed: Vec<ParsedOutbound>,
    apply: bool,
) -> Result<RefreshResult> {
    let previous: HashMap<String, String> =
        serde_json::from_str(&sub.link_identities).unwrap_or_default();
    let previous_by_index: Vec<String> =
        serde_json::from_str::<Vec<Value>>(&sub.last_fetched_outbounds)
            .map(|items| {
                items
                    .iter()
                    .map(|item| {
                        item.get("tag")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string()
                    })
                    .collect()
            })
            .unwrap_or_default();

    let (mut outbounds, identities) = split_parsed(parsed);
    let prefix = if sub.tag_prefix.trim().is_empty() {
        format!("sub{}-", sub.id)
    } else {
        sub.tag_prefix.clone()
    };
    let tags = subscription::assign_stable_tags(
        &mut outbounds,
        &identities,
        &previous,
        &previous_by_index,
        &prefix,
    );
    let identities_map: Map<String, Value> = identities
        .iter()
        .cloned()
        .zip(tags.iter().cloned().map(Value::String))
        .collect();
    let outbounds_json = serde_json::to_string(&outbounds)?;
    let identities_json = serde_json::to_string(&identities_map)?;
    let changed =
        outbounds_json != sub.last_fetched_outbounds || identities_json != sub.link_identities;

    db::outbound_subs::set_result(&state.db, sub.id, &outbounds_json, &identities_json, "").await?;

    if apply && changed {
        if let Err(err) = service::apply(state).await {
            db::outbound_subs::restore_result(
                &state.db,
                sub.id,
                &sub.last_fetched_outbounds,
                &sub.link_identities,
                sub.last_updated,
                &sub.last_error,
            )
            .await?;
            return Err(err.context("新出站被 Xray 拒绝，已保留原有列表"));
        }
    }
    Ok(RefreshResult { outbounds })
}

async fn record_error(state: &AppState, id: i64, err: &anyhow::Error) {
    let message = format!("{err:#}");
    if let Err(db_err) = db::outbound_subs::set_error(&state.db, id, &message).await {
        tracing::warn!(id, error = %db_err, "record outbound subscription error failed");
    }
}

pub async fn refresh_one(state: &AppState, id: i64, apply: bool) -> Result<Vec<Value>> {
    let sub = db::outbound_subs::get(&state.db, id)
        .await?
        .context("订阅不存在")?;
    refresh_record(state, sub, apply).await
}

pub async fn refresh_due(state: &AppState) -> Result<usize> {
    let subs = db::outbound_subs::list_enabled(&state.db).await?;
    let now = now();
    let mut refreshed = 0;
    for sub in subs {
        let due = sub.last_updated == 0
            || sub.last_updated + sub.update_interval.clamp(60, 7 * 86_400) <= now;
        if !due {
            continue;
        }
        let label = sub.remark.clone();
        match refresh_record(state, sub, true).await {
            Ok(_) => {
                refreshed += 1;
                tracing::info!(remark = %label, "outbound subscription refreshed");
            }
            Err(err) => {
                tracing::warn!(remark = %label, error = %err, "outbound subscription refresh failed");
            }
        }
    }
    Ok(refreshed)
}

/// Active subscription outbounds ordered for the runtime config: prepend
/// group first, then append group. Returns `(prepend, append)`.
pub async fn active_split(state: &AppState) -> Result<(Vec<Value>, Vec<Value>)> {
    let subs = db::outbound_subs::list_enabled(&state.db).await?;
    let mut prepend = Vec::new();
    let mut append = Vec::new();
    for sub in subs {
        if sub.last_fetched_outbounds.trim().is_empty() {
            continue;
        }
        let items: Vec<Value> = match serde_json::from_str(&sub.last_fetched_outbounds) {
            Ok(items) => items,
            Err(err) => {
                tracing::warn!(id = sub.id, error = %err, "corrupt outbound subscription payload");
                continue;
            }
        };
        if sub.prepend {
            prepend.extend(items);
        } else {
            append.extend(items);
        }
    }
    Ok((prepend, append))
}

pub async fn active_outbounds(state: &AppState) -> Result<Vec<Value>> {
    let (prepend, append) = active_split(state).await?;
    let mut all = prepend;
    all.extend(append);
    Ok(all)
}

// Traffic rows and routing rules may name a subscription outbound even though
// it never enters the editable template, so callers that retain known tags
// must include these.
pub async fn active_tags(state: &AppState) -> Result<Vec<String>> {
    Ok(active_outbounds(state)
        .await?
        .iter()
        .filter_map(|outbound| outbound.get("tag").and_then(Value::as_str))
        .map(str::to_string)
        .collect())
}

/// Merge subscription outbounds around the editable template outbounds. They
/// are never written back to the template; only the runtime config sees them.
pub fn merge_into_extra(extra: &mut Value, prepend: &[Value], append: &[Value]) {
    if prepend.is_empty() && append.is_empty() {
        return;
    }
    let mut outbounds = prepend.to_vec();
    match extra.get("outbounds").and_then(Value::as_array) {
        Some(existing) => outbounds.extend(existing.iter().cloned()),
        None => {
            outbounds.push(json!({"tag": "direct", "protocol": "freedom"}));
            outbounds.push(json!({"tag": "blocked", "protocol": "blackhole"}));
        }
    }
    outbounds.extend(append.iter().cloned());
    if let Some(object) = extra.as_object_mut() {
        object.insert("outbounds".into(), Value::Array(outbounds));
    }
}

pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        tokio::time::sleep(JOB_INTERVAL).await;
        let mut interval = tokio::time::interval(JOB_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            if let Err(err) = refresh_due(&state).await {
                tracing::warn!(error = %err, "outbound subscription job failed");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::OutboundSubscription;

    fn sub(id: i64, prefix: &str) -> OutboundSubscription {
        OutboundSubscription {
            id,
            remark: String::new(),
            url: "https://example.com/sub".into(),
            enabled: true,
            allow_private: false,
            allow_insecure: false,
            user_agent: String::new(),
            proxy_url: String::new(),
            tag_prefix: prefix.into(),
            update_interval: 600,
            priority: 0,
            prepend: false,
            last_updated: 0,
            last_error: String::new(),
            last_fetched_outbounds: String::new(),
            link_identities: String::new(),
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn default_prefix_frees_deleted_numbers() {
        let subs = vec![sub(1, "sub1-"), sub(2, "sub3-"), sub(3, "")];
        // 2 is free; the blank prefix reserves its own id (3), already used.
        assert_eq!(default_prefix_number(&subs, 0), 2);
        // Editing sub1 may reuse 1.
        assert_eq!(default_prefix_number(&subs, 1), 1);
    }

    #[test]
    fn blocked_private_addresses_are_rejected() {
        for address in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.1.1",
            "100.64.0.1",
            "0.0.0.0",
            "224.0.0.1",
            "::1",
            "fc00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
        ] {
            let ip: IpAddr = address.parse().unwrap();
            assert!(blocked_ip(ip), "{address} must be blocked");
        }
        for address in ["1.1.1.1", "8.8.8.8", "2606:4700:4700::1111"] {
            let ip: IpAddr = address.parse().unwrap();
            assert!(!blocked_ip(ip), "{address} must be allowed");
        }
    }

    #[test]
    fn url_sanitizer_requires_http_and_strips_credentials() {
        assert_eq!(
            sanitize_http_url(" https://user:pass@example.com/sub ").unwrap(),
            "https://example.com/sub"
        );
        assert!(sanitize_http_url("ftp://example.com/sub").is_err());
        assert!(sanitize_http_url("").is_err());
        assert!(sanitize_http_url("https://").is_err());
    }

    #[test]
    fn subscription_proxy_errors_are_chinese() {
        let io = std::io::Error::from(std::io::ErrorKind::ConnectionRefused);
        let err = anyhow::Error::new(ureq::Error::Io(io)).context("请求 https://example.com 失败");
        let message = format!(
            "{:#}",
            subscription_fetch_error(err, "http://127.0.0.1:10813", true)
        );
        assert!(message.contains("代理不可用"));
        assert!(!message.contains("Connection refused"));
        assert!(!message.contains("os error"));

        let io = std::io::Error::from(std::io::ErrorKind::ConnectionRefused);
        let direct = format!(
            "{:#}",
            subscription_fetch_error(anyhow::Error::new(ureq::Error::Io(io)), "", false)
        );
        assert!(direct.contains("无法连接订阅地址"));
    }

    #[test]
    fn proxy_url_validation_accepts_http_and_socks5_only() {
        assert_eq!(
            validate_proxy(" socks5://127.0.0.1:1080 ").unwrap(),
            "socks5://127.0.0.1:1080"
        );
        assert_eq!(
            validate_proxy("http://user:pass@proxy.example.com:8080").unwrap(),
            "http://user:pass@proxy.example.com:8080"
        );
        assert_eq!(validate_proxy("").unwrap(), "");
        assert!(validate_proxy("ftp://proxy.example.com").is_err());
        assert!(validate_proxy("socks5://").is_err());
    }

    #[test]
    fn merge_places_subscriptions_around_the_template_and_never_drops_it() {
        let mut extra = json!({"outbounds": [{"tag": "manual", "protocol": "freedom"}]});
        merge_into_extra(
            &mut extra,
            &[json!({"tag": "pre", "protocol": "vless"})],
            &[json!({"tag": "post", "protocol": "vless"})],
        );
        let tags: Vec<&str> = extra["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|outbound| outbound["tag"].as_str().unwrap())
            .collect();
        assert_eq!(tags, ["pre", "manual", "post"]);

        let mut empty = json!({});
        merge_into_extra(&mut empty, &[], &[]);
        assert!(empty.get("outbounds").is_none(), "nothing to merge");

        let mut defaulted = json!({});
        merge_into_extra(
            &mut defaulted,
            &[],
            &[json!({"tag": "post", "protocol": "vless"})],
        );
        let tags: Vec<&str> = defaulted["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|outbound| outbound["tag"].as_str().unwrap())
            .collect();
        assert_eq!(tags, ["direct", "blocked", "post"]);
    }
}
