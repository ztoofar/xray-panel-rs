// tonic's generated client returns large Status errors; generated code is not ours to reshape.
#![allow(clippy::result_large_err)]

mod api;
mod auth;
mod config;
mod db;
mod error;
mod jobs;
mod notify;
mod outbound_subs;
mod panel_log;
mod panel_update;
mod service;
mod state;
mod sys;
mod traffic;
mod updates;
mod web;
mod xray;

use std::{
    collections::{HashMap, VecDeque},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{Arc, RwLock},
    time::Duration,
};

use anyhow::Context;
use tokio::sync::{Mutex, Notify};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

use crate::{
    config::Config, db::models::now, panel_log::PanelLogBuffer, state::AppState,
    xray::config::DEFAULT_EXTRA_JSON,
};

fn main() -> anyhow::Result<()> {
    let panel_logs = init_tracing();
    let cfg = Config::from_env()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(cfg.workers)
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    runtime.block_on(run(cfg, panel_logs))
}

fn init_tracing() -> PanelLogBuffer {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let level = std::env::var("PANEL_LOG").unwrap_or_else(|_| "info".into());
        EnvFilter::new(format!("xray_panel_rs={level},tower_http=warn"))
    });
    // Two fmt layers: stdout keeps the normal output, the second one mirrors
    // the same events into the in-memory buffer served by the log viewer.
    // ANSI is disabled there so the web UI does not show escape sequences.
    let panel_logs = panel_log::buffer();
    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .compact()
        .with_filter(filter.clone());
    let buffer_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_ansi(false)
        .compact()
        .with_writer(panel_log::PanelLogMakeWriter::new(panel_logs.clone()))
        .with_filter(filter);
    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(buffer_layer)
        .init();
    panel_logs
}

async fn run(cfg: Config, panel_logs: PanelLogBuffer) -> anyhow::Result<()> {
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        workers = cfg.workers,
        "starting xray-panel-rs"
    );

    let db = db::init(&cfg.db_path).await?;
    seed_defaults(&db).await?;
    seed_admin(&cfg, &db).await?;
    // The settings page owns the listen address; PANEL_LISTEN is only the
    // bootstrap default until the admin saves one.
    let cfg = apply_saved_runtime(cfg, &db).await;
    let base_path = load_base_path(&db).await;

    let xray_logs = Arc::new(Mutex::new(VecDeque::new()));
    let xray = xray::XrayHandle::spawn(Arc::new(cfg.clone()), xray_logs.clone());
    let stats = xray::StatsClient::new(cfg.xray_api_url());
    let state = AppState {
        db: db.clone(),
        cfg: Arc::new(cfg.clone()),
        xray,
        stats,
        xray_logs,
        panel_logs,
        online_clients: Arc::new(Mutex::new(HashMap::new())),
        online_last_seen: Arc::new(Mutex::new(HashMap::new())),
        active_inbounds: Arc::new(Mutex::new(Vec::new())),
        active_inbound_last_seen: Arc::new(Mutex::new(HashMap::new())),
        login_limiter: Arc::new(Mutex::new(HashMap::new())),
        traffic_history: Arc::new(Mutex::new(VecDeque::with_capacity(240))),
        refresh_notify: Arc::new(Notify::new()),
        base_path: Arc::new(RwLock::new(base_path)),
        started_at: now(),
    };

    spawn_maintenance(state.clone());
    service::apply_on_start(&state).await;
    jobs::spawn(state.clone());
    outbound_subs::spawn(state.clone());

    let listener = tokio::net::TcpListener::bind(cfg.listen)
        .await
        .with_context(|| format!("bind {}", cfg.listen))?;
    tracing::info!(listen = %cfg.listen, "panel listening");

    axum::serve(
        listener,
        api::router(state.clone()).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .context("http server")?;

    state.xray.stop().await;
    tracing::info!("panel stopped");
    Ok(())
}

