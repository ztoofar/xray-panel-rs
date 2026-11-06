use std::{
    collections::VecDeque,
    path::Path,
    process::Stdio,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader},
    process::{Child, Command},
    sync::{mpsc, oneshot, Mutex},
};

use crate::{config::Config, db::models::now};

pub type LogBuffer = Arc<Mutex<VecDeque<String>>>;
const MAX_LOG_LINES: usize = 500;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct XrayStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub started_at: Option<i64>,
    pub last_exit_at: Option<i64>,
    pub last_exit_code: Option<i32>,
    pub last_error: Option<String>,
    pub restarts: u64,
    pub version: Option<String>,
}

enum CommandMsg {
    Restart {
        reply: oneshot::Sender<Result<(), String>>,
    },
    Status {
        reply: oneshot::Sender<XrayStatus>,
    },
    Stop {
        reply: oneshot::Sender<()>,
    },
}

#[derive(Clone)]
pub struct XrayHandle {
    tx: mpsc::Sender<CommandMsg>,
    generation: Arc<AtomicU64>,
    start_epoch: Arc<AtomicU64>,
}

impl XrayHandle {
    pub fn spawn(cfg: Arc<Config>, logs: LogBuffer) -> Self {
        let (tx, rx) = mpsc::channel(8);
        let generation = Arc::new(AtomicU64::new(0));
        let start_epoch = Arc::new(AtomicU64::new(0));
        tokio::spawn(run_supervisor(
            cfg,
            logs,
            generation.clone(),
            start_epoch.clone(),
            rx,
        ));
        Self {
            tx,
            generation,
            start_epoch,
        }
    }

    pub async fn restart(&self) -> std::result::Result<(), String> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(CommandMsg::Restart { reply })
            .await
            .map_err(|_| "xray supervisor stopped".to_string())?;
        rx.await
            .map_err(|_| "xray supervisor stopped".to_string())?
    }

    pub async fn status(&self) -> XrayStatus {
        let (reply, rx) = oneshot::channel();
        if self.tx.send(CommandMsg::Status { reply }).await.is_err() {
            return XrayStatus::default();
        }
        rx.await.unwrap_or_default()
    }

    pub async fn stop(&self) {
        let (reply, rx) = oneshot::channel();
        if self.tx.send(CommandMsg::Stop { reply }).await.is_ok() {
            let _ = rx.await;
        }
    }

    /// Monotonic counter bumped whenever the xray child starts or stops.
    /// Every live connection dies with the process, so the panel clears its
    /// online view whenever this value changes.
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    /// Monotonic counter bumped only when a new child process starts. Unlike
    /// [`Self::generation`] it does not change on stop, so the traffic job
    /// can tell "a fresh process now owns the counters" from "the old process
    /// is going away".
    pub fn start_epoch(&self) -> u64 {
        self.start_epoch.load(Ordering::Relaxed)
    }
}

async fn run_supervisor(
    cfg: Arc<Config>,
    logs: LogBuffer,
    generation: Arc<AtomicU64>,
    start_epoch: Arc<AtomicU64>,
    mut rx: mpsc::Receiver<CommandMsg>,
) {
    let mut status = XrayStatus {
        version: probe_version(&cfg).await,
        ..XrayStatus::default()
    };
    let mut child: Option<Child> = None;
    let mut restart_at: Option<Instant> = None;
    let mut backoff = Duration::from_secs(2);
    let mut stopping = false;

    match start_child(&cfg, &logs, &generation, &start_epoch, &mut status).await {
        Ok(started) => child = Some(started),
        Err(err) => {
            status.last_error = Some(err.to_string());
            tracing::warn!(error = %err, "xray start failed");
        }
    }

    let mut ticker = tokio::time::interval(Duration::from_secs(2));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            message = rx.recv() => {
                let Some(message) = message else { break };
                match message {
                    CommandMsg::Status { reply } => {
                        let _ = reply.send(status.clone());
                    }
                    CommandMsg::Restart { reply } => {
                        stopping = false;
                        restart_at = None;
                        let result = restart_child(
                            &cfg,
                            &logs,
                            &generation,
                            &start_epoch,
                            &mut child,
                            &mut status,
                        )
                        .await;
                        let _ = reply.send(result);
                    }
                    CommandMsg::Stop { reply } => {
                        // Stop only the child: the panel's dashboard can start
                        // xray again afterwards, so the supervisor must stay up.
                        stopping = true;
                        restart_at = None;
                        stop_child(&mut child, &generation).await;
                        status.running = false;
                        status.pid = None;
                        let _ = reply.send(());
                    }
                }
            }
            _ = ticker.tick() => {
                if let Some(running) = child.as_mut() {
                    match running.try_wait() {
                        Ok(Some(exit)) => {
                            status.running = false;
                            status.pid = None;
                            status.last_exit_at = Some(now());
                            status.last_exit_code = exit.code();
                            child = None;
                            // The process is gone, and its connections with it.
                            bump_generation(&generation);
                            if !stopping {
                                restart_at = Some(Instant::now() + backoff);
                                backoff = (backoff * 2).min(Duration::from_secs(30));
                                tracing::warn!(code = ?status.last_exit_code, "xray exited, scheduling restart");
                            }
                        }
                        Ok(None) => {}
                        Err(err) => status.last_error = Some(format!("wait failed: {err}")),
                    }
                } else if !stopping
                    && restart_at.is_some_and(|at| Instant::now() >= at)
                {
                    restart_at = None;
                    match start_child(&cfg, &logs, &generation, &start_epoch, &mut status).await {
                        Ok(started) => {
                            child = Some(started);
                            status.restarts += 1;
                            backoff = Duration::from_secs(2);
                        }
                        Err(err) => {
                            status.last_error = Some(err.to_string());
                            restart_at = Some(Instant::now() + backoff);
                        }
                    }
                }
            }
        }
    }

    stop_child(&mut child, &generation).await;
}

