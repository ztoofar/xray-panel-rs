use std::{env, net::SocketAddr, path::PathBuf};

use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct Config {
    pub listen: SocketAddr,
    pub db_path: PathBuf,
    pub xray_bin: PathBuf,
    pub xray_config: PathBuf,
    pub xray_log_file: PathBuf,
    pub xray_api_addr: String,
    pub init_username: String,
    pub init_password: Option<String>,
    pub workers: usize,
    pub secure_cookie: bool,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let listen_raw = env::var("PANEL_LISTEN").unwrap_or_else(|_| "0.0.0.0:2053".into());
        let listen = listen_raw
            .parse::<SocketAddr>()
            .with_context(|| format!("invalid PANEL_LISTEN: {listen_raw}"))?;

        let workers = env::var("PANEL_WORKERS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(2)
            .max(1);

        let db_path: PathBuf = env::var("PANEL_DB")
            .unwrap_or_else(|_| "xray-panel.db".into())
            .into();
        let xray_log_file = env::var("PANEL_XRAY_LOG")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                db_path
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                    .map(|parent| parent.join("xray.log"))
                    .unwrap_or_else(|| PathBuf::from("xray.log"))
            });
        Ok(Self {
            listen,
            db_path,
            xray_bin: env::var("XRAY_BIN")
                .unwrap_or_else(|_| "/usr/local/bin/xray".into())
                .into(),
            xray_config: env::var("XRAY_CONFIG")
                .unwrap_or_else(|_| "/usr/local/etc/xray/config.json".into())
                .into(),
            xray_log_file,
            xray_api_addr: env::var("XRAY_API_ADDR").unwrap_or_else(|_| "127.0.0.1:10085".into()),
            init_username: env::var("PANEL_USERNAME").unwrap_or_else(|_| "admin".into()),
            init_password: env::var("PANEL_PASSWORD").ok().filter(|v| !v.is_empty()),
            workers,
            secure_cookie: env::var("PANEL_SECURE_COOKIE")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
        })
    }

    pub fn xray_api_url(&self) -> String {
        format!("http://{}", self.xray_api_addr)
    }

    pub fn xray_api_port(&self) -> u16 {
        self.xray_api_addr
            .rsplit(':')
            .next()
            .and_then(|p| p.parse().ok())
            .unwrap_or(10085)
    }
}
