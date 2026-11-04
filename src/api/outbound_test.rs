use std::{
    collections::HashSet,
    net::TcpListener,
    path::Path,
    process::Stdio,
    time::{Duration, Instant},
};

use axum::{
    extract::{Request, State},
    Json,
};
use serde_json::{json, Map, Value};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    process::Command,
    task::JoinSet,
    time::timeout,
};

use crate::{api::compat, state::AppState};

// Plain HTTP 204 endpoint, so the probe measures a real round trip through the
// tunnel without needing a TLS client.
const TEST_HOST: &str = "www.gstatic.com";
const TEST_PORT: u16 = 80;
const DIAL_TIMEOUT: Duration = Duration::from_secs(5);
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_OUTBOUNDS: usize = 50;

pub async fn test_outbounds(State(state): State<AppState>, req: Request) -> Json<Value> {
    let params = match compat::read_params(req).await {
        Ok(params) => params,
        Err(_) => return compat::fail("请求内容无效"),
    };
    let outbounds: Vec<Value> = params
        .get("outbounds")
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_default();
    if outbounds.is_empty() {
        return compat::ok(json!([]));
    }
    let outbounds = &outbounds[..outbounds.len().min(MAX_OUTBOUNDS)];
    let all_outbounds: Vec<Value> = params
        .get("allOutbounds")
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_default();
    let mode = match params.get("mode").map(String::as_str) {
        Some("tcp") => "tcp",
        Some("real") => "real",
        _ => "http",
    };
    let results = if mode == "tcp" {
        tcp_probe(outbounds).await
    } else {
        tunnel_probe(&state, outbounds, &all_outbounds, mode).await
    };
    compat::ok(Value::Array(results))
}

// Shared by the periodic outbound checker: a real TCP+SOCKS+HTTP request
// through a temporary Xray instance, the same probe the UI's "real" mode uses.
pub(crate) async fn probe_real(
    state: &AppState,
    outbounds: &[Value],
    all_outbounds: &[Value],
) -> Vec<Value> {
    tunnel_probe(state, outbounds, all_outbounds, "real").await
}

fn result_object(
    tag: &str,
    mode: &str,
    success: bool,
    delay: Option<u64>,
    error: Option<String>,
    endpoints: Vec<Value>,
) -> Value {
    let mut map = Map::new();
    map.insert("tag".into(), json!(tag));
    map.insert("success".into(), json!(success));
    map.insert("mode".into(), json!(mode));
    if let Some(delay) = delay {
        map.insert("delay".into(), json!(delay));
    }
    if let Some(error) = error {
        map.insert("error".into(), json!(error));
    }
    if !endpoints.is_empty() {
        map.insert("endpoints".into(), Value::Array(endpoints));
    }
    Value::Object(map)
}

async fn collect_results(
    mut jobs: JoinSet<(usize, Value)>,
    count: usize,
    mode: &str,
    fallback: &str,
) -> Vec<Value> {
    let mut slots: Vec<Option<Value>> = (0..count).map(|_| None).collect();
    while let Some(joined) = jobs.join_next().await {
        if let Ok((index, value)) = joined {
            if let Some(slot) = slots.get_mut(index) {
                *slot = Some(value);
            }
        }
    }
    slots
        .into_iter()
        .map(|slot| {
            slot.unwrap_or_else(|| {
                result_object(
                    "",
                    mode,
                    false,
                    None,
                    Some(fallback.to_string()),
                    Vec::new(),
                )
            })
        })
        .collect()
}

