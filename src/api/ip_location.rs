use std::{net::IpAddr, time::Duration};

use axum::{
    extract::{Request, State},
    Json,
};
use serde_json::{json, Value};

use crate::{db, state::AppState};

use super::compat;

const LOOKUP_TIMEOUT: Duration = Duration::from_secs(8);

// Location lookups are only made when the admin clicks the button, so a slow
// third-party call never touches the dashboard polling path.
pub async fn ip_location(State(state): State<AppState>, req: Request) -> Json<Value> {
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
    let api_key = ip_data_cloud_key(&state).await;
    match lookup(ip, api_key).await {
        Ok(value) => compat::ok(value),
        Err(err) => compat::fail(format!("查询 IP 位置失败：{err}")),
    }
}

async fn ip_data_cloud_key(state: &AppState) -> String {
    db::settings::ui_settings(&state.db)
        .await
        .ok()
        .and_then(|settings| {
            settings
                .get("ipDataCloudKey")
                .and_then(|value| match value {
                    Value::String(text) => Some(text.trim().to_string()),
                    Value::Number(number) => Some(number.to_string()),
                    _ => None,
                })
        })
        .filter(|key| !key.is_empty())
        .unwrap_or_default()
}

async fn lookup(ip: IpAddr, api_key: String) -> anyhow::Result<Value> {
    let ip_text = ip.to_string();
    let base_task = tokio::task::spawn_blocking(move || base_lookup(ip));
    let cloud_task = tokio::task::spawn_blocking(move || {
        if api_key.is_empty() {
            None
        } else {
            Some(ip_data_cloud_lookup(ip, &api_key))
        }
    });
    let (base_result, cloud_result) = tokio::join!(base_task, cloud_task);
    let base_result = base_result.map_err(|err| anyhow::anyhow!("查询任务失败：{err}"))?;
    let cloud_result = cloud_result.map_err(|err| anyhow::anyhow!("查询任务失败：{err}"))?;
    match base_result {
        Ok(mut value) => {
            if let (Some(result), Some(object)) = (cloud_result, value.as_object_mut()) {
                match result {
                    Ok(cloud) => {
                        object.insert("ipDataCloud".into(), cloud);
                    }
                    Err(err) => {
                        object.insert("ipDataCloudError".into(), json!(err.to_string()));
                    }
                }
            }
            Ok(value)
        }
        Err(base_err) => match cloud_result {
            Some(Ok(cloud)) => {
                let mut value = location_object(
                    &ip_text,
                    "ipdatacloud",
                    LocationFields {
                        country: text_field(&cloud, "country"),
                        region: text_field(&cloud, "province"),
                        city: text_field(&cloud, "city"),
                        isp: text_field(&cloud, "isp"),
                        timezone: text_field(&cloud, "timezone"),
                        latitude: cloud.get("latitude").and_then(Value::as_f64),
                        longitude: cloud.get("longitude").and_then(Value::as_f64),
                    },
                );
                if let Some(object) = value.as_object_mut() {
                    object.insert("ipDataCloud".into(), cloud);
                }
                Ok(value)
            }
            Some(Err(cloud_err)) => Err(anyhow::anyhow!(
                "位置服务不可用（{base_err}；IP数据云：{cloud_err}）"
            )),
            None => Err(base_err),
        },
    }
}

fn base_lookup(ip: IpAddr) -> anyhow::Result<Value> {
    let ip_text = ip.to_string();
    let primary = format!("https://ip.zxinc.org/api.php?type=json&ip={ip_text}");
    let primary_result = fetch_json(&primary).and_then(|value| parse_zxinc(&ip_text, &value));
    match primary_result {
        Ok(value) => Ok(value),
        Err(primary_err) => {
            let fallback = format!("https://ipwho.is/{ip_text}?lang=zh-CN");
            fetch_json(&fallback)
                .and_then(|value| parse_ipwho(&ip_text, &value))
                .map_err(|fallback_err| {
                    anyhow::anyhow!(
                        "位置服务不可用（纯真：{primary_err}；ipwho.is：{fallback_err}）"
                    )
                })
        }
    }
}

