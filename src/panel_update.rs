use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{state::AppState, updates};

// Set by the release workflow so forks self-update from their own releases.
pub const PANEL_REPO: &str = match option_env!("PANEL_GITHUB_REPO") {
    Some(repo) => repo,
    None => "ztoofar/xray-panel-rs",
};

const PANEL_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_API_BYTES: u64 = 4 << 20;
const MAX_BINARY_BYTES: u64 = 64 << 20;
const MAX_SUMS_BYTES: u64 = 64 << 10;
const API_TIMEOUT: Duration = Duration::from_secs(20);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug, Clone)]
struct Release {
    tag: String,
    version: String,
    html_url: String,
    published_at: String,
    binary_url: String,
    sums_url: String,
}

impl Release {
    fn to_json(&self) -> Value {
        json!({
            "current": PANEL_VERSION,
            "latest": self.version,
            "tag": self.tag,
            "updateAvailable": is_newer(&self.version, PANEL_VERSION),
            "releaseUrl": self.html_url,
            "publishedAt": self.published_at,
            "repo": PANEL_REPO,
        })
    }
}

fn parse_version(raw: &str) -> Option<(u64, u64, u64)> {
    let raw = raw.trim().trim_start_matches('v');
    let mut parts = raw.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.split('-').next()?.parse().ok()?;
    Some((major, minor, patch))
}

fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(candidate), Some(current)) => candidate > current,
        _ => false,
    }
}

async fn latest_release(state: &AppState) -> Result<Release> {
    let proxy = updates::update_proxy(state).await;
    tokio::task::spawn_blocking(move || -> Result<Release> {
        let agent = updates::agent(proxy.as_deref(), API_TIMEOUT)?;
        let url = format!("https://api.github.com/repos/{PANEL_REPO}/releases/latest");
        let body = updates::get_bytes(&agent, &url, MAX_API_BYTES)?;
        let release: Value = serde_json::from_slice(&body).context("parse GitHub release")?;
        let tag = release
            .get("tag_name")
            .and_then(Value::as_str)
            .context("release has no tag_name")?
            .to_string();
        let version = tag.trim_start_matches('v').to_string();
        if parse_version(&version).is_none() {
            bail!("unexpected release tag: {tag}");
        }
        let assets = release
            .get("assets")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let asset_url = |name: &str| {
            assets
                .iter()
                .find(|asset| asset.get("name").and_then(Value::as_str) == Some(name))
                .and_then(|asset| asset.get("browser_download_url"))
                .and_then(Value::as_str)
                .map(str::to_string)
        };
        Ok(Release {
            tag,
            version,
            html_url: release
                .get("html_url")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            published_at: release
                .get("published_at")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            binary_url: asset_url("xray-panel-rs")
                .context("latest release has no xray-panel-rs binary asset")?,
            sums_url: asset_url("SHA256SUMS").context("latest release has no SHA256SUMS asset")?,
        })
    })
    .await
    .context("release lookup task failed")?
}

pub async fn check(state: &AppState) -> Result<Value> {
    let release = latest_release(state).await?;
    Ok(release.to_json())
}

fn download_verified(agent: &ureq::Agent, release: &Release) -> Result<Vec<u8>> {
    let sums = updates::get_bytes(agent, &release.sums_url, MAX_SUMS_BYTES)?;
    let expected = updates::parse_sha256sum(&sums, "xray-panel-rs")?;
    let binary = updates::get_bytes(agent, &release.binary_url, MAX_BINARY_BYTES)?;
    let got = hex::encode(Sha256::digest(&binary));
    if got != expected {
        bail!("panel binary SHA-256 mismatch (expected {expected}, got {got})");
    }
    Ok(binary)
}

fn current_binary() -> Result<PathBuf> {
    let path = std::env::current_exe().context("resolve current executable")?;
    let path = path.canonicalize().unwrap_or(path);
    if !path.exists() {
        bail!("current executable not found: {}", path.display());
    }
    Ok(path)
}

fn stage_binary(target: &Path, bytes: &[u8]) -> Result<PathBuf> {
    let parent = target
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let temp = parent.join(format!(
        ".{}.update-{}",
        target
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("xray-panel-rs"),
        uuid::Uuid::new_v4()
    ));
    let result = (|| -> Result<()> {
        {
            use std::io::Write;
            let mut file = std::fs::File::create(&temp).context("create staged panel binary")?;
            file.write_all(bytes).context("write staged panel binary")?;
            file.sync_all().context("sync staged panel binary")?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o755))
                .context("chmod staged panel binary")?;
        }
        Ok(())
    })();
    if let Err(err) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(err);
    }
    Ok(temp)
}

fn replace_binary(temp: &Path, target: &Path) -> Result<()> {
    let backup = target.with_extension("bak");
    std::fs::copy(target, &backup).with_context(|| {
        let _ = std::fs::remove_file(temp);
        format!("back up current panel binary to {}", backup.display())
    })?;
    std::fs::rename(temp, target).with_context(|| {
        let _ = std::fs::remove_file(temp);
        format!("replace {}", target.display())
    })?;
    Ok(())
}

pub async fn install(state: &AppState) -> Result<Value> {
    if !cfg!(unix) {
        bail!("online panel update is supported on Linux only");
    }
    let release = latest_release(state).await?;
    if !is_newer(&release.version, PANEL_VERSION) {
        bail!("already running the latest version ({PANEL_VERSION})");
    }
    let proxy = updates::update_proxy(state).await;
    let target = current_binary()?;
    let release_for_download = release.clone();
    let bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        let agent = updates::agent(proxy.as_deref(), DOWNLOAD_TIMEOUT)?;
        download_verified(&agent, &release_for_download)
    })
    .await
    .context("panel download task failed")??;

    let temp = stage_binary(&target, &bytes)?;
    replace_binary(&temp, &target)?;
    tracing::info!(
        version = %release.version,
        target = %target.display(),
        "panel binary updated"
    );

    let systemd = std::env::var_os("INVOCATION_ID").is_some();
    if systemd {
        tokio::spawn(async {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            // Restart=on-failure brings the new binary back.
            std::process::exit(1);
        });
    }
    Ok(json!({
        "current": PANEL_VERSION,
        "latest": release.version,
        "restarting": systemd,
        "restartRequired": !systemd,
        "releaseUrl": release.html_url,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison_ignores_the_v_prefix_and_prerelease_suffix() {
        assert!(is_newer("v0.2.16", "0.2.15"));
        assert!(is_newer("0.3.0", "0.2.99"));
        assert!(!is_newer("v0.2.15", "0.2.15"));
        assert!(!is_newer("0.2.14", "0.2.15"));
        assert!(!is_newer("not-a-version", "0.2.15"));
    }
}
