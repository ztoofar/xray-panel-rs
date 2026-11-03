use std::collections::{HashMap, HashSet};

use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};

use crate::{
    db::models::{Client, Inbound},
    xray::identity::stat_email,
};

pub const API_TAG: &str = "api";

// Protocols Xray serves natively. MTProto, TUIC and AmneziaWG are sidecar
// protocols in 3x-ui and are not part of this panel.
pub const XRAY_INBOUND_PROTOCOLS: [&str; 11] = [
    "vless",
    "vmess",
    "trojan",
    "shadowsocks",
    "http",
    "mixed",
    "wireguard",
    "hysteria",
    "tunnel",
    "tun",
    "masque",
];

pub fn is_xray_inbound(protocol: &str) -> bool {
    XRAY_INBOUND_PROTOCOLS.contains(&protocol)
}

// Seeded as `xray_extra` and returned by "reset to default". Panel-owned
// blocks (inbounds/api/stats) are generated, so they are not part of it.
// Outbound stats start enabled (unlike 3x-ui) because the outbound traffic
// page reads those counters.
pub const DEFAULT_EXTRA_JSON: &str = r#"{
  "log": { "loglevel": "warning" },
  "dns": { "servers": ["1.1.1.1", "8.8.8.8"] },
  "outbounds": [
    { "tag": "direct", "protocol": "freedom" },
    { "tag": "blocked", "protocol": "blackhole" }
  ],
  "policy": {
    "levels": {
      "0": { "statsUserUplink": true, "statsUserDownlink": true }
    },
    "system": {
      "statsInboundUplink": true,
      "statsInboundDownlink": true,
      "statsOutboundUplink": true,
      "statsOutboundDownlink": true
    }
  },
  "routing": { "domainStrategy": "AsIs", "rules": [] }
}"#;

pub fn default_extra() -> Value {
    serde_json::from_str(DEFAULT_EXTRA_JSON).expect("default xray extra is valid JSON")
}

pub fn build_config(
    inbounds: &[Inbound],
    clients: &[Client],
    extra: &Value,
    api_port: u16,
    runtime: bool,
) -> Result<Value> {
    let mut grouped: HashMap<i64, Vec<&Client>> = HashMap::new();
    for client in clients {
        let inbound_ids = if client.inbound_ids.is_empty() {
            vec![client.inbound_id]
        } else {
            client.inbound_ids.clone()
        };
        for inbound_id in inbound_ids {
            grouped.entry(inbound_id).or_default().push(client);
        }
    }

    let mut built = Vec::with_capacity(inbounds.len() + 1);
    for inbound in inbounds {
        // Sidecar-served protocols must never reach Xray; a stray legacy row
        // would otherwise fail the whole config test.
        if !is_xray_inbound(&inbound.protocol) {
            continue;
        }
        let empty = Vec::new();
        let inbound_clients = grouped.get(&inbound.id).unwrap_or(&empty);
        built.push(build_inbound(inbound, inbound_clients)?);
    }

    built.push(json!({
        "tag": API_TAG,
        "listen": "127.0.0.1",
        "port": api_port,
        "protocol": "dokodemo-door",
        "settings": { "address": "127.0.0.1" }
    }));

    let mut config = match extra {
        Value::Object(map) => map.clone(),
        Value::Null => Map::new(),
        _ => bail!("xray extra config must be a JSON object"),
    };

    // These blocks are owned by the panel; never let stale values win.
    config.remove("inbounds");
    // Keep the user's log block (the Xray page edits log.access/error), only
    // guaranteeing a level so Xray never runs with an empty log object.
    let mut log = config
        .remove("log")
        .unwrap_or_else(|| json!({ "loglevel": "warning" }));
    if !log.is_object() {
        log = json!({ "loglevel": "warning" });
    }
    log.as_object_mut()
        .expect("object")
        .entry("loglevel".to_string())
        .or_insert_with(|| json!("warning"));
    config.insert("log".into(), log);
    config.insert("stats".into(), json!({}));
    config.insert(
        "api".into(),
        json!({
            "tag": API_TAG,
            "services": ["StatsService", "HandlerService", "RoutingService"]
        }),
    );

    let mut policy = config.remove("policy").unwrap_or_else(|| json!({}));
    ensure_policy(&mut policy);
    config.insert("policy".into(), policy);

    let mut routing = config
        .remove("routing")
        .unwrap_or_else(|| json!({ "domainStrategy": "AsIs", "rules": [] }));
    ensure_api_rule(&mut routing);
    if runtime {
        expand_routing_users(&mut routing, clients, inbounds);
    }
    config.insert("routing".into(), routing);

    if !config.get("outbounds").is_some_and(Value::is_array) {
        config.insert(
            "outbounds".into(),
            json!([
                { "tag": "direct", "protocol": "freedom" },
                { "tag": "blocked", "protocol": "blackhole" }
            ]),
        );
    }
    config.insert("inbounds".into(), Value::Array(built));

    Ok(Value::Object(config))
}

