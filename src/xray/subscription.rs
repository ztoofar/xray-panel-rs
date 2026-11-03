use std::collections::{HashMap, HashSet};

use anyhow::{bail, Context, Result};
use base64::{
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
    Engine as _,
};
use percent_encoding::percent_decode_str;
use serde_json::{json, Map, Value};
use url::{form_urlencoded, Host, Url};

/// One parsed subscription entry plus its refresh-stable identity.
pub struct ParsedOutbound {
    pub outbound: Value,
    pub identity: String,
}

/// Parse a subscription body: a plain newline list of share links or the common
/// base64-encoded wrapper. Unsupported or malformed lines are skipped.
pub fn parse_subscription_body(body: &[u8]) -> Vec<ParsedOutbound> {
    let text = String::from_utf8_lossy(body);
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    let decoded = decode_base64_text(text).unwrap_or_else(|| text.to_string());
    let mut out = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    for line in split_lines(&decoded) {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(mut parsed) = parse_link(line) else {
            continue;
        };
        // Duplicate identities would share one tag and shift every tag later.
        let count = seen.entry(parsed.identity.clone()).or_insert(0);
        if *count > 0 {
            parsed.identity = format!("{}#{}", parsed.identity, count);
        }
        *count += 1;
        out.push(parsed);
    }
    out
}

pub fn parse_link(link: &str) -> Option<ParsedOutbound> {
    let link = link.trim();
    if link.starts_with("vmess://") {
        parse_vmess(link).ok()
    } else if link.starts_with("vless://") {
        parse_vless(link).ok()
    } else if link.starts_with("trojan://") {
        parse_trojan(link).ok()
    } else if link.starts_with("ss://") {
        parse_shadowsocks(link).ok()
    } else if link.starts_with("hysteria2://") || link.starts_with("hy2://") {
        parse_hysteria2(link).ok()
    } else if link.starts_with("wireguard://") || link.starts_with("wg://") {
        parse_wireguard(link).ok()
    } else {
        None
    }
}

fn split_lines(text: &str) -> Vec<String> {
    text.replace("\\n", "\n")
        .split(['\n', '\r'])
        .map(str::to_string)
        .collect()
}

fn decode_base64_text(text: &str) -> Option<String> {
    let bytes = decode_base64_bytes(text)?;
    String::from_utf8(bytes).ok()
}

fn decode_base64_bytes(text: &str) -> Option<Vec<u8>> {
    let clean: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
    if clean.is_empty() {
        return None;
    }
    let mut padded = clean.clone();
    while padded.len() % 4 != 0 {
        padded.push('=');
    }
    STANDARD
        .decode(&padded)
        .ok()
        .or_else(|| URL_SAFE.decode(&padded).ok())
        .or_else(|| STANDARD_NO_PAD.decode(clean.trim_end_matches('=')).ok())
        .or_else(|| URL_SAFE_NO_PAD.decode(clean.trim_end_matches('=')).ok())
}

fn decode_component(value: &str) -> String {
    percent_decode_str(value).decode_utf8_lossy().into_owned()
}

fn fragment(url: &Url) -> String {
    url.fragment().map(decode_component).unwrap_or_default()
}

fn param<'a>(params: &'a [(String, String)], key: &str) -> &'a str {
    params
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
        .unwrap_or("")
}

fn first_non_empty<'a>(first: &'a str, second: &'a str) -> &'a str {
    if first.is_empty() {
        second
    } else {
        first
    }
}

fn host_of(url: &Url) -> Result<String> {
    match url.host() {
        Some(Host::Domain(domain)) => Ok(domain.to_string()),
        Some(Host::Ipv4(ip)) => Ok(ip.to_string()),
        Some(Host::Ipv6(ip)) => Ok(ip.to_string()),
        None => bail!("URL host is required"),
    }
}

fn query_pairs(url: &Url) -> Vec<(String, String)> {
    url.query_pairs().into_owned().collect()
}

fn default_port(port: Option<u16>, default: u16) -> u16 {
    port.filter(|value| *value > 0).unwrap_or(default)
}

fn split_comma(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn split_comma_or(value: &str, default: &[&str]) -> Vec<String> {
    if value.is_empty() {
        default.iter().map(|item| (*item).to_string()).collect()
    } else {
        split_comma(value)
    }
}

fn canonical_query(params: &[(String, String)], security: &str) -> String {
    let reality = security == "reality";
    let mut pairs: Vec<&(String, String)> = params
        .iter()
        .filter(|(key, _)| !(reality && matches!(key.as_str(), "sid" | "sni" | "spx")))
        .collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    pairs
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&")
}

// --- stream settings ---

fn object_entry<'a>(object: &'a mut Map<String, Value>, key: &str) -> &'a mut Map<String, Value> {
    if !object.get(key).is_some_and(Value::is_object) {
        object.insert(key.to_string(), Value::Object(Map::new()));
    }
    object
        .get_mut(key)
        .and_then(Value::as_object_mut)
        .expect("object inserted above")
}

