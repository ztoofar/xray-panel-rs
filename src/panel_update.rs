use std::{
    io::{BufReader, Read},
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
const PANEL_ASSET: &str = "xray-panel-rs-linux-amd64-lumen.tar.gz";
const PANEL_ENTRY: &str = "xray-panel-rs-linux-amd64-lumen/xray-panel-rs";
const MAX_API_BYTES: u64 = 4 << 20;
const MAX_ARCHIVE_BYTES: u64 = 64 << 20;
const MAX_BINARY_BYTES: u64 = 64 << 20;
const MAX_EXTRACTED_BYTES: u64 = 192 << 20;
const API_TIMEOUT: Duration = Duration::from_secs(20);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug, Clone)]
struct Release {
    tag: String,
    version: String,
    html_url: String,
    published_at: String,
    archive_url: String,
    archive_digest: String,
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

fn proxy_label(proxy: Option<&str>) -> String {
    match proxy {
        Some(proxy) => format!("via update proxy {proxy}"),
        None => "directly (no update proxy configured)".into(),
    }
}

async fn latest_release(state: &AppState) -> Result<Release> {
    let proxy = updates::update_proxy(state).await;
    let label = proxy_label(proxy.as_deref());
    let proxy_hint = proxy.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<Release> {
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
        let archive = assets
            .iter()
            .find(|asset| asset.get("name").and_then(Value::as_str) == Some(PANEL_ASSET))
            .with_context(|| format!("latest release has no {PANEL_ASSET} asset"))?;
        let archive_url = archive
            .get("browser_download_url")
            .and_then(Value::as_str)
            .context("release asset has no download URL")?
            .to_string();
        // GitHub computes the digest for every uploaded asset; no separate
        // checksum asset is needed.
        let archive_digest = archive
            .get("digest")
            .and_then(Value::as_str)
            .and_then(|value| value.strip_prefix("sha256:"))
            .map(str::to_ascii_lowercase)
            .filter(|value| value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit()))
            .context("release asset has no sha256 digest")?;
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
            archive_url,
            archive_digest,
        })
    })
    .await
    .context("release lookup task failed")?;
    result
        .with_context(|| format!("query GitHub releases {label}"))
        .map_err(|err| updates::with_network_hint(err, proxy_hint.as_deref()))
}

pub async fn check(state: &AppState) -> Result<Value> {
    let release = latest_release(state).await?;
    Ok(release.to_json())
}

fn download_verified(agent: &ureq::Agent, release: &Release) -> Result<Vec<u8>> {
    let archive = updates::get_bytes(agent, &release.archive_url, MAX_ARCHIVE_BYTES)?;
    let got = hex::encode(Sha256::digest(&archive));
    if got != release.archive_digest {
        bail!(
            "panel archive SHA-256 mismatch (expected {}, got {got})",
            release.archive_digest
        );
    }
    extract_panel_binary(&archive)
}

fn extract_panel_binary(archive: &[u8]) -> Result<Vec<u8>> {
    let decoder = flate2::read::GzDecoder::new(archive);
    extract_from_tar(decoder)
}

fn tar_name(header: &[u8; 512]) -> String {
    let field = |start: usize, len: usize| {
        let raw = &header[start..start + len];
        let end = raw.iter().position(|byte| *byte == 0).unwrap_or(raw.len());
        String::from_utf8_lossy(&raw[..end]).trim().to_string()
    };
    let name = field(0, 100);
    let prefix = field(345, 155);
    if prefix.is_empty() {
        name
    } else {
        format!("{prefix}/{name}")
    }
}

fn tar_size(header: &[u8; 512]) -> Result<u64> {
    let raw = &header[124..136];
    let text = String::from_utf8_lossy(raw);
    let text = text.trim_matches(|c: char| c == '\0' || c == ' ');
    u64::from_str_radix(text, 8).context("parse tar entry size")
}

fn read_exact_limited<R: Read>(reader: &mut R, len: u64, budget: &mut u64) -> Result<Vec<u8>> {
    if len > MAX_BINARY_BYTES || len > *budget {
        bail!("archive binary exceeds the size limit");
    }
    *budget -= len;
    let mut bytes = vec![0u8; len as usize];
    reader.read_exact(&mut bytes).context("read tar entry")?;
    Ok(bytes)
}

fn skip_exact<R: Read>(reader: &mut R, len: u64, budget: &mut u64) -> Result<()> {
    if len > *budget {
        bail!("archive expands beyond the size limit");
    }
    *budget -= len;
    let skipped = std::io::copy(&mut reader.by_ref().take(len), &mut std::io::sink())
        .context("skip tar entry")?;
    if skipped != len {
        bail!("truncated tar archive");
    }
    Ok(())
}

fn pad_after(size: u64) -> u64 {
    (512 - size % 512) % 512
}