fn build_inbound(inbound: &Inbound, clients: &[&Client]) -> Result<Value> {
    let mut settings = inbound.settings.0.clone();
    if !settings.is_object() {
        bail!("inbound {}: settings must be a JSON object", inbound.id);
    }

    match inbound.protocol.as_str() {
        "vless" | "vmess" | "trojan" => {
            let shaped = build_credentials(
                inbound.protocol.as_str(),
                inbound.id,
                inbound.disable_flow,
                clients,
            )?;
            settings
                .as_object_mut()
                .expect("checked object")
                .insert("clients".into(), shaped);
        }
        "shadowsocks" => shape_shadowsocks(inbound, clients, &mut settings)?,
        "wireguard" => shape_wireguard(inbound, clients, &mut settings)?,
        "hysteria" => shape_hysteria(inbound, clients, &mut settings)?,
        "masque" => shape_masque(clients, &mut settings)?,
        "http" | "mixed" => {
            let accounts: Vec<Value> = clients
                .iter()
                .filter(|client| client.enable)
                .map(|client| {
                    let creds = client.settings.0.as_object();
                    json!({
                        "user": creds
                            .and_then(|v| v.get("user"))
                            .and_then(Value::as_str)
                            .unwrap_or(&client.email),
                        "pass": creds
                            .and_then(|v| v.get("pass"))
                            .and_then(Value::as_str)
                            .unwrap_or(""),
                    })
                })
                .collect();
            settings
                .as_object_mut()
                .expect("checked object")
                .insert("accounts".into(), Value::Array(accounts));
        }
        // tunnel/tun are client-less transports; their settings pass through.
        _ => {}
    }

    let mut object = Map::new();
    object.insert("tag".into(), Value::String(inbound.tag.clone()));
    if !inbound.listen.is_empty() {
        object.insert("listen".into(), Value::String(inbound.listen.clone()));
    }
    object.insert("port".into(), json!(inbound.port));
    object.insert("protocol".into(), Value::String(inbound.protocol.clone()));
    object.insert("settings".into(), settings);

    let mut stream = inbound.stream_settings.0.clone();
    if inbound.protocol == "hysteria" {
        if let Some(object) = stream.as_object_mut() {
            let settings = object
                .entry("hysteriaSettings")
                .or_insert_with(|| json!({}));
            if !settings.is_object() {
                *settings = json!({});
            }
            settings
                .as_object_mut()
                .expect("checked object")
                .insert("version".into(), json!(2));
        }
    }
    if stream.as_object().is_some_and(|m| !m.is_empty()) {
        object.insert("streamSettings".into(), stream);
    } else {
        object.insert("streamSettings".into(), json!({ "network": "tcp" }));
    }

    let sniffing = inbound.sniffing.0.clone();
    if sniffing.as_object().is_some_and(|m| !m.is_empty()) {
        object.insert("sniffing".into(), sniffing);
    }

    Ok(Value::Object(object))
}