fn build_stream(network: &str, security: &str) -> Value {
    let mut stream = Map::new();
    stream.insert("network".into(), json!(network));
    stream.insert("security".into(), json!(security));
    match network {
        "ws" => {
            stream.insert(
                "wsSettings".into(),
                json!({"path": "/", "host": "", "headers": {}, "heartbeatPeriod": 0}),
            );
        }
        "grpc" => {
            stream.insert(
                "grpcSettings".into(),
                json!({"serviceName": "", "authority": "", "multiMode": false}),
            );
        }
        "httpupgrade" => {
            stream.insert(
                "httpupgradeSettings".into(),
                json!({"path": "/", "host": "", "headers": {}}),
            );
        }
        "xhttp" => {
            stream.insert(
                "xhttpSettings".into(),
                json!({"path": "/", "host": "", "mode": "auto", "headers": {}, "xPaddingBytes": "100-1000"}),
            );
        }
        "kcp" => {
            stream.insert(
                "kcpSettings".into(),
                json!({
                    "mtu": 1350, "tti": 20, "uplinkCapacity": 5, "downlinkCapacity": 20,
                    "cwndMultiplier": 1, "maxSendingWindow": 2097152
                }),
            );
        }
        _ => {
            stream.insert("tcpSettings".into(), json!({"header": {"type": "none"}}));
        }
    }
    match security {
        "tls" => {
            stream.insert(
                "tlsSettings".into(),
                json!({
                    "serverName": "", "alpn": [], "fingerprint": "",
                    "echConfigList": "", "verifyPeerCertByName": "",
                    "pinnedPeerCertSha256": "", "cipherSuites": ""
                }),
            );
        }
        "reality" => {
            stream.insert(
                "realitySettings".into(),
                json!({
                    "publicKey": "", "fingerprint": "chrome", "serverName": "",
                    "shortId": "", "spiderX": "", "mldsa65Verify": ""
                }),
            );
        }
        _ => {}
    }
    Value::Object(stream)
}

fn apply_transport(stream: &mut Value, params: &[(String, String)]) {
    let network = stream
        .get("network")
        .and_then(Value::as_str)
        .unwrap_or("tcp")
        .to_string();
    let host = param(params, "host");
    let path = first_non_empty(param(params, "path"), "/");
    let Some(stream) = stream.as_object_mut() else {
        return;
    };
    match network.as_str() {
        "ws" => {
            let ws = object_entry(stream, "wsSettings");
            ws.insert("host".into(), json!(host));
            ws.insert("path".into(), json!(path));
        }
        "grpc" => {
            let grpc = object_entry(stream, "grpcSettings");
            grpc.insert(
                "serviceName".into(),
                json!(first_non_empty(param(params, "serviceName"), path)),
            );
            grpc.insert("authority".into(), json!(param(params, "authority")));
            grpc.insert("multiMode".into(), json!(param(params, "mode") == "multi"));
        }
        "httpupgrade" => {
            let upgrade = object_entry(stream, "httpupgradeSettings");
            upgrade.insert("host".into(), json!(host));
            upgrade.insert("path".into(), json!(path));
        }
        "xhttp" => {
            let xhttp = object_entry(stream, "xhttpSettings");
            xhttp.insert("host".into(), json!(host));
            xhttp.insert("path".into(), json!(path));
            if let Some(mode) = params
                .iter()
                .find(|(key, _)| key == "mode")
                .map(|(_, value)| value.as_str())
                .filter(|value| !value.is_empty())
            {
                xhttp.insert("mode".into(), json!(mode));
            }
            if let Some(padding) = params
                .iter()
                .find(|(key, _)| key == "x_padding_bytes")
                .map(|(_, value)| value.as_str())
                .filter(|value| !value.is_empty())
            {
                xhttp.insert("xPaddingBytes".into(), json!(padding));
            }
            if let Some(extra) = params
                .iter()
                .find(|(key, _)| key == "extra")
                .map(|(_, value)| value.as_str())
                .filter(|value| !value.is_empty())
            {
                if let Ok(Value::Object(extra)) = serde_json::from_str::<Value>(extra) {
                    for (key, value) in extra {
                        xhttp.insert(key, value);
                    }
                }
            }
        }
        "kcp" => {
            let kcp = object_entry(stream, "kcpSettings");
            if let Some(mtu) = parse_in_range(param(params, "mtu"), 21, i32::MAX as i64) {
                kcp.insert("mtu".into(), json!(mtu));
            }
            if let Some(tti) = parse_in_range(param(params, "tti"), 10, 1000) {
                kcp.insert("tti".into(), json!(tti));
            }
        }
        "tcp" if param(params, "headerType") == "http" || param(params, "type") == "http" => {
            stream.insert(
                "tcpSettings".into(),
                json!({
                    "header": {
                        "type": "http",
                        "request": {
                            "version": "1.1",
                            "method": "GET",
                            "path": split_comma(path),
                            "headers": {"Host": split_comma(host)}
                        }
                    }
                }),
            );
        }
        _ => {}
    }
}

fn parse_in_range(value: &str, min: i64, max: i64) -> Option<i64> {
    let parsed: i64 = value.trim().parse().ok()?;
    (parsed >= min && parsed <= max).then_some(parsed)
}