async fn apply_saved_runtime(mut cfg: Config, db: &db::Db) -> Config {
    let Ok(settings) = db::settings::ui_settings(db).await else {
        return cfg;
    };
    let listen = settings
        .get("webListen")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim();
    let port = settings.get("webPort").and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
    });
    let ip = if listen.is_empty() {
        Some(IpAddr::V4(Ipv4Addr::UNSPECIFIED))
    } else {
        listen.parse().ok()
    };
    if let (Some(ip), Some(port)) = (ip, port.and_then(|value| u16::try_from(value).ok())) {
        if port > 0 {
            cfg.listen = SocketAddr::new(ip, port);
        }
    }
    let path = |key: &str| {
        settings
            .get(key)
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(std::path::PathBuf::from)
    };
    if let Some(bin) = path("xrayBin") {
        cfg.xray_bin = bin;
    }
    if let Some(config) = path("xrayConfig") {
        cfg.xray_config = config;
    }
    if let Some(log_file) = path("xrayLogFile") {
        cfg.xray_log_file = log_file;
    }
    if let Some(api) = settings
        .get("xrayApiAddr")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        cfg.xray_api_addr = api.to_string();
    }
    if let Some(value) = settings
        .get("secureCookie")
        .and_then(|value| value.as_bool())
    {
        cfg.secure_cookie = value;
    }
    cfg
}

async fn seed_defaults(db: &db::Db) -> anyhow::Result<()> {
    db::settings::seed(
        db,
        &[
            ("panel_name", "Xray Panel"),
            ("public_address", ""),
            ("traffic_poll_secs", "5"),
            ("xray_extra", DEFAULT_EXTRA_JSON),
        ],
    )
    .await?;
    Ok(())
}

async fn seed_admin(cfg: &Config, db: &db::Db) -> anyhow::Result<()> {
    if db::users::count(db).await? > 0 {
        if let Ok(Some(user)) = db::users::find_by_username(db, &cfg.init_username).await {
            if auth::verify_password("admin", &user.password_hash) {
                tracing::warn!(
                    "default admin/admin credentials are still active; change the password now"
                );
            }
        }
        return Ok(());
    }

    let password = cfg
        .init_password
        .clone()
        .unwrap_or_else(|| "admin".to_string());
    let hash = auth::hash_password(&password)?;
    db::users::create(db, &cfg.init_username, &hash).await?;

    println!("============================================================");
    println!("  首次启动已创建管理员账号");
    println!("  用户名: {}", cfg.init_username);
    println!("  密码:   {password}");
    println!("  请登录后立即在“面板设置”中修改密码");
    println!("============================================================");
    tracing::warn!(
        username = %cfg.init_username,
        "initial admin created with default credentials unless PANEL_PASSWORD was set"
    );
    Ok(())
}

async fn load_base_path(db: &db::Db) -> String {
    db::settings::ui_settings(db)
        .await
        .ok()
        .and_then(|settings| {
            settings
                .get("webBasePath")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .and_then(|raw| web::base_path::normalize(&raw))
        .unwrap_or_else(|| "/".to_string())
}

const MAINTENANCE_INTERVAL: Duration = Duration::from_secs(15 * 60);

fn spawn_maintenance(state: AppState) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(MAINTENANCE_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // The first interval tick fires immediately; skip it so startup stays quiet.
        ticker.tick().await;
        loop {
            ticker.tick().await;
            match db::sessions::purge_expired(&state.db, now()).await {
                Ok(0) => {}
                Ok(removed) => tracing::debug!(removed, "purged expired sessions"),
                Err(err) => tracing::warn!(error = %err, "purge sessions failed"),
            }
            release_memory(&state).await;
        }
    });
}

async fn release_memory(state: &AppState) {
    // SQLite page/temp caches are the largest reclaimable block the panel owns.
    if let Err(err) = sqlx::query("PRAGMA shrink_memory").execute(&state.db).await {
        tracing::debug!(error = %err, "sqlite shrink_memory failed");
    }
    state.online_clients.lock().await.shrink_to_fit();
    state.online_last_seen.lock().await.shrink_to_fit();
    state.active_inbound_last_seen.lock().await.shrink_to_fit();
    state.active_inbounds.lock().await.shrink_to_fit();
    state.xray_logs.lock().await.shrink_to_fit();
    if let Ok(mut logs) = state.panel_logs.lock() {
        logs.shrink_to_fit();
    }
    state.traffic_history.lock().await.shrink_to_fit();
    {
        let mut limiter = state.login_limiter.lock().await;
        let cutoff = now() - 60;
        limiter.retain(|_, attempts| {
            attempts.retain(|at| *at > cutoff);
            !attempts.is_empty()
        });
        limiter.shrink_to_fit();
    }
    trim_heap();
    tracing::debug!("maintenance: released idle panel memory");
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn trim_heap() {
    // glibc retains freed arenas; musl's mallocng madvises empty groups itself.
    unsafe {
        libc::malloc_trim(0);
    }
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn trim_heap() {}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(err) => tracing::warn!(error = %err, "SIGTERM handler unavailable"),
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
}