fn build_credentials(
    protocol: &str,
    inbound_id: i64,
    disable_flow: bool,
    clients: &[&Client],
) -> Result<Value> {
    let mut entries = Vec::new();
    for client in clients.iter().filter(|client| client.enable) {
        let creds = client
            .settings
            .0
            .as_object()
            .with_context(|| format!("client {}: settings must be a JSON object", client.id))?;
        let mut entry = Map::new();
        entry.insert(
            "email".into(),
            Value::String(stat_email(inbound_id, &client.email)),
        );
        entry.insert("level".into(), json!(0));

        match protocol {
            "vless" => {
                entry.insert("id".into(), required(creds, "id", &client.email)?);
                // 3x-ui's "disable flow" flag wins over every client's flow.
                if !disable_flow {
                    if let Some(flow) = creds.get("flow").and_then(Value::as_str) {
                        if !flow.is_empty() {
                            entry.insert("flow".into(), Value::String(flow.to_string()));
                        }
                    }
                }
            }
            "vmess" => {
                entry.insert("id".into(), required(creds, "id", &client.email)?);
                entry.insert(
                    "alterId".into(),
                    creds.get("alterId").cloned().unwrap_or(json!(0)),
                );
                entry.insert(
                    "security".into(),
                    creds
                        .get("security")
                        .and_then(Value::as_str)
                        .filter(|value| !value.is_empty())
                        .map(|value| json!(value))
                        .unwrap_or_else(|| json!("auto")),
                );
            }
            "trojan" => {
                entry.insert(
                    "password".into(),
                    required(creds, "password", &client.email)?,
                );
            }
            _ => unreachable!("caller restricts protocol"),
        }
        entries.push(Value::Object(entry));
    }
    Ok(Value::Array(entries))
}

fn shape_shadowsocks(inbound: &Inbound, clients: &[&Client], settings: &mut Value) -> Result<()> {
    let method = settings
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("2022-blake3-aes-128-gcm")
        .to_string();
    let object = settings.as_object_mut().expect("checked object");
    object
        .entry("network")
        .or_insert_with(|| Value::String("tcp,udp".into()));

    if method.starts_with("2022-") {
        let mut entries = Vec::new();
        for client in clients.iter().filter(|client| client.enable) {
            let creds =
                client.settings.0.as_object().with_context(|| {
                    format!("client {}: settings must be a JSON object", client.id)
                })?;
            entries.push(json!({
                "method": method,
                "password": required(creds, "password", &client.email)?,
                "email": stat_email(inbound.id, &client.email),
            }));
        }
        let has_clients = !entries.is_empty();
        object.insert("clients".into(), Value::Array(entries));
        // Xray falls back to the single `password` form when there are no
        // clients; dropping it leaves an inbound with no usable key.
        if has_clients {
            object.remove("password");
        }
    } else {
        // Legacy ciphers accept one password; Xray rejects a clients array here.
        object.remove("clients");
        let first = clients.iter().find(|client| client.enable);
        match first {
            Some(client) => {
                let creds = client.settings.0.as_object().with_context(|| {
                    format!("client {}: settings must be a JSON object", client.id)
                })?;
                object.insert(
                    "password".into(),
                    required(creds, "password", &client.email)?,
                );
            }
            None => {
                object
                    .entry("password".to_string())
                    .or_insert_with(|| Value::String(String::new()));
            }
        }
    }
    if inbound.port <= 0 || inbound.port > 65535 {
        bail!("inbound {}: invalid port", inbound.id);
    }
    Ok(())
}

