use std::{net::IpAddr, time::Duration};

use axum::{extract::Request, Json};
use serde_json::{json, Value};

use super::compat;

const LOOKUP_TIMEOUT: Duration = Duration::from_secs(8);

// Location lookups are only made when the admin clicks the button, so a slow
// third-party call never touches the dashboard polling path.
pub async fn ip_location(req: Request) -> Json<Value> {
    let params = match compat::read_params(req).await {
        Ok(params) => params,
        Err(_) => return compat::fail("请求内容无效"),
    };
    let raw = params
        .get("ip")
        .map(String::as_str)
        .map(str::trim)
        .unwrap_or_default();
    let ip: IpAddr = match raw.parse() {
        Ok(ip) => ip,
        Err(_) => return compat::fail("IP 地址格式无效"),
    };
    match lookup(ip).await {
        Ok(value) => compat::ok(value),
        Err(err) => compat::fail(format!("查询 IP 位置失败：{err}")),
    }
}

async fn lookup(ip: IpAddr) -> anyhow::Result<Value> {
    tokio::task::spawn_blocking(move || {
        let ip_text = ip.to_string();
        let primary = format!("https://ipwho.is/{ip_text}?lang=zh-CN");
        match fetch_json(&primary) {
            Ok(value) => parse_ipwho(&ip_text, &value),
            Err(primary_err) => {
                let fallback = format!("https://ipapi.co/{ip_text}/json/");
                match fetch_json(&fallback) {
                    Ok(value) => parse_ipapi(&ip_text, &value),
                    Err(fallback_err) => Err(anyhow::anyhow!(
                        "位置服务不可用（{primary_err}；{fallback_err}）"
                    )),
                }
            }
        }
    })
    .await
    .map_err(|err| anyhow::anyhow!("查询任务失败：{err}"))?
}

fn fetch_json(url: &str) -> anyhow::Result<Value> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(LOOKUP_TIMEOUT))
        .max_redirects(3)
        .user_agent("lumen-panel")
        .http_status_as_error(false)
        .build()
        .into();
    let mut response = agent
        .get(url)
        .call()
        .map_err(|err| anyhow::anyhow!("网络请求失败：{err}"))?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(anyhow::anyhow!("位置服务返回 HTTP {status}"));
    }
    let body = response
        .body_mut()
        .with_config()
        .limit(256 * 1024)
        .read_to_string()
        .map_err(|err| anyhow::anyhow!("读取位置服务响应失败：{err}"))?;
    serde_json::from_str(&body).map_err(|err| anyhow::anyhow!("位置服务返回格式错误：{err}"))
}

fn text_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty() && !text.eq_ignore_ascii_case("null"))
        .unwrap_or_default()
        .to_string()
}

fn number_field(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(|entry| {
        entry
            .as_f64()
            .or_else(|| entry.as_str().and_then(|text| text.parse().ok()))
    })
}

struct LocationFields {
    country: String,
    region: String,
    city: String,
    isp: String,
    timezone: String,
    latitude: Option<f64>,
    longitude: Option<f64>,
}

fn location_object(ip: &str, provider: &str, fields: LocationFields) -> Value {
    json!({
        "ip": ip,
        "provider": provider,
        "country": fields.country,
        "region": fields.region,
        "city": fields.city,
        "isp": fields.isp,
        "timezone": fields.timezone,
        "latitude": fields.latitude,
        "longitude": fields.longitude,
    })
}

fn parse_ipwho(ip: &str, value: &Value) -> anyhow::Result<Value> {
    if value.get("success").and_then(Value::as_bool) == Some(false) {
        let message = text_field(value, "message");
        anyhow::bail!(if message.is_empty() {
            "位置服务拒绝查询".to_string()
        } else {
            message
        });
    }
    let isp = value
        .get("connection")
        .map(|connection| text_field(connection, "isp"))
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| {
            value
                .get("connection")
                .map(|connection| text_field(connection, "org"))
                .unwrap_or_default()
        });
    let timezone = value
        .get("timezone")
        .map(|entry| text_field(entry, "id"))
        .unwrap_or_default();
    Ok(location_object(
        ip,
        "ipwho.is",
        LocationFields {
            country: text_field(value, "country"),
            region: text_field(value, "region"),
            city: text_field(value, "city"),
            isp,
            timezone,
            latitude: number_field(value, "latitude"),
            longitude: number_field(value, "longitude"),
        },
    ))
}

fn parse_ipapi(ip: &str, value: &Value) -> anyhow::Result<Value> {
    if value.get("error").and_then(Value::as_bool) == Some(true) {
        let reason = text_field(value, "reason");
        anyhow::bail!(if reason.is_empty() {
            "位置服务拒绝查询".to_string()
        } else {
            reason
        });
    }
    Ok(location_object(
        ip,
        "ipapi.co",
        LocationFields {
            country: text_field(value, "country_name"),
            region: text_field(value, "region"),
            city: text_field(value, "city"),
            isp: text_field(value, "org"),
            timezone: text_field(value, "timezone"),
            latitude: number_field(value, "latitude"),
            longitude: number_field(value, "longitude"),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipwho_response_is_normalized() {
        let value = json!({
            "ip": "8.8.8.8",
            "success": true,
            "country": "美国",
            "region": "加利福尼亚州",
            "city": "山景城",
            "latitude": 37.4,
            "longitude": -122.07,
            "connection": { "isp": "Google LLC", "org": "Google LLC" },
            "timezone": { "id": "America/Los_Angeles" }
        });
        let parsed = parse_ipwho("8.8.8.8", &value).unwrap();
        assert_eq!(parsed["country"], json!("美国"));
        assert_eq!(parsed["city"], json!("山景城"));
        assert_eq!(parsed["isp"], json!("Google LLC"));
        assert_eq!(parsed["timezone"], json!("America/Los_Angeles"));
    }

    #[test]
    fn ipwho_failure_carries_the_provider_message() {
        let value = json!({ "success": false, "message": "invalid IP" });
        let err = parse_ipwho("1.2.3.4", &value).unwrap_err();
        assert!(err.to_string().contains("invalid IP"));
    }

    #[test]
    fn ipapi_response_is_normalized() {
        let value = json!({
            "ip": "1.1.1.1",
            "country_name": "澳大利亚",
            "region": "昆士兰州",
            "city": "布里斯班",
            "org": "Cloudflare",
            "timezone": "Australia/Brisbane",
            "latitude": -27.47,
            "longitude": 153.02
        });
        let parsed = parse_ipapi("1.1.1.1", &value).unwrap();
        assert_eq!(parsed["country"], json!("澳大利亚"));
        assert_eq!(parsed["isp"], json!("Cloudflare"));
    }
}