fn apply_security(stream: &mut Value, params: &[(String, String)]) {
    let security = stream
        .get("security")
        .and_then(Value::as_str)
        .unwrap_or("none")
        .to_string();
    let Some(stream) = stream.as_object_mut() else {
        return;
    };
    match security.as_str() {
        "tls" => {
            let tls = object_entry(stream, "tlsSettings");
            tls.insert("serverName".into(), json!(param(params, "sni")));
            tls.insert("fingerprint".into(), json!(param(params, "fp")));
            let alpn = param(params, "alpn");
            if !alpn.is_empty() {
                tls.insert("alpn".into(), json!(split_comma(alpn)));
            }
            tls.insert("echConfigList".into(), json!(param(params, "ech")));
            tls.insert("verifyPeerCertByName".into(), json!(param(params, "vcn")));
            tls.insert("pinnedPeerCertSha256".into(), json!(param(params, "pcs")));
            let cipher_suites = param(params, "cs");
            if !cipher_suites.is_empty() {
                tls.insert("cipherSuites".into(), json!(cipher_suites));
            }
        }
        "reality" => {
            let reality = object_entry(stream, "realitySettings");
            reality.insert("serverName".into(), json!(param(params, "sni")));
            reality.insert(
                "fingerprint".into(),
                json!(first_non_empty(param(params, "fp"), "chrome")),
            );
            reality.insert("publicKey".into(), json!(param(params, "pbk")));
            reality.insert("shortId".into(), json!(param(params, "sid")));
            reality.insert("spiderX".into(), json!(param(params, "spx")));
            reality.insert("mldsa65Verify".into(), json!(param(params, "pqv")));
        }
        _ => {}
    }
}

fn apply_final_mask(stream: &mut Value, params: &[(String, String)]) {
    let mask = param(params, "fm");
    if mask.is_empty() {
        return;
    }
    if let Ok(Value::Object(mask)) = serde_json::from_str::<Value>(mask) {
        if let Some(stream) = stream.as_object_mut() {
            stream.insert("finalmask".into(), Value::Object(mask));
        }
    }
}

fn append_udp_mask(stream: &mut Value, mask: Value) {
    let Some(stream) = stream.as_object_mut() else {
        return;
    };
    let finalmask = object_entry(stream, "finalmask");
    let udp = finalmask
        .entry("udp")
        .or_insert_with(|| Value::Array(Vec::new()));
    if let Some(udp) = udp.as_array_mut() {
        udp.push(mask);
    }
}

fn apply_mkcp_legacy(stream: &mut Value, header_type: &str, seed: &str) {
    let header = match header_type.trim() {
        "" | "none" => "",
        "dns" => "dns",
        "dtls" => "dtls",
        "srtp" => "srtp",
        "utp" => "utp",
        "wechat-video" => "wechat",
        "wireguard" => "wireguard",
        _ => return,
    };
    if header.is_empty() && seed.is_empty() {
        return;
    }
    if stream.get("network").and_then(Value::as_str) != Some("kcp") {
        return;
    }
    if seed.is_empty() {
        append_udp_mask(
            stream,
            json!({"type": "mkcp-legacy", "settings": {"header": header, "value": ""}}),
        );
        return;
    }
    if header.is_empty() {
        append_udp_mask(
            stream,
            json!({"type": "mkcp-legacy", "settings": {"header": "", "value": seed}}),
        );
        return;
    }
    append_udp_mask(
        stream,
        json!({"type": "mkcp-legacy", "settings": {"header": "", "value": seed}}),
    );
    append_udp_mask(
        stream,
        json!({"type": "mkcp-legacy", "settings": {"header": header, "value": ""}}),
    );
}

// --- vless / trojan ---

fn parse_vless(link: &str) -> Result<ParsedOutbound> {
    let url = Url::parse(link).context("parse vless link")?;
    if url.scheme() != "vless" {
        bail!("not vless");
    }
    let id = decode_component(url.username());
    let host = host_of(&url)?;
    let port = default_port(url.port(), 443);
    let params = query_pairs(&url);
    let network = first_non_empty(param(&params, "type"), "tcp");
    let security = first_non_empty(param(&params, "security"), "none");
    let mut stream = build_stream(network, security);
    apply_transport(&mut stream, &params);
    apply_security(&mut stream, &params);
    apply_final_mask(&mut stream, &params);

    let identity = format!(
        "vless:vless://{}@{}:{}?{}",
        id,
        host,
        port,
        canonical_query(&params, security)
    );
    let mut settings = Map::new();
    settings.insert("address".into(), json!(host));
    settings.insert("port".into(), json!(port));
    settings.insert("id".into(), json!(id));
    let flow = param(&params, "flow");
    if !flow.is_empty() {
        settings.insert("flow".into(), json!(flow));
    }
    settings.insert(
        "encryption".into(),
        json!(first_non_empty(param(&params, "encryption"), "none")),
    );
    Ok(ParsedOutbound {
        outbound: json!({
            "protocol": "vless",
            "tag": fragment(&url),
            "settings": settings,
            "streamSettings": stream,
        }),
        identity,
    })
}

fn parse_trojan(link: &str) -> Result<ParsedOutbound> {
    let url = Url::parse(link).context("parse trojan link")?;
    if url.scheme() != "trojan" {
        bail!("not trojan");
    }
    let password = decode_component(url.username());
    let host = host_of(&url)?;
    let port = default_port(url.port(), 443);
    let params = query_pairs(&url);
    let network = first_non_empty(param(&params, "type"), "tcp");
    let security = first_non_empty(param(&params, "security"), "tls");
    let mut stream = build_stream(network, security);
    apply_transport(&mut stream, &params);
    apply_security(&mut stream, &params);
    apply_final_mask(&mut stream, &params);

    let identity = format!(
        "trojan:trojan://{}@{}:{}?{}",
        password,
        host,
        port,
        canonical_query(&params, security)
    );
    Ok(ParsedOutbound {
        outbound: json!({
            "protocol": "trojan",
            "tag": fragment(&url),
            "settings": {
                "servers": [{"address": host, "port": port, "password": password}]
            },
            "streamSettings": stream,
        }),
        identity,
    })
}