// WireGuard's inbound takes `peers`, not the panel's `clients` rows. Clients
// are the source of truth (3x-ui), so rebuild the peer list from them.
fn shape_wireguard(inbound: &Inbound, clients: &[&Client], settings: &mut Value) -> Result<()> {
    let object = settings.as_object_mut().expect("checked object");
    object.remove("clients");
    let mut peers = Vec::new();
    for client in clients.iter().filter(|client| client.enable) {
        let creds = client
            .settings
            .0
            .as_object()
            .with_context(|| format!("client {}: settings must be a JSON object", client.id))?;
        let Some(public_key) = creds
            .get("publicKey")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let mut peer = Map::new();
        peer.insert("publicKey".into(), json!(public_key));
        peer.insert(
            "email".into(),
            Value::String(stat_email(inbound.id, &client.email)),
        );
        peer.insert("level".into(), json!(0));
        if let Some(psk) = creds
            .get("preSharedKey")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        {
            peer.insert("preSharedKey".into(), json!(psk));
        }
        if let Some(allowed) = creds.get("allowedIPs").and_then(Value::as_array) {
            if !allowed.is_empty() {
                peer.insert("allowedIPs".into(), Value::Array(allowed.clone()));
            }
        }
        if let Some(keep_alive) = creds
            .get("keepAlive")
            .and_then(Value::as_i64)
            .filter(|value| *value > 0)
        {
            peer.insert("keepAlive".into(), json!(keep_alive));
        }
        peers.push(Value::Object(peer));
    }
    object.insert("peers".into(), Value::Array(peers));
    Ok(())
}

// xray-core builds Hysteria version 2 only and rejects the whole config for
// anything else (3x-ui heals legacy rows the same way).
fn shape_hysteria(inbound: &Inbound, clients: &[&Client], settings: &mut Value) -> Result<()> {
    let object = settings.as_object_mut().expect("checked object");
    object.insert("version".into(), json!(2));
    let mut list = Vec::new();
    for client in clients.iter().filter(|client| client.enable) {
        let creds = client
            .settings
            .0
            .as_object()
            .with_context(|| format!("client {}: settings must be a JSON object", client.id))?;
        if let Some(auth) = creds
            .get("auth")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        {
            list.push(json!({
                "auth": auth,
                "email": stat_email(inbound.id, &client.email),
                "level": 0,
            }));
        }
    }
    object.insert("clients".into(), Value::Array(list));
    Ok(())
}

// MASQUE reads `pass`, and refuses emails containing ":", so its users keep the
// raw panel email instead of the per-inbound stat identity.
fn shape_masque(clients: &[&Client], settings: &mut Value) -> Result<()> {
    let object = settings.as_object_mut().expect("checked object");
    let mut list = Vec::new();
    for client in clients.iter().filter(|client| client.enable) {
        let creds = client
            .settings
            .0
            .as_object()
            .with_context(|| format!("client {}: settings must be a JSON object", client.id))?;
        if let Some(password) = creds
            .get("password")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        {
            list.push(json!({
                "email": client.email,
                "pass": password,
                "level": 0,
            }));
        }
    }
    object.insert("clients".into(), Value::Array(list));
    Ok(())
}

fn required(creds: &Map<String, Value>, key: &str, email: &str) -> Result<Value> {
    let value = creds.get(key).cloned().unwrap_or(Value::Null);
    match value.as_str() {
        Some(text) if !text.is_empty() => Ok(Value::String(text.to_string())),
        _ => bail!("client {email}: missing credential \"{key}\""),
    }
}