async fn restart_child(
    cfg: &Config,
    logs: &LogBuffer,
    generation: &AtomicU64,
    start_epoch: &AtomicU64,
    child: &mut Option<Child>,
    status: &mut XrayStatus,
) -> std::result::Result<(), String> {
    stop_child(child, generation).await;
    status.running = false;
    status.pid = None;
    match start_child(cfg, logs, generation, start_epoch, status).await {
        Ok(started) => {
            *child = Some(started);
            Ok(())
        }
        Err(err) => {
            status.last_error = Some(err.to_string());
            Err(err.to_string())
        }
    }
}

async fn start_child(
    cfg: &Config,
    logs: &LogBuffer,
    generation: &AtomicU64,
    start_epoch: &AtomicU64,
    status: &mut XrayStatus,
) -> Result<Child> {
    if !cfg.xray_bin.exists() {
        bail!("找不到 Xray 可执行文件：{}", cfg.xray_bin.display());
    }

    let mut command = Command::new(&cfg.xray_bin);
    command
        .arg("run")
        .arg("-c")
        .arg(&cfg.xray_config)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    apply_accurate_traffic_env(&mut command);

    let mut started = command
        .spawn()
        .with_context(|| format!("spawn {}", cfg.xray_bin.display()))?;

    if let Some(stdout) = started.stdout.take() {
        spawn_log_reader(stdout, logs.clone(), "stdout", cfg.xray_log_file.clone());
    }
    if let Some(stderr) = started.stderr.take() {
        spawn_log_reader(stderr, logs.clone(), "stderr", cfg.xray_log_file.clone());
    }

    status.running = true;
    status.pid = started.id();
    status.started_at = Some(now());
    status.last_error = None;
    // A fresh process has no connections; make the panel drop its stale
    // online map even if the previous one still looked alive.
    bump_generation(generation);
    // A fresh process starts with zero counters; the traffic job uses this
    // epoch to reset its stored baselines before sampling.
    start_epoch.fetch_add(1, Ordering::Relaxed);
    tracing::info!(pid = ?status.pid, "xray started");
    let started_line = format!("[panel] xray started (pid {:?})", status.pid);
    {
        let mut buffer = logs.lock().await;
        if buffer.len() >= MAX_LOG_LINES {
            buffer.pop_front();
        }
        buffer.push_back(started_line.clone());
    }
    append_log_line(&cfg.xray_log_file, &started_line).await;
    Ok(started)
}

/// Xray's freedom spliced copy (Linux/Android) tallies a stream's bytes only
/// when the connection ends: a tunnel download reads 0 B/s for its whole
/// lifetime, lands as one lump at close, and is lost entirely if the core is
/// killed mid-transfer. The panel needs per-write counters for both the live
/// speed and stable billing, so splice is off unless the operator chose it.
fn apply_accurate_traffic_env(command: &mut Command) {
    if let Some((name, value)) = splice_env_override(|key| std::env::var_os(key).is_some()) {
        tracing::debug!(name, value, "xray child environment override");
        command.env(name, value);
    }
}

fn splice_env_override(configured: impl Fn(&str) -> bool) -> Option<(&'static str, &'static str)> {
    const NAME: &str = "XRAY_BUF_SPLICE";
    const ALT_NAME: &str = "xray.buf.splice";
    if configured(NAME) || configured(ALT_NAME) {
        None
    } else {
        Some((NAME, "disable"))
    }
}

async fn stop_child(child: &mut Option<Child>, generation: &AtomicU64) {
    if let Some(mut running) = child.take() {
        bump_generation(generation);
        let _ = running.start_kill();
        let _ = tokio::time::timeout(Duration::from_secs(5), running.wait()).await;
    }
}

fn bump_generation(generation: &AtomicU64) {
    generation.fetch_add(1, Ordering::Relaxed);
}