// --- vmess ---

fn parse_vmess(link: &str) -> Result<ParsedOutbound> {
    let encoded = link.strip_prefix("vmess://").context("not vmess")?;
    let raw = decode_base64_bytes(encoded).context("decode vmess payload")?;
    let value: Value = serde_json::from_slice(&raw).context("parse vmess JSON")?;
    let j = value
        .as_object()
        .context("vmess payload is not an object")?;

    let identity = {
        let core: Map<String, Value> = j
            .iter()
            .filter(|(key, _)| key.as_str() != "ps")
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        format!("vmess:{}", serde_json::to_string(&core)?)
    };

    let network = get_str(j, "net", "tcp");
    let security = if get_str(j, "tls", "") == "tls" {
        "tls"
    } else {
        "none"
    };
    let mut stream = build_stream(&network, security);
    let host = get_str(j, "host", "");
    let path = get_str(j, "path", "/");
    match network.as_str() {
        "ws" => {
            if let Some(ws) = stream.get_mut("wsSettings").and_then(Value::as_object_mut) {
                ws.insert("host".into(), json!(host));
                ws.insert("path".into(), json!(path));
            }
        }
        "grpc" => {
            if let Some(grpc) = stream
                .get_mut("grpcSettings")
                .and_then(Value::as_object_mut)
            {
                grpc.insert("serviceName".into(), json!(get_str(j, "path", "")));
                grpc.insert("authority".into(), json!(get_str(j, "authority", "")));
                grpc.insert("multiMode".into(), json!(get_str(j, "type", "") == "multi"));
            }
        }
        "httpupgrade" => {
            if let Some(upgrade) = stream
                .get_mut("httpupgradeSettings")
                .and_then(Value::as_object_mut)
            {
                upgrade.insert("host".into(), json!(host));
                upgrade.insert("path".into(), json!(path));
            }
        }
        "xhttp" => {
            if let Some(xhttp) = stream
                .get_mut("xhttpSettings")
                .and_then(Value::as_object_mut)
            {
                xhttp.insert("host".into(), json!(host));
                xhttp.insert("path".into(), json!(path));
                let mode = get_str(j, "mode", "");
                if !mode.is_empty() {
                    xhttp.insert("mode".into(), json!(mode));
                }
            }
        }
        "tcp" => {
            if get_str(j, "type", "") == "http" {
                if let Some(object) = stream.as_object_mut() {
                    object.insert(
                        "tcpSettings".into(),
                        json!({
                            "header": {
                                "type": "http",
                                "request": {
                                    "version": "1.1",
                                    "method": "GET",
                                    "path": split_comma(&path),
                                    "headers": {"Host": split_comma(&host)}
                                }
                            }
                        }),
                    );
                }
            }
        }
        "kcp" => {
            if let Some(kcp) = stream.get_mut("kcpSettings").and_then(Value::as_object_mut) {
                if let Some(mtu) =
                    num(&j.get("mtu").cloned().unwrap_or(Value::Null)).filter(|value| *value >= 21)
                {
                    kcp.insert("mtu".into(), json!(mtu));
                }
                if let Some(tti) = num(&j.get("tti").cloned().unwrap_or(Value::Null))
                    .filter(|value| (10..=1000).contains(value))
                {
                    kcp.insert("tti".into(), json!(tti));
                }
            }
        }
        _ => {}
    }
    if security == "tls" {
        if let Some(tls) = stream.get_mut("tlsSettings").and_then(Value::as_object_mut) {
            tls.insert("serverName".into(), json!(get_str(j, "sni", "")));
            tls.insert("fingerprint".into(), json!(get_str(j, "fp", "")));
            let alpn = get_str(j, "alpn", "");
            if !alpn.is_empty() {
                tls.insert("alpn".into(), json!(split_comma(&alpn)));
            }
            tls.insert("echConfigList".into(), json!(get_str(j, "ech", "")));
            tls.insert("verifyPeerCertByName".into(), json!(get_str(j, "vcn", "")));
            tls.insert("pinnedPeerCertSha256".into(), json!(get_str(j, "pcs", "")));
            tls.insert("cipherSuites".into(), json!(get_str(j, "cs", "")));
        }
    }
    if let Some(mask) = j
        .get("fm")
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
    {
        if let Ok(Value::Object(mask)) = serde_json::from_str::<Value>(mask) {
            if let Some(stream) = stream.as_object_mut() {
                stream.insert("finalmask".into(), Value::Object(mask));
            }
        }
    }
    if network == "kcp" {
        apply_mkcp_legacy(
            &mut stream,
            &get_str(j, "type", ""),
            &get_str(j, "path", ""),
        );
    }

    let port = num(&j.get("port").cloned().unwrap_or(Value::Null)).unwrap_or(0);
    let mut scy = get_str(j, "scy", "auto");
    if scy == "none" || scy == "zero" {
        scy = "auto".to_string();
    }
    Ok(ParsedOutbound {
        outbound: json!({
            "protocol": "vmess",
            "tag": get_str(j, "ps", ""),
            "settings": {
                "vnext": [{
                    "address": get_str(j, "add", ""),
                    "port": port,
                    "users": [{"id": get_str(j, "id", ""), "security": scy}]
                }]
            },
            "streamSettings": stream,
        }),
        identity,
    })
}