fn ensure_policy(policy: &mut Value) {
    if !policy.is_object() {
        *policy = json!({});
    }
    let object = policy.as_object_mut().expect("object");
    let levels = object.entry("levels").or_insert_with(|| json!({}));
    if !levels.is_object() {
        *levels = json!({});
    }
    let levels = levels.as_object_mut().expect("object");
    levels.entry("0").or_insert_with(|| json!({}));
    // 3x-ui forces statsUserOnline on every level: clients may carry an
    // explicit level from the template, and only tracked levels report online
    // IPs, so level 0 alone silently loses connection-based online (and IP
    // limits) for those clients.
    for level in levels.values_mut() {
        if !level.is_object() {
            *level = json!({});
        }
        level
            .as_object_mut()
            .expect("level object")
            .insert("statsUserOnline".into(), json!(true));
    }
    // Traffic accounting needs these on level 0; an explicit false from the
    // template still wins (3x-ui only forces statsUserOnline).
    let level_zero = levels
        .get_mut("0")
        .and_then(Value::as_object_mut)
        .expect("level 0 object");
    level_zero
        .entry("statsUserUplink")
        .or_insert_with(|| json!(true));
    level_zero
        .entry("statsUserDownlink")
        .or_insert_with(|| json!(true));

    let system = object.entry("system").or_insert_with(|| json!({}));
    if !system.is_object() {
        *system = json!({});
    }
    let system = system.as_object_mut().expect("object");
    for key in [
        "statsInboundUplink",
        "statsInboundDownlink",
        "statsOutboundUplink",
        "statsOutboundDownlink",
    ] {
        system.entry(key).or_insert_with(|| json!(true));
    }
}

// Xray's user stats are keyed by the generated attachment identity, so a
// routing rule written with the panel email must be expanded to every
// attachment or it silently stops matching after this change. Only the
// runtime config is expanded; the API keeps showing the panel email.
fn expand_routing_users(routing: &mut Value, clients: &[Client], inbounds: &[Inbound]) {
    let enabled: HashSet<i64> = inbounds.iter().map(|inbound| inbound.id).collect();
    let mut identities: HashMap<&str, Vec<String>> = HashMap::new();
    for client in clients {
        let ids = if client.inbound_ids.is_empty() {
            vec![client.inbound_id]
        } else {
            client.inbound_ids.clone()
        };
        for id in ids {
            if enabled.contains(&id) {
                identities
                    .entry(client.email.as_str())
                    .or_default()
                    .push(stat_email(id, &client.email));
            }
        }
    }
    let Some(rules) = routing.get_mut("rules").and_then(Value::as_array_mut) else {
        return;
    };
    for rule in rules {
        let Some(users) = rule.get_mut("user").and_then(Value::as_array_mut) else {
            continue;
        };
        let mut expanded = Vec::with_capacity(users.len());
        let mut seen = HashSet::new();
        for user in users.iter() {
            let Some(ids) = user.as_str().and_then(|email| identities.get(email)) else {
                expanded.push(user.clone());
                continue;
            };
            for identity in ids {
                if seen.insert(identity.clone()) {
                    expanded.push(Value::String(identity.clone()));
                }
            }
        }
        *users = expanded;
    }
}