fn spawn_log_reader<R>(
    reader: R,
    logs: LogBuffer,
    stream: &'static str,
    log_file: std::path::PathBuf,
) where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
            .await
            .ok();
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let formatted = format!("[{stream}] {line}");
            if let Some(file) = file.as_mut() {
                let _ = file.write_all(formatted.as_bytes()).await;
                let _ = file.write_all(b"\n").await;
            }
            {
                let mut buffer = logs.lock().await;
                if buffer.len() >= MAX_LOG_LINES {
                    buffer.pop_front();
                }
                buffer.push_back(formatted);
            }
            if stream == "stderr" {
                tracing::warn!(target: "xray", "{}", line);
            } else {
                tracing::debug!(target: "xray", "{}", line);
            }
        }
    });
}

async fn append_log_line(path: &Path, line: &str) {
    if let Ok(mut file) = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await
    {
        let _ = file.write_all(line.as_bytes()).await;
        let _ = file.write_all(b"\n").await;
    }
}

async fn probe_version(cfg: &Config) -> Option<String> {
    if !cfg.xray_bin.exists() {
        return None;
    }
    let output = tokio::time::timeout(
        Duration::from_secs(5),
        Command::new(&cfg.xray_bin).arg("version").output(),
    )
    .await
    .ok()?
    .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .and_then(parse_version_line)
}

// `xray version` prints `Xray 26.10.31 (Xray, Penetrates Everything.) ...`;
// the UI wants just `26.10.31`, like the release tags without the leading v.
fn parse_version_line(line: &str) -> Option<String> {
    line.split_whitespace()
        .map(|token| token.trim_start_matches('v'))
        .find(|token| {
            !token.is_empty()
                && token.contains('.')
                && token.chars().all(|ch| ch.is_ascii_digit() || ch == '.')
                && token.chars().any(|ch| ch != '.')
        })
        .map(str::to_string)
}

/// Xray infers the config format from the extension, so temp files must still end in `.json`.
pub fn temp_config_path(path: &Path, marker: &str) -> std::path::PathBuf {
    let file = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config.json");
    path.with_file_name(format!("{file}.{marker}.json"))
}

/// Validate and atomically replace the config file. A failed test never touches the live file.
pub async fn write_config(cfg: &Config, config: &serde_json::Value) -> Result<()> {
    let path = &cfg.xray_config;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("create {}", parent.display()))?;
        }
    }

    let temp = temp_config_path(path, "tmp");
    let bytes = serde_json::to_vec_pretty(config).context("序列化 Xray 配置失败")?;
    tokio::fs::write(&temp, &bytes)
        .await
        .with_context(|| format!("write {}", temp.display()))?;

    if let Err(err) = test_config(cfg, &temp).await {
        let _ = tokio::fs::remove_file(&temp).await;
        return Err(err);
    }

    if tokio::fs::try_exists(path).await.unwrap_or(false) {
        let backup = path.with_extension("json.bak");
        let _ = tokio::fs::copy(path, &backup).await;
    }

    #[cfg(windows)]
    {
        let _ = tokio::fs::remove_file(path).await;
    }
    tokio::fs::rename(&temp, path)
        .await
        .with_context(|| format!("replace {}", path.display()))?;
    Ok(())
}

pub async fn test_config(cfg: &Config, path: &Path) -> Result<()> {
    if !cfg.xray_bin.exists() {
        bail!("找不到 Xray 可执行文件：{}", cfg.xray_bin.display());
    }
    let output = tokio::time::timeout(
        Duration::from_secs(15),
        Command::new(&cfg.xray_bin)
            .arg("run")
            .arg("-test")
            .arg("-c")
            .arg(path)
            .output(),
    )
    .await
    .context("Xray 配置校验超时")?
    .with_context(|| format!("run {} run -test", cfg.xray_bin.display()))?;

    if output.status.success() {
        return Ok(());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    bail!("Xray 配置校验失败：{}{}", stdout.trim(), stderr.trim())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{parse_version_line, splice_env_override, temp_config_path};

    #[test]
    fn version_line_keeps_only_the_short_version() {
        let line = "Xray 26.10.31 (Xray, Penetrates Everything.) a4a3dff (go1.27.3 windows/amd64)";
        assert_eq!(parse_version_line(line).as_deref(), Some("26.10.31"));
        assert_eq!(
            parse_version_line("Xray v25.3.6 (Xray, Penetrates Everything.)").as_deref(),
            Some("25.3.6")
        );
        assert_eq!(parse_version_line("unknown"), None);
    }

    #[test]
    fn splice_is_disabled_unless_the_operator_set_it() {
        assert_eq!(
            splice_env_override(|_| false),
            Some(("XRAY_BUF_SPLICE", "disable"))
        );
        assert_eq!(splice_env_override(|key| key == "XRAY_BUF_SPLICE"), None);
        assert_eq!(splice_env_override(|key| key == "xray.buf.splice"), None);
    }

    #[test]
    fn temp_config_path_keeps_json_extension() {
        let path = Path::new("/usr/local/etc/xray/config.json");
        let temp = temp_config_path(path, "test");
        assert_eq!(
            temp.file_name().and_then(|name| name.to_str()),
            Some("config.json.test.json")
        );
    }
}
