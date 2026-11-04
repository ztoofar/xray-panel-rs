use anyhow::{bail, Context};
use serde_json::Value;

use crate::{
    db, outbound_subs,
    state::AppState,
    xray::{
        config::build_config,
        supervisor::{temp_config_path, test_config, write_config},
    },
};

pub async fn load_extra(state: &AppState) -> anyhow::Result<Value> {
    let raw = db::settings::get(&state.db, "xray_extra")
        .await?
        .unwrap_or_else(|| "{}".into());
    let extra: Value = serde_json::from_str(&raw).context("xray_extra 不是有效的 JSON")?;
    if !extra.is_object() {
        bail!("xray_extra 必须为 JSON 对象");
    }
    Ok(extra)
}

pub async fn clients_with_inbounds(state: &AppState) -> anyhow::Result<Vec<db::models::Client>> {
    let mut clients = db::clients::list_all(&state.db).await?;
    let associations = db::client_inbounds::all(&state.db).await?;
    for client in &mut clients {
        client.inbound_ids = associations
            .get(&client.id)
            .cloned()
            .unwrap_or_else(|| vec![client.inbound_id]);
    }
    Ok(clients)
}

pub async fn config_value(state: &AppState) -> anyhow::Result<Value> {
    state_config(state, false).await
}

// What the Xray page edits: the effective config minus blocks the panel owns
// (inbounds are DB-backed, api/stats are generated). This keeps the advanced
// JSON editor from showing values that a save would silently discard.
pub async fn editable_config_value(state: &AppState) -> anyhow::Result<Value> {
    let mut config = config_value(state).await?;
    if let Some(object) = config.as_object_mut() {
        object.remove("inbounds");
        object.remove("api");
        object.remove("stats");
    }
    Ok(config)
}

// `runtime` expands per-client routing rules to the attachment identities that
// actually exist in Xray. API responses keep the panel email so the UI never
// shows "12:alice".
async fn state_config(state: &AppState, runtime: bool) -> anyhow::Result<Value> {
    let inbounds = db::inbounds::list_enabled(&state.db).await?;
    let clients = clients_with_inbounds(state).await?;
    let extra = load_extra(state).await?;
    build_config(
        &inbounds,
        &clients,
        &extra,
        state.cfg.xray_api_port(),
        runtime,
    )
}

pub async fn test_extra(state: &AppState, extra: &Value) -> anyhow::Result<()> {
    let inbounds = db::inbounds::list_enabled(&state.db).await?;
    let clients = clients_with_inbounds(state).await?;
    // The runtime config also carries subscription outbounds, so a template
    // save must be validated against the same merged view.
    let mut extra = extra.clone();
    let (prepend, append) = outbound_subs::active_split(state).await?;
    outbound_subs::merge_into_extra(&mut extra, &prepend, &append);
    let config = build_config(&inbounds, &clients, &extra, state.cfg.xray_api_port(), true)?;
    write_test_config(state, &config).await
}

pub async fn test_current(state: &AppState) -> anyhow::Result<()> {
    let extra = load_extra(state).await?;
    test_extra(state, &extra).await
}

async fn write_test_config(state: &AppState, config: &Value) -> anyhow::Result<()> {
    let path = temp_config_path(&state.cfg.xray_config, "test");
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
    }
    tokio::fs::write(&path, serde_json::to_vec_pretty(config)?).await?;
    let result = test_config(&state.cfg, &path).await;
    let _ = tokio::fs::remove_file(&path).await;
    result
}

// The exact config Xray would run, including subscription outbounds. Callers
// that inspect outbounds (connectivity checks) must see the merged view.
pub async fn runtime_config_value(state: &AppState) -> anyhow::Result<Value> {
    let inbounds = db::inbounds::list_enabled(&state.db).await?;
    let clients = clients_with_inbounds(state).await?;
    let mut extra = load_extra(state).await?;
    let (prepend, append) = outbound_subs::active_split(state).await?;
    outbound_subs::merge_into_extra(&mut extra, &prepend, &append);
    build_config(&inbounds, &clients, &extra, state.cfg.xray_api_port(), true)
}

pub async fn write_current(state: &AppState) -> anyhow::Result<()> {
    let config = runtime_config_value(state).await?;
    write_config(&state.cfg, &config).await
}

pub async fn apply(state: &AppState) -> anyhow::Result<()> {
    write_current(state).await?;
    state
        .xray
        .restart()
        .await
        .map_err(|err| anyhow::anyhow!("重启 Xray 失败：{err}"))
}

pub async fn apply_on_start(state: &AppState) {
    match write_current(state).await {
        Ok(()) => match state.xray.restart().await {
            Ok(()) => tracing::info!("xray config applied on startup"),
            Err(err) => tracing::warn!(error = %err, "xray did not start"),
        },
        Err(err) => tracing::warn!(error = %err, "could not apply xray config on startup"),
    }
}