fn ip_data_cloud_lookup(ip: IpAddr, api_key: &str) -> anyhow::Result<Value> {
    let mut url = url::Url::parse("https://api.ipdatacloud.com/v2/query")
        .map_err(|err| anyhow::anyhow!("IP数据云地址无效：{err}"))?;
    url.query_pairs_mut()
        .append_pair("ip", &ip.to_string())
        .append_pair("key", api_key);
    let ip_text = ip.to_string();
    let value = fetch_json(url.as_str())?;
    parse_ipdatacloud(&ip_text, &value)
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

fn integer_field(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|entry| {
        entry
            .as_i64()
            .or_else(|| entry.as_str().and_then(|text| text.trim().parse().ok()))
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

fn parse_zxinc(ip: &str, value: &Value) -> anyhow::Result<Value> {
    if value.get("code").and_then(Value::as_i64) != Some(0) {
        let message = text_field(value, "msg");
        anyhow::bail!(if message.is_empty() {
            "位置服务拒绝查询".to_string()
        } else {
            message
        });
    }
    let data = value
        .get("data")
        .ok_or_else(|| anyhow::anyhow!("位置服务返回格式错误：缺少 data"))?;
    // zxinc returns 中国–浙江–杭州 (en dash) or tab-separated names for IPv6.
    let parts: Vec<String> = text_field(data, "country")
        .split(['–', '-', '\t'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect();
    let country = parts.first().cloned().unwrap_or_default();
    let region = parts.get(1).cloned().unwrap_or_default();
    let city = if parts.len() > 2 {
        parts[2..].join(" ")
    } else {
        String::new()
    };
    let isp = text_field(data, "local")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if country.is_empty() && region.is_empty() && city.is_empty() && isp.is_empty() {
        anyhow::bail!("位置服务未返回归属地");
    }
    Ok(location_object(
        ip,
        "zxinc",
        LocationFields {
            country,
            region,
            city,
            isp,
            timezone: String::new(),
            latitude: None,
            longitude: None,
        },
    ))
}

fn parse_ipdatacloud(ip: &str, value: &Value) -> anyhow::Result<Value> {
    if let Some(code) = integer_field(value, "code") {
        if code != 200 {
            let message = text_field(value, "msg");
            anyhow::bail!(if message.is_empty() {
                format!("IP数据云拒绝查询（code {code}）")
            } else {
                message
            });
        }
    }
    let data = value
        .get("data")
        .ok_or_else(|| anyhow::anyhow!("IP数据云返回格式错误：缺少 data"))?;
    // Some products nest the attribution fields under data.location.
    let source = data
        .get("location")
        .filter(|entry| entry.is_object())
        .unwrap_or(data);
    let country = text_field(source, "country");
    let province = text_field(source, "province");
    let city = text_field(source, "city");
    let district = text_field(source, "district");
    let street = text_field(source, "street");
    let isp = text_field(source, "isp");
    let mut usage_type = text_field(source, "usage_type");
    if usage_type.is_empty() {
        usage_type = text_field(data, "usage_type");
    }
    if usage_type.is_empty() {
        if let Some(scenes) = data.get("scenes") {
            usage_type = text_field(scenes, "usage_type");
        }
    }
    let timezone = text_field(source, "time_zone");
    if country.is_empty()
        && province.is_empty()
        && city.is_empty()
        && district.is_empty()
        && street.is_empty()
        && isp.is_empty()
    {
        anyhow::bail!("IP数据云未返回归属地");
    }
    Ok(json!({
        "ip": ip,
        "provider": "ipdatacloud",
        "country": country,
        "province": province,
        "city": city,
        "district": district,
        "street": street,
        "isp": isp,
        "usageType": usage_type,
        "timezone": timezone,
        "latitude": number_field(source, "latitude"),
        "longitude": number_field(source, "longitude"),
    }))
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
    fn zxinc_response_is_normalized() {
        let value = json!({
            "code": 0,
            "data": {
                "location": "中国–浙江–杭州 电信",
                "country": "中国–浙江–杭州",
                "local": "电信"
            }
        });
        let parsed = parse_zxinc("115.193.109.37", &value).unwrap();
        assert_eq!(parsed["provider"], json!("zxinc"));
        assert_eq!(parsed["country"], json!("中国"));
        assert_eq!(parsed["region"], json!("浙江"));
        assert_eq!(parsed["city"], json!("杭州"));
        assert_eq!(parsed["isp"], json!("电信"));
        assert!(parsed["latitude"].is_null());
    }

    #[test]
    fn zxinc_tab_separated_and_extra_levels_are_normalized() {
        let value = json!({
            "code": 0,
            "data": {
                "country": "中国\t浙江省\t\t杭州市",
                "local": "中国电信\t公众宽带"
            }
        });
        let parsed = parse_zxinc("240e:390:2ca:ec30::1", &value).unwrap();
        assert_eq!(parsed["country"], json!("中国"));
        assert_eq!(parsed["region"], json!("浙江省"));
        assert_eq!(parsed["city"], json!("杭州市"));
        assert_eq!(parsed["isp"], json!("中国电信 公众宽带"));

        let foreign = json!({
            "code": 0,
            "data": {
                "country": "美国–加利福尼亚州–圣克拉拉–山景城",
                "local": "谷歌公司DNS服务器"
            }
        });
        let parsed = parse_zxinc("8.8.8.8", &foreign).unwrap();
        assert_eq!(parsed["country"], json!("美国"));
        assert_eq!(parsed["region"], json!("加利福尼亚州"));
        assert_eq!(parsed["city"], json!("圣克拉拉 山景城"));
    }

    #[test]
    fn zxinc_failure_is_reported() {
        let value = json!({ "code": 1, "msg": "invalid IP" });
        let err = parse_zxinc("1.2.3.4", &value).unwrap_err();
        assert!(err.to_string().contains("invalid IP"));
    }

    #[test]
    fn ipdatacloud_response_is_normalized() {
        let value = json!({
            "code": 200,
            "msg": "success",
            "data": {
                "country": "中国",
                "province": "浙江省",
                "city": "杭州市",
                "district": "西湖区",
                "street": "文三路",
                "isp": "电信",
                "usage_type": "家庭宽带",
                "time_zone": "Asia/Shanghai",
                "latitude": 30.27,
                "longitude": 120.15
            }
        });
        let parsed = parse_ipdatacloud("115.193.109.37", &value).unwrap();
        assert_eq!(parsed["provider"], json!("ipdatacloud"));
        assert_eq!(parsed["country"], json!("中国"));
        assert_eq!(parsed["province"], json!("浙江省"));
        assert_eq!(parsed["city"], json!("杭州市"));
        assert_eq!(parsed["district"], json!("西湖区"));
        assert_eq!(parsed["street"], json!("文三路"));
        assert_eq!(parsed["isp"], json!("电信"));
        assert_eq!(parsed["usageType"], json!("家庭宽带"));
        assert_eq!(parsed["timezone"], json!("Asia/Shanghai"));
    }

    #[test]
    fn ipdatacloud_nested_location_and_string_numbers_are_normalized() {
        let value = json!({
            "code": "200",
            "data": {
                "location": {
                    "country": "美国",
                    "province": "加利福尼亚州",
                    "city": "山景城",
                    "isp": "Google",
                    "latitude": "37.4",
                    "longitude": "-122.07"
                },
                "scenes": { "usage_type": "IDC" }
            }
        });
        let parsed = parse_ipdatacloud("8.8.8.8", &value).unwrap();
        assert_eq!(parsed["country"], json!("美国"));
        assert_eq!(parsed["city"], json!("山景城"));
        assert_eq!(parsed["usageType"], json!("IDC"));
        assert_eq!(parsed["latitude"], json!(37.4));
        assert_eq!(parsed["longitude"], json!(-122.07));
    }

    #[test]
    fn ipdatacloud_failure_is_reported() {
        let value = json!({ "code": 500, "msg": "key不能为空" });
        let err = parse_ipdatacloud("1.2.3.4", &value).unwrap_err();
        assert!(err.to_string().contains("key不能为空"));
    }
}