fn get_str(object: &Map<String, Value>, key: &str, default: &str) -> String {
    object
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn num(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().map(|v| v as i64))
            .or_else(|| number.as_f64().map(|v| v as i64)),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

// --- shadowsocks ---

fn parse_shadowsocks(link: &str) -> Result<ParsedOutbound> {
    let mut rest = link.strip_prefix("ss://").context("not ss")?;
    let (before_fragment, fragment_raw) = match rest.split_once('#') {
        Some((left, right)) => (left, right),
        None => (rest, ""),
    };
    rest = before_fragment;
    let query = match rest.split_once('?') {
        Some((left, right)) => {
            rest = left;
            right
        }
        None => "",
    };
    let remark = decode_component(fragment_raw);
    let params: Vec<(String, String)> = form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();

    let core = rest.trim_end_matches('/');
    let (user_info, host_port) = if let Some((user, hp)) = core.split_once('@') {
        let decoded = decode_base64_text(user)
            .or_else(|| {
                percent_decode_str(user)
                    .decode_utf8()
                    .ok()
                    .map(|v| v.into_owned())
            })
            .unwrap_or_else(|| user.to_string());
        (decoded, hp.to_string())
    } else {
        let decoded = decode_base64_text(core).context("decode legacy ss payload")?;
        let (user, hp) = decoded.split_once('@').context("bad legacy ss payload")?;
        (user.to_string(), hp.to_string())
    };
    let (host, port) = split_host_port(&host_port)?;
    let (method, password) = match user_info.split_once(':') {
        Some((method, password)) => (method.to_string(), password.to_string()),
        None => ("2022-blake3-aes-128-gcm".to_string(), user_info),
    };

    let mut params = params;
    apply_obfs_local_plugin(&mut params);
    let network = first_non_empty(param(&params, "type"), "tcp");
    let security = first_non_empty(param(&params, "security"), "none");
    let mut stream = build_stream(network, security);
    apply_transport(&mut stream, &params);
    apply_security(&mut stream, &params);
    apply_final_mask(&mut stream, &params);

    Ok(ParsedOutbound {
        outbound: json!({
            "protocol": "shadowsocks",
            "tag": remark,
            "settings": {
                "servers": [{
                    "address": host,
                    "port": port,
                    "password": password,
                    "method": method
                }]
            },
            "streamSettings": stream,
        }),
        identity: format!("ss:{method}:{password}@{host}:{port}"),
    })
}

fn split_host_port(value: &str) -> Result<(String, u16)> {
    let value = value.trim_end_matches('/');
    let (host, port) = value.rsplit_once(':').context("bad host:port")?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let port: u16 = port.trim().parse().context("bad port")?;
    if host.is_empty() || port == 0 {
        bail!("bad host:port");
    }
    Ok((host.to_string(), port))
}

fn apply_obfs_local_plugin(params: &mut Vec<(String, String)>) {
    if param(params, "headerType") == "http" || param(params, "type") == "http" {
        return;
    }
    let plugin = params
        .iter()
        .find(|(key, _)| key == "plugin")
        .map(|(_, value)| value.clone())
        .unwrap_or_default();
    let mut parts = plugin.split(';');
    if parts.next() != Some("obfs-local") {
        return;
    }
    let mut obfs = "";
    let mut host = "";
    for part in parts {
        if let Some((key, value)) = part.split_once('=') {
            match key {
                "obfs" => obfs = value,
                "obfs-host" => host = value,
                _ => {}
            }
        }
    }
    if obfs != "http" {
        return;
    }
    params.retain(|(key, _)| key != "type" && key != "headerType" && key != "host");
    params.push(("type".into(), "tcp".into()));
    params.push(("headerType".into(), "http".into()));
    if !host.is_empty() {
        params.push(("host".into(), host.into()));
    }
}

// --- hysteria2 ---

fn parse_hysteria2(link: &str) -> Result<ParsedOutbound> {
    let url = Url::parse(link).context("parse hysteria2 link")?;
    if url.scheme() != "hysteria2" && url.scheme() != "hy2" {
        bail!("not hysteria2");
    }
    let auth = decode_component(url.username());
    let host = host_of(&url)?;
    let port = default_port(url.port(), 443);
    let params = query_pairs(&url);

    let mut stream = json!({
        "network": "hysteria",
        "security": "tls",
        "hysteriaSettings": {"version": 2, "auth": auth, "udpIdleTimeout": 60},
        "tlsSettings": {
            "serverName": param(&params, "sni"),
            "alpn": split_comma_or(param(&params, "alpn"), &["h3"]),
            "fingerprint": param(&params, "fp"),
            "echConfigList": param(&params, "ech"),
            "verifyPeerCertByName": param(&params, "vcn"),
            "pinnedPeerCertSha256": param(&params, "pinSHA256"),
        },
    });
    apply_final_mask(&mut stream, &params);
    apply_hysteria2_obfs(&mut stream, &params);
    apply_hysteria2_hop(&mut stream, &params);

    let identity = format!(
        "hysteria2:{}@{}:{}?{}",
        auth,
        host,
        port,
        canonical_query(&params, "tls")
    );
    Ok(ParsedOutbound {
        outbound: json!({
            "protocol": "hysteria",
            "tag": fragment(&url),
            "settings": {"address": host, "port": port, "version": 2},
            "streamSettings": stream,
        }),
        identity,
    })
}

fn apply_hysteria2_obfs(stream: &mut Value, params: &[(String, String)]) {
    let obfs = param(params, "obfs");
    let is_gecko = obfs.eq_ignore_ascii_case("gecko");
    if !is_gecko && !obfs.eq_ignore_ascii_case("salamander") {
        return;
    }
    let password = first_non_empty(
        first_non_empty(
            param(params, "obfs-password"),
            param(params, "obfs_password"),
        ),
        param(params, "obfsPassword"),
    );
    if password.is_empty() {
        return;
    }
    let mut settings = Map::new();
    settings.insert("password".into(), json!(password));
    if is_gecko {
        if let (Some(min), Some(max)) = (
            parse_in_range(param(params, "minPacketSize"), 1, 2048),
            parse_in_range(param(params, "maxPacketSize"), 1, 2048),
        ) {
            if min <= max {
                settings.insert("packetSize".into(), json!(format!("{min}-{max}")));
            }
        }
    }
    let already = stream
        .get("finalmask")
        .and_then(|mask| mask.get("udp"))
        .and_then(Value::as_array)
        .is_some_and(|masks| {
            masks
                .iter()
                .any(|mask| mask.get("type").and_then(Value::as_str) == Some("salamander"))
        });
    if already {
        return;
    }
    append_udp_mask(stream, json!({"type": "salamander", "settings": settings}));
}

fn apply_hysteria2_hop(stream: &mut Value, params: &[(String, String)]) {
    let ports = param(params, "mport");
    if ports.is_empty() {
        return;
    }
    let already = stream
        .get("finalmask")
        .and_then(|mask| mask.get("udp"))
        .and_then(Value::as_array)
        .is_some_and(|masks| {
            masks
                .iter()
                .any(|mask| mask.get("type").and_then(Value::as_str) == Some("udphop"))
        });
    if already {
        return;
    }
    append_udp_mask(
        stream,
        json!({
            "type": "udphop",
            "settings": {"mode": "intervalremote", "interval": "5-10", "remotePorts": ports}
        }),
    );
}

// --- wireguard ---

fn parse_wireguard(link: &str) -> Result<ParsedOutbound> {
    let url = Url::parse(link).context("parse wireguard link")?;
    if url.scheme() != "wireguard" && url.scheme() != "wg" {
        bail!("not wireguard");
    }
    let secret = decode_component(url.username());
    let params = query_pairs(&url);
    let host = host_of(&url)?;
    let endpoint = match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.clone(),
    };

    let mut addresses = split_comma(first_non_empty(
        param(&params, "address"),
        param(&params, "ip"),
    ));
    if addresses.is_empty() {
        addresses = vec!["0.0.0.0/0".into(), "::/0".into()];
    }
    let mut allowed = split_comma(first_non_empty(
        param(&params, "allowedips"),
        param(&params, "allowed_ips"),
    ));
    if allowed.is_empty() {
        allowed = vec!["0.0.0.0/0".into(), "::/0".into()];
    }
    let public_key = first_non_empty(
        first_non_empty(param(&params, "publickey"), param(&params, "publicKey")),
        first_non_empty(
            param(&params, "public_key"),
            param(&params, "peerPublicKey"),
        ),
    );
    let mut peer = Map::new();
    peer.insert("publicKey".into(), json!(public_key));
    peer.insert("endpoint".into(), json!(endpoint));
    peer.insert("allowedIPs".into(), json!(allowed));
    let psk = first_non_empty(
        first_non_empty(
            param(&params, "presharedkey"),
            param(&params, "preshared_key"),
        ),
        first_non_empty(param(&params, "pre-shared-key"), param(&params, "psk")),
    );
    if !psk.is_empty() {
        peer.insert("preSharedKey".into(), json!(psk));
    }
    let keepalive = first_non_empty(
        param(&params, "keepalive"),
        first_non_empty(
            param(&params, "persistentkeepalive"),
            param(&params, "persistent_keepalive"),
        ),
    );
    if let Ok(seconds) = keepalive.parse::<i64>() {
        peer.insert("keepAlive".into(), json!(seconds));
    }

    let mut settings = Map::new();
    settings.insert("secretKey".into(), json!(secret));
    settings.insert("address".into(), json!(addresses));
    settings.insert("peers".into(), json!([peer]));
    if let Some(mtu) = parse_in_range(param(&params, "mtu"), 1, i64::MAX) {
        settings.insert("mtu".into(), json!(mtu));
    }
    let reserved: Vec<i64> = split_comma(param(&params, "reserved"))
        .iter()
        .filter_map(|part| part.parse().ok())
        .collect();
    if reserved.len() == 3 {
        settings.insert("reserved".into(), json!(reserved));
    }

    let identity = format!(
        "wireguard:{}@{}?{}",
        secret,
        endpoint,
        canonical_query(&params, "none")
    );
    Ok(ParsedOutbound {
        outbound: json!({
            "protocol": "wireguard",
            "tag": fragment(&url),
            "settings": settings,
        }),
        identity,
    })
}