fn outbound_tag(outbound: &Value) -> String {
    outbound
        .get("tag")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

async fn tcp_probe(outbounds: &[Value]) -> Vec<Value> {
    let mut jobs = JoinSet::new();
    for (index, outbound) in outbounds.iter().enumerate() {
        let outbound = outbound.clone();
        jobs.spawn(async move { (index, tcp_probe_one(&outbound).await) });
    }
    collect_results(jobs, outbounds.len(), "tcp", "检测失败").await
}

async fn tcp_probe_one(outbound: &Value) -> Value {
    let tag = outbound_tag(outbound);
    let addresses = outbound_addresses(outbound);
    let mut endpoints = Vec::new();
    let mut best: Option<u64> = None;
    for address in &addresses {
        let started = Instant::now();
        let outcome = timeout(DIAL_TIMEOUT, TcpStream::connect(address)).await;
        let delay = started.elapsed().as_millis() as u64;
        match outcome {
            Ok(Ok(_stream)) => {
                best = Some(best.map_or(delay, |current| current.min(delay)));
                endpoints.push(json!({
                    "address": address,
                    "delay": delay,
                    "success": true,
                }));
            }
            Ok(Err(err)) => endpoints.push(json!({
                "address": address,
                "success": false,
                "error": err.to_string(),
            })),
            Err(_) => endpoints.push(json!({
                "address": address,
                "success": false,
                "error": "超时",
            })),
        }
    }
    let success = endpoints
        .iter()
        .any(|entry| entry["success"] == json!(true));
    let error = if success {
        None
    } else if addresses.is_empty() {
        Some("出站没有可连接的地址".to_string())
    } else {
        endpoints
            .iter()
            .find_map(|entry| {
                entry
                    .get("error")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .or_else(|| Some("连接失败".to_string()))
    };
    result_object(&tag, "tcp", success, best, error, endpoints)
}

async fn tunnel_probe(
    state: &AppState,
    outbounds: &[Value],
    all_outbounds: &[Value],
    mode: &str,
) -> Vec<Value> {
    let ports: Vec<u16> = (0..outbounds.len()).map(|_| unused_local_port()).collect();
    if ports.contains(&0) {
        return outbounds
            .iter()
            .map(|outbound| {
                result_object(
                    &outbound_tag(outbound),
                    mode,
                    false,
                    None,
                    Some("无法分配本地测试端口".into()),
                    Vec::new(),
                )
            })
            .collect();
    }
    let config = build_config(outbounds, all_outbounds, &ports);
    let path = std::env::temp_dir().join(format!("xray-panel-test-{}.json", uuid::Uuid::new_v4()));
    if let Err(err) = write_config(&path, &config) {
        return failure_all(outbounds, mode, format!("写入测试配置失败：{err}"));
    }
    let mut child = match Command::new(&state.cfg.xray_bin)
        .arg("-c")
        .arg(&path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            let _ = std::fs::remove_file(&path);
            return failure_all(outbounds, mode, format!("启动测试 Xray 失败：{err}"));
        }
    };
    let results = if !wait_ready(&ports).await {
        let detail = match child.try_wait() {
            Ok(Some(status)) => format!("测试 Xray 启动后退出（{status}）"),
            Ok(None) => "测试 Xray 已运行但 SOCKS 端口未打开".to_string(),
            Err(err) => format!("读取测试 Xray 状态失败：{err}"),
        };
        failure_all(outbounds, mode, detail)
    } else {
        let mut jobs = JoinSet::new();
        for (index, port) in ports.iter().copied().enumerate() {
            let tag = outbound_tag(&outbounds[index]);
            let mode = mode.to_string();
            jobs.spawn(async move {
                let started = Instant::now();
                let outcome = timeout(PROBE_TIMEOUT, async {
                    let mut stream = socks_connect(port).await?;
                    let connect_ms = started.elapsed().as_millis() as u64;
                    let request_started = Instant::now();
                    let status = http_probe(&mut stream).await?;
                    let ttfb_ms = request_started.elapsed().as_millis() as u64;
                    Ok::<_, String>((status, connect_ms, ttfb_ms))
                })
                .await;
                let value = match outcome {
                    Ok(Ok((status, connect_ms, ttfb_ms))) => {
                        // "real" includes tunnel setup; "http" is the warm
                        // request round trip after the tunnel is up.
                        let delay = if mode == "real" {
                            started.elapsed().as_millis() as u64
                        } else {
                            ttfb_ms
                        };
                        let mut value =
                            result_object(&tag, &mode, true, Some(delay), None, Vec::new());
                        if let Some(object) = value.as_object_mut() {
                            object.insert("httpStatus".into(), json!(status));
                            object.insert("connectMs".into(), json!(connect_ms));
                            object.insert("ttfbMs".into(), json!(ttfb_ms));
                        }
                        value
                    }
                    Ok(Err(err)) => result_object(&tag, &mode, false, None, Some(err), Vec::new()),
                    Err(_) => {
                        result_object(&tag, &mode, false, None, Some("超时".into()), Vec::new())
                    }
                };
                (index, value)
            });
        }
        collect_results(jobs, outbounds.len(), mode, "检测失败").await
    };
    let _ = child.kill().await;
    let _ = tokio::fs::remove_file(&path).await;
    results
}

fn failure_all(outbounds: &[Value], mode: &str, reason: String) -> Vec<Value> {
    outbounds
        .iter()
        .map(|outbound| {
            result_object(
                &outbound_tag(outbound),
                mode,
                false,
                None,
                Some(reason.clone()),
                Vec::new(),
            )
        })
        .collect()
}

fn build_config(outbounds: &[Value], all_outbounds: &[Value], ports: &[u16]) -> Value {
    let mut inbounds = Vec::new();
    let mut rules = Vec::new();
    let mut config_outbounds: Vec<Value> = Vec::new();
    let mut seen_tags = HashSet::new();
    for outbound in all_outbounds {
        match outbound.get("tag").and_then(Value::as_str) {
            Some(tag) if !seen_tags.insert(tag.to_string()) => {}
            _ => config_outbounds.push(outbound.clone()),
        }
    }
    for (index, (outbound, port)) in outbounds.iter().zip(ports).enumerate() {
        let inbound_tag = format!("__xpanel_test_in_{index}");
        let outbound_tag = format!("__xpanel_test_out_{index}");
        inbounds.push(json!({
            "tag": inbound_tag,
            "listen": "127.0.0.1",
            "port": port,
            "protocol": "socks",
            "settings": { "auth": "noauth", "udp": false },
            "sniffing": { "enabled": false },
        }));
        let mut object = outbound.as_object().cloned().unwrap_or_default();
        object.insert("tag".into(), json!(outbound_tag));
        config_outbounds.push(Value::Object(object));
        rules.push(json!({
            "type": "field",
            "inboundTag": [inbound_tag],
            "outboundTag": outbound_tag,
        }));
    }
    if config_outbounds.is_empty() {
        config_outbounds.push(json!({ "tag": "__xpanel_direct", "protocol": "freedom" }));
    }
    json!({
        "log": { "loglevel": "warning" },
        "inbounds": inbounds,
        "outbounds": config_outbounds,
        "routing": { "domainStrategy": "AsIs", "rules": rules },
    })
}

fn unused_local_port() -> u16 {
    for _ in 0..5 {
        if let Ok(listener) = TcpListener::bind(("127.0.0.1", 0)) {
            if let Ok(address) = listener.local_addr() {
                return address.port();
            }
        }
    }
    0
}

fn write_config(path: &Path, config: &Value) -> std::io::Result<()> {
    let text = serde_json::to_vec(config).map_err(std::io::Error::other)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(path)?;
        std::io::Write::write_all(&mut file, &text)?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, text)
    }
}