fn extract_from_tar<R: Read>(reader: R) -> Result<Vec<u8>> {
    let mut reader = BufReader::new(reader);
    let mut header = [0u8; 512];
    let mut budget = MAX_EXTRACTED_BYTES;
    loop {
        match reader.read_exact(&mut header) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
                bail!("tar archive ends before the panel binary");
            }
            Err(err) => return Err(err).context("read tar header"),
        }
        if header.iter().all(|byte| *byte == 0) {
            bail!("tar archive has no {PANEL_ENTRY}");
        }
        let name = tar_name(&header);
        let size = tar_size(&header)?;
        let typeflag = header[156];
        let regular = typeflag == b'0' || typeflag == 0;
        if regular && name == PANEL_ENTRY {
            let binary = read_exact_limited(&mut reader, size, &mut budget)?;
            if binary.len() < 4 || &binary[..4] != b"\x7fELF" {
                bail!("{PANEL_ENTRY} is not a Linux ELF binary");
            }
            return Ok(binary);
        }
        skip_exact(&mut reader, size, &mut budget)?;
        let padding = pad_after(size);
        if padding > 0 {
            skip_exact(&mut reader, padding, &mut budget)?;
        }
    }
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

// INVOCATION_ID is inherited by shells started from a systemd session, so it
// alone is not proof that this process is the xray-panel service. The unit sets
// PANEL_SYSTEMD=1; legacy units fall back to comparing systemd's MainPID.
fn running_under_systemd() -> bool {
    if std::env::var("PANEL_SYSTEMD").ok().as_deref() == Some("1") {
        return true;
    }
    if std::env::var_os("INVOCATION_ID").is_none() {
        return false;
    }
    let output = std::process::Command::new("systemctl")
        .args(["show", "-p", "MainPID", "--value", "xray-panel"])
        .output();
    match output {
        Ok(output) if output.status.success() => {
            let pid = String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse::<u32>()
                .ok();
            pid == Some(std::process::id())
        }
        _ => false,
    }
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
    let label = proxy_label(proxy.as_deref());
    let proxy_hint = proxy.clone();
    let target = current_binary()?;
    let release_for_download = release.clone();
    let bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        let agent = updates::agent(proxy.as_deref(), DOWNLOAD_TIMEOUT)?;
        download_verified(&agent, &release_for_download)
    })
    .await
    .context("panel download task failed")?
    .with_context(|| format!("download panel update {label}"))
    .map_err(|err| updates::with_network_hint(err, proxy_hint.as_deref()))?;

    let temp = stage_binary(&target, &bytes)?;
    replace_binary(&temp, &target)?;
    tracing::info!(
        version = %release.version,
        target = %target.display(),
        "panel binary updated"
    );

    let systemd = running_under_systemd();
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
    use std::io::Write;

    use flate2::{write::GzEncoder, Compression};

    use super::*;

    fn tar_header(name: &str, size: usize, typeflag: u8) -> [u8; 512] {
        let mut header = [0u8; 512];
        header[..name.len()].copy_from_slice(name.as_bytes());
        header[100..108].copy_from_slice(b"0000755\0");
        header[108..116].copy_from_slice(b"0000000\0");
        header[116..124].copy_from_slice(b"0000000\0");
        header[124..136].copy_from_slice(format!("{size:011o}\0").as_bytes());
        header[136..148].copy_from_slice(b"00000000000\0");
        header[156] = typeflag;
        header
    }

    fn tar_entry(tar: &mut Vec<u8>, name: &str, data: &[u8], typeflag: u8) {
        tar.extend_from_slice(&tar_header(name, data.len(), typeflag));
        tar.extend_from_slice(data);
        let padding = pad_after(data.len() as u64);
        tar.extend(std::iter::repeat_n(0u8, padding as usize));
    }

    fn gzip(archive: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(archive).expect("gzip");
        encoder.finish().expect("gzip finish")
    }

    #[test]
    fn extracts_the_panel_binary_from_the_release_tarball() {
        let mut tar = Vec::new();
        tar_entry(&mut tar, "xray-panel-rs-linux-amd64-lumen/", &[], b'5');
        tar_entry(
            &mut tar,
            "xray-panel-rs-linux-amd64-lumen/README.md",
            b"readme",
            b'0',
        );
        tar_entry(&mut tar, PANEL_ENTRY, b"\x7fELF-panel", b'0');
        tar.extend(std::iter::repeat_n(0u8, 512));

        let binary = extract_panel_binary(&gzip(&tar)).expect("extract");
        assert_eq!(binary, b"\x7fELF-panel");
    }

    #[test]
    fn rejects_archives_without_the_panel_binary() {
        let mut tar = Vec::new();
        tar_entry(&mut tar, "other/file", b"data", b'0');
        tar.extend(std::iter::repeat_n(0u8, 512));
        let err = extract_panel_binary(&gzip(&tar)).expect_err("missing binary");
        assert!(format!("{err:#}").contains("has no"), "{err:#}");
    }

    #[test]
    fn rejects_non_elf_payloads() {
        let mut tar = Vec::new();
        tar_entry(&mut tar, PANEL_ENTRY, b"not-an-elf", b'0');
        let err = extract_panel_binary(&gzip(&tar)).expect_err("non-elf");
        assert!(format!("{err:#}").contains("not a Linux ELF"), "{err:#}");
    }

    #[test]
    fn version_comparison_ignores_the_v_prefix_and_prerelease_suffix() {
        assert!(is_newer("v0.2.16", "0.2.15"));
        assert!(is_newer("0.3.0", "0.2.99"));
        assert!(!is_newer("v0.2.15", "0.2.15"));
        assert!(!is_newer("0.2.14", "0.2.15"));
        assert!(!is_newer("not-a-version", "0.2.15"));
    }
}