// --- tag assignment ---

/// Assign tags with stability so balancers and routing rules keep matching:
/// identity map first, positional reuse second, prefix + remark slug last.
pub fn assign_stable_tags(
    outbounds: &mut [Value],
    identities: &[String],
    previous: &HashMap<String, String>,
    previous_by_index: &[String],
    prefix: &str,
) -> Vec<String> {
    let mut reserved: HashSet<String> = HashSet::new();
    for identity in identities {
        if let Some(tag) = previous.get(identity).filter(|tag| !tag.is_empty()) {
            reserved.insert(tag.clone());
        }
    }

    let mut used: HashSet<String> = HashSet::new();
    let mut assigned = Vec::with_capacity(outbounds.len());
    for (index, outbound) in outbounds.iter_mut().enumerate() {
        let identity = identities.get(index).map(String::as_str).unwrap_or("");
        let identity_tag = previous
            .get(identity)
            .filter(|tag| !tag.is_empty())
            .cloned()
            .unwrap_or_default();
        let mut candidate = identity_tag.clone();
        if candidate.is_empty() {
            if let Some(old) = previous_by_index.get(index).filter(|tag| !tag.is_empty()) {
                if !reserved.contains(old) {
                    candidate = old.clone();
                }
            }
        }
        if candidate.is_empty() {
            let remark = outbound
                .get("tag")
                .and_then(Value::as_str)
                .unwrap_or_default();
            candidate = suggest_tag(prefix, remark, index);
        }
        let mut final_tag = candidate.clone();
        let mut suffix = 1;
        while used.contains(&final_tag)
            || (reserved.contains(&final_tag) && final_tag != identity_tag)
        {
            final_tag = format!("{candidate}-{suffix}");
            suffix += 1;
        }
        used.insert(final_tag.clone());
        assigned.push(final_tag.clone());
        if let Some(object) = outbound.as_object_mut() {
            object.insert("tag".into(), json!(final_tag));
        }
    }
    assigned
}