async fn wait_ready(ports: &[u16]) -> bool {
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    loop {
        let mut all_ready = true;
        for port in ports {
            let probe = timeout(
                Duration::from_millis(300),
                TcpStream::connect(("127.0.0.1", *port)),
            )
            .await;
            if !matches!(probe, Ok(Ok(_))) {
                all_ready = false;
                break;
            }
        }
        if all_ready {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
}

async fn socks_connect(proxy_port: u16) -> Result<TcpStream, String> {
    let mut stream = TcpStream::connect(("127.0.0.1", proxy_port))
        .await
        .map_err(|err| err.to_string())?;
    stream
        .write_all(&[0x05, 0x01, 0x00])
        .await
        .map_err(|err| err.to_string())?;
    let mut chosen = [0u8; 2];
    stream
        .read_exact(&mut chosen)
        .await
        .map_err(|err| err.to_string())?;
    if chosen != [0x05, 0x00] {
        return Err(format!("SOCKS 握手被拒绝（代码 {}）", chosen[1]));
    }
    let mut request = vec![0x05, 0x01, 0x00, 0x03, TEST_HOST.len() as u8];
    request.extend_from_slice(TEST_HOST.as_bytes());
    request.extend_from_slice(&TEST_PORT.to_be_bytes());
    stream
        .write_all(&request)
        .await
        .map_err(|err| err.to_string())?;
    let mut head = [0u8; 4];
    stream
        .read_exact(&mut head)
        .await
        .map_err(|err| err.to_string())?;
    if head[0] != 0x05 {
        return Err("SOCKS 响应异常".into());
    }
    if head[1] != 0x00 {
        return Err(socks_error(head[1]));
    }
    let skip = match head[3] {
        0x01 => 4,
        0x03 => {
            let mut len = [0u8; 1];
            stream
                .read_exact(&mut len)
                .await
                .map_err(|err| err.to_string())?;
            len[0] as usize
        }
        0x04 => 16,
        _ => return Err("SOCKS 地址类型异常".into()),
    };
    let mut rest = vec![0u8; skip + 2];
    stream
        .read_exact(&mut rest)
        .await
        .map_err(|err| err.to_string())?;
    Ok(stream)
}

// SOCKS success alone can arrive at TCP-handshake speed, so the probe only
// counts a tunnel as working once the target answers a real HTTP request.
async fn http_probe(stream: &mut TcpStream) -> Result<u16, String> {
    let request = format!(
        "GET /generate_204 HTTP/1.1\r\nHost: {TEST_HOST}\r\nUser-Agent: lumen-panel\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|err| err.to_string())?;
    let mut response = Vec::new();
    loop {
        let mut chunk = [0u8; 512];
        let read = stream
            .read(&mut chunk)
            .await
            .map_err(|err| err.to_string())?;
        if read == 0 {
            break;
        }
        response.extend_from_slice(&chunk[..read]);
        if response.windows(2).any(|window| window == b"\r\n") {
            break;
        }
        if response.len() > 8192 {
            break;
        }
    }
    if response.is_empty() {
        return Err("HTTP 响应为空".into());
    }
    let text = String::from_utf8_lossy(&response);
    let first = text.lines().next().unwrap_or_default();
    first
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| format!("HTTP 状态行异常：{first}"))
}

fn socks_error(code: u8) -> String {
    let reason = match code {
        1 => "一般故障",
        2 => "连接不被允许",
        3 => "网络不可达",
        4 => "主机不可达",
        5 => "连接被拒绝",
        6 => "TTL 已过期",
        7 => "命令不受支持",
        8 => "地址类型不受支持",
        _ => "未知错误",
    };
    format!("SOCKS 连接失败：{reason}（代码 {code}）")
}

fn outbound_addresses(outbound: &Value) -> Vec<String> {
    let settings = outbound.get("settings").unwrap_or(&Value::Null);
    let protocol = outbound
        .get("protocol")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match protocol {
        "vmess" => array_addresses(settings.get("vnext")),
        "vless" | "hysteria" | "masque" => {
            let mut addresses = array_addresses(settings.get("vnext"));
            if addresses.is_empty() {
                let address = settings.get("address").and_then(Value::as_str);
                let port = port_value(settings.get("port"));
                if let (Some(address), Some(port)) = (address, port) {
                    addresses.push(format_endpoint(address, port));
                }
            }
            addresses
        }
        "http" | "socks" | "shadowsocks" | "trojan" => array_addresses(settings.get("servers")),
        "dns" => {
            let address = settings
                .get("rewriteAddress")
                .or_else(|| settings.get("address"))
                .and_then(Value::as_str);
            let port = port_value(settings.get("rewritePort"))
                .or_else(|| port_value(settings.get("port")));
            match (address, port) {
                (Some(address), Some(port)) => vec![format_endpoint(address, port)],
                (Some(address), None) => vec![address.to_string()],
                _ => Vec::new(),
            }
        }
        "wireguard" | "amneziawg" => settings
            .get("peers")
            .and_then(Value::as_array)
            .map(|peers| {
                peers
                    .iter()
                    .filter_map(|peer| {
                        peer.get("endpoint")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                    .collect()
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn array_addresses(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let address = item.get("address").and_then(Value::as_str)?;
                    let port = port_value(item.get("port"))?;
                    Some(format_endpoint(address, port))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn port_value(value: Option<&Value>) -> Option<u16> {
    match value? {
        Value::Number(number) => number.as_u64().and_then(|port| u16::try_from(port).ok()),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

fn format_endpoint(address: &str, port: u16) -> String {
    if address.contains(':') && !address.starts_with('[') {
        format!("[{address}]:{port}")
    } else {
        format!("{address}:{port}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_endpoints_per_protocol() {
        let vless = json!({
            "tag": "p",
            "protocol": "vless",
            "settings": { "vnext": [{ "address": "example.com", "port": 443 }] }
        });
        assert_eq!(outbound_addresses(&vless), ["example.com:443"]);

        let trojan = json!({
            "protocol": "trojan",
            "settings": { "servers": [{ "address": "1.2.3.4", "port": "8443" }] }
        });
        assert_eq!(outbound_addresses(&trojan), ["1.2.3.4:8443"]);

        let wg = json!({
            "protocol": "wireguard",
            "settings": { "peers": [{ "endpoint": "wg.example.com:51820" }] }
        });
        assert_eq!(outbound_addresses(&wg), ["wg.example.com:51820"]);

        let freedom = json!({ "protocol": "freedom", "settings": {} });
        assert!(outbound_addresses(&freedom).is_empty());
    }

    #[test]
    fn build_config_routes_each_test_inbound_to_its_outbound() {
        let all = vec![
            json!({ "tag": "proxy", "protocol": "vless", "settings": {} }),
            json!({ "tag": "direct", "protocol": "freedom" }),
        ];
        let tested = vec![json!({ "tag": "proxy", "protocol": "vless", "settings": {} })];
        let config = build_config(&tested, &all, &[23456]);
        assert_eq!(config["inbounds"][0]["port"], json!(23456));
        assert_eq!(
            config["routing"]["rules"][0]["outboundTag"],
            json!("__xpanel_test_out_0")
        );
        assert_eq!(config["inbounds"][0]["tag"], json!("__xpanel_test_in_0"));
        let tags: Vec<&str> = config["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|outbound| outbound["tag"].as_str())
            .collect();
        assert!(tags.contains(&"__xpanel_test_out_0"));
        assert!(tags.contains(&"direct"));
        assert_eq!(tags.iter().filter(|tag| **tag == "proxy").count(), 1);
    }

    #[tokio::test]
    async fn socks_connect_completes_handshake_against_stub_proxy() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut greeting = [0u8; 3];
            socket.read_exact(&mut greeting).await.unwrap();
            socket.write_all(&[0x05, 0x00]).await.unwrap();
            let mut head = [0u8; 5];
            socket.read_exact(&mut head).await.unwrap();
            let mut host = vec![0u8; head[4] as usize];
            socket.read_exact(&mut host).await.unwrap();
            let mut target_port = [0u8; 2];
            socket.read_exact(&mut target_port).await.unwrap();
            socket
                .write_all(&[0x05, 0x00, 0x00, 0x01, 127, 0, 0, 1, 0, 80])
                .await
                .unwrap();
        });
        assert!(socks_connect(port).await.is_ok());
    }

    #[tokio::test]
    async fn http_probe_waits_for_and_parses_the_status_line() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 512];
            let _ = socket.read(&mut request).await;
            socket
                .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n")
                .await
                .unwrap();
        });
        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        assert_eq!(http_probe(&mut client).await.unwrap(), 204);
    }

    #[test]
    fn result_omits_absent_delay_and_error() {
        let value = result_object("p", "tcp", false, None, Some("boom".into()), Vec::new());
        assert!(value.get("delay").is_none());
        assert_eq!(value["error"], json!("boom"));
    }
}
