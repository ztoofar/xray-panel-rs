//! Periodic real-connection checks for selected outbounds.
//!
//! The check reuses the same temporary-Xray SOCKS+HTTP probe as the outbound
//! page's "real" mode, then notifies through the WeCom webhook configured in
//! panel settings. Settings are read from `x3ui_settings` on every tick so a
//! save takes effect without restarting the panel or Xray.

use std::{collections::HashMap, time::Duration};

use axum::{extract::State, Json};
use serde::Serialize;
use serde_json::Value;

use crate::{api::compat, api::outbound_test, db, notify, service, state::AppState};

pub const ENABLED_KEY: &str = "outboundCheckEnabled";
pub const TAGS_KEY: &str = "outboundCheckTags";
pub const INTERVAL_KEY: &str = "outboundCheckIntervalMinutes";

const DEFAULT_INTERVAL_MINUTES: i64 = 5;
const MIN_INTERVAL_MINUTES: i64 = 1;
const MAX_INTERVAL_MINUTES: i64 = 1440;
const TICK: Duration = Duration::from_secs(10);
// While an outbound stays down, repeat the alert at most this often.
const REPEAT_NOTIFY_SECS: i64 = 30 * 60;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboundCheckResult {
    pub tag: String,
    pub success: bool,
    pub delay: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboundCheckRun {
    pub checked_at: i64,
    pub results: Vec<OutboundCheckResult>,
}

pub fn settings_enabled(settings: &serde_json::Map<String, Value>) -> bool {
    settings
        .get(ENABLED_KEY)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub fn interval_minutes(settings: &serde_json::Map<String, Value>) -> i64 {
    settings
        .get(INTERVAL_KEY)
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        })
        .unwrap_or(DEFAULT_INTERVAL_MINUTES)
        .clamp(MIN_INTERVAL_MINUTES, MAX_INTERVAL_MINUTES)
}

pub fn selected_tags(settings: &serde_json::Map<String, Value>) -> Vec<String> {
    let raw = settings
        .get(TAGS_KEY)
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut tags = Vec::new();
    for tag in raw.split(',') {
        let tag = tag.trim();
        if !tag.is_empty() && !tags.iter().any(|known| known == tag) {
            tags.push(tag.to_string());
        }
    }
    tags
}

pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        let mut last_run = db::models::now();
        let mut previous: HashMap<String, bool> = HashMap::new();
        let mut last_notified: HashMap<String, i64> = HashMap::new();
        loop {
            let settings = db::settings::ui_settings(&state.db)
                .await
                .unwrap_or_default();
            if !settings_enabled(&settings) {
                last_run = 0;
                previous.clear();
                last_notified.clear();
            } else {
                let interval = interval_minutes(&settings) * 60;
                let now = db::models::now();
                if now.saturating_sub(last_run) >= interval {
                    last_run = now;
                    let tags = selected_tags(&settings);
                    if !tags.is_empty() {
                        match run(&state, &tags).await {
                            Ok(run) => {
                                notify_changes(&state, &run, &mut previous, &mut last_notified)
                                    .await;
                            }
                            Err(err) => {
                                tracing::warn!(error = %err, "outbound connectivity check failed");
                            }
                        }
                    }
                }
            }
            tokio::select! {
                _ = tokio::time::sleep(TICK) => {}
                _ = state.outbound_check_notify.notified() => {
                    last_run = 0;
                }
            }
        }
    });
}