fn ensure_api_rule(routing: &mut Value) {
    if !routing.is_object() {
        *routing = json!({});
    }
    let object = routing.as_object_mut().expect("object");
    let rules = object.entry("rules").or_insert_with(|| json!([]));
    if !rules.is_array() {
        *rules = json!([]);
    }
    let rules = rules.as_array_mut().expect("array");
    // The generated config is also the payload the routing page saves back, so
    // strip every previously-injected API rule before adding today's one.
    rules.retain(|rule| {
        let api_inbound = rule
            .get("inboundTag")
            .and_then(Value::as_array)
            .is_some_and(|tags| tags.iter().any(|tag| tag.as_str() == Some(API_TAG)));
        let api_outbound = rule.get("outboundTag").and_then(Value::as_str) == Some(API_TAG);
        !(api_inbound && api_outbound)
    });
    rules.insert(
        0,
        json!({ "type": "field", "inboundTag": [API_TAG], "outboundTag": API_TAG }),
    );
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sqlx::types::Json;

    use super::*;

    fn inbound(id: i64, protocol: &str, settings: Value) -> Inbound {
        Inbound {
            id,
            user_id: 1,
            up: 0,
            down: 0,
            last_up: 0,
            last_down: 0,
            total: 0,
            remark: "test".into(),
            sub_sort_index: 1,
            exclude_from_sub: false,
            enable: true,
            expiry_time: 0,
            traffic_reset: "never".into(),
            traffic_reset_day: 1,
            last_traffic_reset_time: 0,
            listen: String::new(),
            port: 443,
            protocol: protocol.into(),
            settings: Json(settings),
            stream_settings: Json(json!({"network":"tcp","security":"reality"})),
            sniffing: Json(json!({})),
            tag: format!("in-{protocol}-{id}"),
            share_addr_strategy: "node".into(),
            share_addr: String::new(),
            disable_flow: false,
            created_at: 0,
            updated_at: 0,
        }
    }

    fn client(id: i64, inbound_id: i64, settings: Value) -> Client {
        Client {
            id,
            inbound_id,
            email: format!("u{id}"),
            enable: true,
            up: 0,
            down: 0,
            last_up: 0,
            last_down: 0,
            total: 0,
            expiry_time: 0,
            settings: Json(settings),
            comment: String::new(),
            group_name: String::new(),
            limit_ip: 0,
            limit_hwid: 0,
            reset_days: 0,
            last_reset_time: 0,
            last_online: 0,
            inbound_ids: vec![inbound_id],
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn api_rule_is_replaced_instead_of_duplicated() {
        let mut routing = json!({
            "rules": [
                { "type": "field", "inboundTag": ["api"], "outboundTag": "api" },
                { "type": "field", "outboundTag": "direct" }
            ]
        });
        ensure_api_rule(&mut routing);
        ensure_api_rule(&mut routing);
        let rules = routing["rules"].as_array().unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(
            rules
                .iter()
                .filter(|rule| rule["outboundTag"] == "api")
                .count(),
            1
        );
        assert_eq!(rules[0]["inboundTag"][0], json!(API_TAG));
    }

    #[test]
    fn vless_clients_are_injected_and_api_rule_prepended() {
        let inbounds = vec![inbound(1, "vless", json!({"decryption":"none"}))];
        let clients = vec![client(
            1,
            1,
            json!({"id":"uuid-1","flow":"xtls-rprx-vision"}),
        )];
        let config = build_config(
            &inbounds,
            &clients,
            &json!({"routing":{"rules":[{"type":"field","outboundTag":"direct"}]}}),
            10085,
            false,
        )
        .expect("config");

        let generated = &config["inbounds"][0];
        assert_eq!(generated["tag"], "in- vless-1".replace(' ', ""));
        assert_eq!(generated["settings"]["clients"][0]["id"], "uuid-1");
        assert_eq!(generated["settings"]["clients"][0]["email"], "1:u1");
        assert_eq!(
            generated["settings"]["clients"][0]["flow"],
            "xtls-rprx-vision"
        );
        assert_eq!(config["routing"]["rules"][0]["inboundTag"][0], API_TAG);
        assert_eq!(config["policy"]["levels"]["0"]["statsUserUplink"], true);
        assert_eq!(config["inbounds"][1]["tag"], API_TAG);
    }

    #[test]
    fn every_policy_level_tracks_online_users_like_xui() {
        let inbounds = vec![inbound(1, "vless", json!({"decryption":"none"}))];
        let extra = json!({
            "policy": {
                "levels": {
                    "0": {"handshake": 4},
                    "1": {"statsUserOnline": false}
                }
            }
        });
        let config = build_config(
            &inbounds,
            &[client(1, 1, json!({"id":"uuid-1"}))],
            &extra,
            10085,
            false,
        )
        .expect("config");
        assert_eq!(config["policy"]["levels"]["0"]["statsUserOnline"], true);
        assert_eq!(config["policy"]["levels"]["1"]["statsUserOnline"], true);
        // A client carrying an explicit level must still be tracked online.
        assert_eq!(config["policy"]["levels"]["0"]["handshake"], 4);
    }

    #[test]
    fn connection_limits_and_explicit_stats_choice_survive_generation() {
        let inbounds = vec![inbound(1, "vless", json!({"decryption":"none"}))];
        let extra = json!({
            "policy": {
                "levels": { "0": { "connIdle": 123, "bufferSize": 64 } },
                "system": { "statsOutboundUplink": false }
            }
        });
        let config = build_config(
            &inbounds,
            &[client(1, 1, json!({"id":"uuid-1"}))],
            &extra,
            10085,
            false,
        )
        .expect("config");
        let policy = &config["policy"];
        assert_eq!(policy["levels"]["0"]["connIdle"], 123);
        assert_eq!(policy["levels"]["0"]["bufferSize"], 64);
        // Missing traffic flags still default on so accounting keeps working.
        assert_eq!(policy["levels"]["0"]["statsUserUplink"], true);
        assert_eq!(policy["levels"]["0"]["statsUserDownlink"], true);
        assert_eq!(policy["levels"]["0"]["statsUserOnline"], true);
        assert_eq!(policy["system"]["statsInboundUplink"], true);
        // An explicit false is the user's statistics switch, not ours to undo.
        assert_eq!(policy["system"]["statsOutboundUplink"], false);
    }

    #[test]
    fn disable_flow_removes_flow_from_generated_clients() {
        let mut inbound = inbound(1, "vless", json!({"decryption":"none"}));
        inbound.disable_flow = true;
        let config = build_config(
            &[inbound],
            &[client(
                1,
                1,
                json!({"id":"uuid-1","flow":"xtls-rprx-vision"}),
            )],
            &json!({}),
            10085,
            false,
        )
        .expect("config");
        assert!(config["inbounds"][0]["settings"]["clients"][0]
            .get("flow")
            .is_none());
    }

    #[test]
    fn disabled_clients_are_omitted() {
        let inbounds = vec![inbound(1, "trojan", json!({}))];
        let mut disabled = client(2, 1, json!({"password":"secret"}));
        disabled.enable = false;
        let config = build_config(
            &inbounds,
            &[client(1, 1, json!({"password":"one"})), disabled],
            &json!({}),
            10085,
            false,
        )
        .expect("config");
        let clients = config["inbounds"][0]["settings"]["clients"]
            .as_array()
            .expect("array");
        assert_eq!(clients.len(), 1);
        assert_eq!(clients[0]["email"], "1:u1");
    }

    #[test]
    fn shadowsocks_2022_keeps_clients_array() {
        let inbounds = vec![inbound(
            1,
            "shadowsocks",
            json!({"method":"2022-blake3-aes-128-gcm"}),
        )];
        let config = build_config(
            &inbounds,
            &[client(1, 1, json!({"password":"abc"}))],
            &json!({}),
            10085,
            false,
        )
        .expect("config");
        assert_eq!(
            config["inbounds"][0]["settings"]["clients"][0]["password"],
            "abc"
        );
        assert_eq!(
            config["inbounds"][0]["settings"]["clients"][0]["method"],
            "2022-blake3-aes-128-gcm"
        );
        assert_eq!(
            config["inbounds"][0]["settings"]["clients"][0]["email"],
            "1:u1"
        );
    }

    #[test]
    fn wireguard_clients_become_peers_and_sidecar_protocols_never_reach_xray() {
        let mut wg = inbound(
            1,
            "wireguard",
            json!({"secretKey":"server-key","clients":[{"email":"ignored"}]}),
        );
        wg.stream_settings = Json(json!({"security":"none"}));
        let sidecar = inbound(2, "mtproto", json!({"clients":[]}));
        let mut client_settings = json!({
            "publicKey": "peer-key",
            "allowedIPs": ["10.0.0.2/32"],
            "keepAlive": 25,
        });
        client_settings["email"] = json!("wg-user");
        let config = build_config(
            &[wg, sidecar],
            &[client(1, 1, client_settings)],
            &json!({}),
            10085,
            false,
        )
        .expect("config");

        let inbounds = config["inbounds"].as_array().unwrap();
        assert_eq!(inbounds.len(), 2, "wireguard + api only");
        let generated = &inbounds[0];
        assert_eq!(generated["protocol"], "wireguard");
        assert!(generated["settings"].get("clients").is_none());
        let peers = generated["settings"]["peers"].as_array().unwrap();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0]["publicKey"], "peer-key");
        assert_eq!(peers[0]["email"], "1:u1");
        assert_eq!(peers[0]["allowedIPs"][0], "10.0.0.2/32");
        assert_eq!(peers[0]["keepAlive"], 25);
    }

    #[test]
    fn hysteria_is_forced_to_version_two_and_gets_client_auths() {
        let mut hy = inbound(1, "hysteria", json!({"version":1}));
        hy.stream_settings = Json(json!({
            "network":"hysteria",
            "security":"tls",
            "hysteriaSettings":{"version":1,"udpIdleTimeout":60}
        }));
        let config = build_config(
            &[hy],
            &[client(1, 1, json!({"auth":"secret"}))],
            &json!({}),
            10085,
            false,
        )
        .expect("config");
        let generated = &config["inbounds"][0];
        assert_eq!(generated["settings"]["version"], 2);
        assert_eq!(
            generated["streamSettings"]["hysteriaSettings"]["version"],
            2
        );
        assert_eq!(
            generated["streamSettings"]["hysteriaSettings"]["udpIdleTimeout"],
            60
        );
        assert_eq!(generated["settings"]["clients"][0]["auth"], "secret");
        assert_eq!(generated["settings"]["clients"][0]["email"], "1:u1");
    }

    #[test]
    fn masque_uses_pass_and_keeps_unprefixed_emails() {
        let config = build_config(
            &[inbound(
                1,
                "masque",
                json!({"address":["10.14.0.1/24"],"clients":[]}),
            )],
            &[client(1, 1, json!({"password":"pw"}))],
            &json!({}),
            10085,
            false,
        )
        .expect("config");
        let user = &config["inbounds"][0]["settings"]["clients"][0];
        assert_eq!(user["pass"], "pw");
        assert_eq!(user["email"], "u1");
        assert!(user.get("password").is_none());
    }

    #[test]
    fn tunnel_and_tun_settings_pass_through() {
        let tunnel = inbound(1, "tunnel", json!({"allowedNetwork":"tcp","portMap":{}}));
        let tun = inbound(2, "tun", json!({"name":"xray0","mtu":1500}));
        let config = build_config(&[tunnel, tun], &[], &json!({}), 10085, false).expect("config");
        assert_eq!(config["inbounds"][0]["settings"]["allowedNetwork"], "tcp");
        assert_eq!(config["inbounds"][1]["settings"]["name"], "xray0");
    }

    #[test]
    fn runtime_routing_expands_panel_emails_to_attachment_identities() {
        let inbounds = vec![
            inbound(1, "vless", json!({"decryption":"none"})),
            inbound(2, "vless", json!({"decryption":"none"})),
        ];
        let mut multi = client(1, 1, json!({"id":"uuid-1"}));
        multi.inbound_ids = vec![1, 2];
        let extra = json!({
            "routing": {"rules": [{"type":"field", "user":["u1"], "outboundTag":"direct"}]}
        });

        let runtime =
            build_config(&inbounds, &[multi.clone()], &extra, 10085, true).expect("runtime config");
        let users = runtime["routing"]["rules"][1]["user"]
            .as_array()
            .expect("user array");
        assert_eq!(users, &vec![json!("1:u1"), json!("2:u1")]);

        // The API-facing config keeps the panel email so the routing editor
        // never shows the internal attachment identity.
        let display =
            build_config(&inbounds, &[multi], &extra, 10085, false).expect("display config");
        let users = display["routing"]["rules"][1]["user"]
            .as_array()
            .expect("user array");
        assert_eq!(users, &vec![json!("u1")]);
    }
}