pub fn suggest_tag(prefix: &str, remark: &str, index: usize) -> String {
    let mut base = slug_remark(remark);
    if base.is_empty() {
        // 3x-ui numbers an unnamed link by its zero-based position.
        base = index.to_string();
    }
    let prefix = prefix.trim_end_matches('-');
    if prefix.is_empty() {
        base
    } else {
        format!("{prefix}-{base}")
    }
}

pub fn slug_remark(remark: &str) -> String {
    let mut slug = String::new();
    let mut pending_dash = false;
    for ch in remark.trim().to_lowercase().chars() {
        if ch.is_alphanumeric() {
            if pending_dash && !slug.is_empty() {
                slug.push('-');
            }
            pending_dash = false;
            slug.push(ch);
        } else {
            pending_dash = true;
        }
    }
    slug
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(lines: &[&str]) -> String {
        STANDARD.encode(lines.join("\n"))
    }

    #[test]
    fn parses_base64_and_plain_subscription_bodies() {
        let vless = "vless://11111111-2222-3333-4444-555555555555@example.com:443?type=ws&security=tls&sni=example.com&path=%2Fws#Tokyo%20One";
        let trojan = "trojan://secret@trojan.example.net:8443?type=tcp&security=tls#Trojan%20Node";
        let encoded = body(&[vless, trojan, "not-a-link"]);
        let parsed = parse_subscription_body(encoded.as_bytes());
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].outbound["protocol"], "vless");
        assert_eq!(parsed[0].outbound["settings"]["address"], "example.com");
        assert_eq!(parsed[0].outbound["streamSettings"]["network"], "ws");
        assert_eq!(
            parsed[0].outbound["streamSettings"]["wsSettings"]["path"],
            "/ws"
        );
        assert_eq!(parsed[1].outbound["protocol"], "trojan");
        assert_eq!(parsed[1].outbound["settings"]["servers"][0]["port"], 8443);

        let plain = format!("{vless}\n\n#{trojan}\n{trojan}");
        let parsed = parse_subscription_body(plain.as_bytes());
        assert_eq!(parsed.len(), 2, "comments are skipped");
    }

    #[test]
    fn vmess_identity_ignores_remark_and_tags_use_ps() {
        let payload = json!({
            "v": "2", "ps": "HK 01", "add": "hk.example.com", "port": "443",
            "id": "11111111-2222-3333-4444-555555555555", "aid": "0", "scy": "auto",
            "net": "ws", "type": "none", "host": "cdn.example.com", "path": "/ray",
            "tls": "tls", "sni": "cdn.example.com"
        });
        let link = format!(
            "vmess://{}",
            STANDARD.encode(serde_json::to_vec(&payload).unwrap())
        );
        let parsed = parse_link(&link).expect("vmess parses");
        assert_eq!(parsed.outbound["tag"], "HK 01");
        assert_eq!(
            parsed.outbound["settings"]["vnext"][0]["address"],
            "hk.example.com"
        );
        assert_eq!(
            parsed.outbound["streamSettings"]["wsSettings"]["path"],
            "/ray"
        );
        assert_eq!(
            parsed.outbound["streamSettings"]["tlsSettings"]["serverName"],
            "cdn.example.com"
        );
        assert!(parsed.identity.starts_with("vmess:{"));

        let mut renamed = payload.clone();
        renamed["ps"] = json!("HK 02");
        let link2 = format!(
            "vmess://{}",
            STANDARD.encode(serde_json::to_vec(&renamed).unwrap())
        );
        assert_eq!(parse_link(&link2).unwrap().identity, parsed.identity);
    }

    #[test]
    fn shadowsocks_modern_and_legacy_forms_match() {
        let modern = format!(
            "ss://{}@ss.example.com:8388?type=tcp&security=none#SS%20Node",
            STANDARD.encode("aes-256-gcm:pass word".as_bytes())
        );
        let legacy = format!(
            "ss://{}#SS%20Node",
            URL_SAFE_NO_PAD.encode("aes-256-gcm:pass word@ss.example.com:8388".as_bytes())
        );
        let a = parse_link(&modern).expect("modern");
        let b = parse_link(&legacy).expect("legacy");
        assert_eq!(a.outbound["settings"], b.outbound["settings"]);
        assert_eq!(
            a.outbound["settings"]["servers"][0]["method"],
            "aes-256-gcm"
        );
        assert_eq!(
            a.outbound["settings"]["servers"][0]["password"],
            "pass word"
        );
        assert_eq!(a.identity, b.identity);
    }

    #[test]
    fn reality_and_finalmask_fields_are_ported() {
        let link = "vless://id@host.example.com:443?type=tcp&security=reality&sni=www.apple.com&fp=chrome&pbk=PUB&sid=ab&spx=%2F&flow=xtls-rprx-vision&fm=%7B%22udp%22%3A%5B%5D%7D#R";
        let parsed = parse_link(link).expect("parses");
        let stream = &parsed.outbound["streamSettings"];
        assert_eq!(stream["realitySettings"]["publicKey"], "PUB");
        assert_eq!(stream["realitySettings"]["shortId"], "ab");
        assert_eq!(stream["realitySettings"]["serverName"], "www.apple.com");
        assert!(stream["finalmask"].is_object());
        assert_eq!(parsed.outbound["settings"]["flow"], "xtls-rprx-vision");
    }

    #[test]
    fn stable_tags_survive_remark_changes_and_allocations_stay_unique() {
        let previous = HashMap::from([
            ("id-a".to_string(), "tokyo".to_string()),
            ("id-b".to_string(), "osaka".to_string()),
        ]);
        let mut outbounds = vec![json!({"tag": "renamed A"}), json!({"tag": "renamed B"})];
        let identities = vec!["id-a".to_string(), "id-b".to_string()];
        let tags = assign_stable_tags(&mut outbounds, &identities, &previous, &[], "sub1-");
        assert_eq!(tags, ["tokyo", "osaka"]);

        // A new identity takes the prefix + slug; a colliding slug gets -1.
        let mut fresh = vec![json!({"tag": "Tokyo"}), json!({"tag": "Tokyo"})];
        let tags = assign_stable_tags(
            &mut fresh,
            &["new-a".to_string(), "new-b".to_string()],
            &HashMap::new(),
            &[],
            "sub1-",
        );
        assert_eq!(tags, ["sub1-tokyo", "sub1-tokyo-1"]);
        assert_eq!(fresh[0]["tag"], "sub1-tokyo");
    }

    #[test]
    fn nameless_links_use_their_position_for_the_tag() {
        assert_eq!(suggest_tag("sub1-", "", 0), "sub1-0");
        assert_eq!(suggest_tag("", "  ", 3), "3");
    }

    #[test]
    fn duplicate_identities_get_distinct_identities() {
        let link = "vless://id@host:443?type=tcp#same";
        let body = format!("{link}\n{link}");
        let parsed = parse_subscription_body(body.as_bytes());
        assert_eq!(parsed.len(), 2);
        assert_ne!(parsed[0].identity, parsed[1].identity);
        assert_eq!(parsed[1].identity, format!("{}#1", parsed[0].identity));
    }

    #[test]
    fn hysteria2_and_wireguard_links_parse() {
        let hy = parse_link("hy2://pass@hy.example.com:443?sni=hy.example.com&obfs=salamander&obfs-password=obfs#HY")
            .expect("hysteria2");
        assert_eq!(hy.outbound["protocol"], "hysteria");
        assert_eq!(hy.outbound["streamSettings"]["network"], "hysteria");
        assert_eq!(
            hy.outbound["streamSettings"]["finalmask"]["udp"][0]["type"],
            "salamander"
        );
        assert_eq!(
            hy.outbound["streamSettings"]["hysteriaSettings"]["auth"],
            "pass"
        );

        let wg = parse_link(
            "wireguard://secret@wg.example.com:51820?publickey=PUB&address=10.0.0.2/32#WG",
        )
        .expect("wireguard");
        assert_eq!(wg.outbound["protocol"], "wireguard");
        assert_eq!(
            wg.outbound["settings"]["peers"][0]["endpoint"],
            "wg.example.com:51820"
        );
        assert_eq!(wg.outbound["settings"]["address"], json!(["10.0.0.2/32"]));
    }
}