async fn run(state: &AppState, tags: &[String]) -> anyhow::Result<OutboundCheckRun> {
    let config = service::runtime_config_value(state).await?;
    let all_outbounds: Vec<Value> = config
        .get("outbounds")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let selected: Vec<Value> = tags
        .iter()
        .filter_map(|tag| {
            all_outbounds
                .iter()
                .find(|outbound| outbound.get("tag").and_then(Value::as_str) == Some(tag))
                .cloned()
        })
        .collect();
    if selected.is_empty() {
        anyhow::bail!("所选出站均不存在，请重新选择");
    }
    let raw = outbound_test::probe_real(state, &selected, &all_outbounds).await;
    let results = raw
        .iter()
        .map(|value| OutboundCheckResult {
            tag: value
                .get("tag")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            success: value
                .get("success")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            delay: value.get("delay").and_then(Value::as_u64),
            error: value
                .get("error")
                .and_then(Value::as_str)
                .map(str::to_string),
        })
        .collect::<Vec<_>>();
    let run = OutboundCheckRun {
        checked_at: db::models::now(),
        results,
    };
    *state.outbound_check.lock().await = Some(run.clone());
    tracing::info!(
        checked = run.results.len(),
        failed = run.results.iter().filter(|result| !result.success).count(),
        "outbound connectivity check finished"
    );
    Ok(run)
}

async fn notify_changes(
    state: &AppState,
    run: &OutboundCheckRun,
    previous: &mut HashMap<String, bool>,
    last_notified: &mut HashMap<String, i64>,
) {
    let mut down: Vec<&OutboundCheckResult> = Vec::new();
    let mut recovered: Vec<&OutboundCheckResult> = Vec::new();
    for result in &run.results {
        let was = previous.get(&result.tag).copied();
        if !result.success {
            let repeat_due = last_notified
                .get(&result.tag)
                .map(|at| run.checked_at.saturating_sub(*at) >= REPEAT_NOTIFY_SECS)
                .unwrap_or(true);
            if was != Some(false) || repeat_due {
                down.push(result);
                last_notified.insert(result.tag.clone(), run.checked_at);
            }
        } else if was == Some(false) {
            recovered.push(result);
        }
        previous.insert(result.tag.clone(), result.success);
    }
    if down.is_empty() && recovered.is_empty() {
        return;
    }
    let mut content = String::from("【Lumen 出站检测】");
    if !down.is_empty() {
        content.push_str("\n以下出站真实连接不可用：");
        for result in &down {
            content.push_str(&format!("\n- {}", result.tag));
            if let Some(error) = result.error.as_deref().filter(|text| !text.is_empty()) {
                content.push_str(&format!("：{error}"));
            }
        }
    }
    if !recovered.is_empty() {
        content.push_str("\n以下出站已恢复：");
        for result in &recovered {
            content.push_str(&format!("\n- {}", result.tag));
            if let Some(delay) = result.delay {
                content.push_str(&format!("（{delay} ms）"));
            }
        }
    }
    content.push_str(&format!(
        "\n时间：{}",
        crate::panel_log::format_china_time(chrono::Utc::now())
    ));
    if let Err(err) = notify::send_text(state, content).await {
        tracing::warn!(error = %err, "outbound check notification failed");
    }
}

pub async fn status(State(state): State<AppState>) -> Json<Value> {
    let run = state.outbound_check.lock().await.clone();
    compat::ok(serde_json::to_value(run.unwrap_or_default()).unwrap_or(Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(pairs: &[(&str, Value)]) -> serde_json::Map<String, Value> {
        pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.clone()))
            .collect()
    }

    #[test]
    fn tag_selection_trims_deduplicates_and_skips_blanks() {
        let map = settings(&[(TAGS_KEY, serde_json::json!(" a, b ,,a, c "))]);
        assert_eq!(selected_tags(&map), ["a", "b", "c"]);
    }

    #[test]
    fn interval_uses_default_and_clamps_out_of_range_values() {
        assert_eq!(interval_minutes(&settings(&[])), DEFAULT_INTERVAL_MINUTES);
        assert_eq!(
            interval_minutes(&settings(&[(INTERVAL_KEY, serde_json::json!(0))])),
            MIN_INTERVAL_MINUTES
        );
        assert_eq!(
            interval_minutes(&settings(&[(INTERVAL_KEY, serde_json::json!(99999))])),
            MAX_INTERVAL_MINUTES
        );
    }
}
