use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{db, state::AppState};

const XRAY_RELEASES_API: &str = "https://api.github.com/repos/XTLS/Xray-core/releases?per_page=30";
const MAX_ARCHIVE_BYTES: u64 = 200 << 20;
const MAX_GEOFILES_BYTES: u64 = 64 << 20;
const MAX_DIGEST_BYTES: u64 = 64 << 10;
const API_TIMEOUT: Duration = Duration::from_secs(30);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(180);

// file, latest download URL, asset name inside the release.
const GEOFILES: [(&str, &str, &str); 6] = [
    (
        "geoip.dat",
        "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/geoip.dat",
        "geoip.dat",
    ),
    (
        "geosite.dat",
        "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/geosite.dat",
        "geosite.dat",
    ),
    (
        "geoip_IR.dat",
        "https://github.com/chocolate4u/Iran-v2ray-rules/releases/latest/download/geoip.dat",
        "geoip.dat",
    ),
    (
        "geosite_IR.dat",
        "https://github.com/chocolate4u/Iran-v2ray-rules/releases/latest/download/geosite.dat",
        "geosite.dat",
    ),
    (
        "geoip_RU.dat",
        "https://github.com/runetfreedom/russia-v2ray-rules-dat/releases/latest/download/geoip.dat",
        "geoip.dat",
    ),
    (
        "geosite_RU.dat",
        "https://github.com/runetfreedom/russia-v2ray-rules-dat/releases/latest/download/geosite.dat",
        "geosite.dat",
    ),
];

pub async fn update_proxy(state: &AppState) -> Option<String> {
    db::settings::ui_settings(&state.db)
        .await
        .ok()
        .and_then(|settings| {
            settings
                .get("updateProxy")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
}

pub(crate) fn agent(proxy: Option<&str>, timeout: Duration) -> Result<ureq::Agent> {
    let mut builder = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .max_redirects(10)
        .user_agent("lumen-panel-updater")
        .http_status_as_error(false);
    if let Some(proxy) = proxy {
        let parsed =
            ureq::Proxy::new(proxy).with_context(|| format!("invalid update proxy: {proxy}"))?;
        builder = builder.proxy(Some(parsed));
    }
    Ok(builder.build().into())
}

fn read_limited(response: &mut ureq::http::Response<ureq::Body>, limit: u64) -> Result<Vec<u8>> {
    response
        .body_mut()
        .with_config()
        .limit(limit)
        .read_to_vec()
        .context("读取响应内容失败")
}

pub(crate) fn get_bytes(agent: &ureq::Agent, url: &str, limit: u64) -> Result<Vec<u8>> {
    let mut response = agent
        .get(url)
        .call()
        .with_context(|| format!("fetch {url}"))?;
    let status = response.status().as_u16();
    if status != 200 {
        bail!("{url} 返回 HTTP {status}");
    }
    read_limited(&mut response, limit).with_context(|| format!("read {url}"))
}

fn io_kind_is_transport(kind: std::io::ErrorKind) -> bool {
    use std::io::ErrorKind;
    matches!(
        kind,
        ErrorKind::ConnectionRefused
            | ErrorKind::ConnectionReset
            | ErrorKind::ConnectionAborted
            | ErrorKind::TimedOut
            | ErrorKind::NotConnected
            | ErrorKind::NetworkUnreachable
            | ErrorKind::HostUnreachable
            | ErrorKind::AddrNotAvailable
    )
}

pub(crate) fn is_transport_failure(err: &anyhow::Error) -> bool {
    let typed = err.chain().any(|cause| {
        if let Some(io) = cause.downcast_ref::<std::io::Error>() {
            return io_kind_is_transport(io.kind());
        }
        if let Some(ureq_err) = cause.downcast_ref::<ureq::Error>() {
            return match ureq_err {
                ureq::Error::Io(io) => io_kind_is_transport(io.kind()),
                ureq::Error::Timeout(_)
                | ureq::Error::HostNotFound
                | ureq::Error::ConnectionFailed
                | ureq::Error::ConnectProxyFailed(_) => true,
                _ => false,
            };
        }
        false
    });
    if typed {
        return true;
    }
    // Windows error text is localized and older platforms may not map every
    // socket error to a kind; keep a narrow textual fallback.
    let text = format!("{err:#}").to_ascii_lowercase();
    [
        "连接被拒绝",
        "os error 111",
        "os error 10061",
        "connection reset",
        "connection timed out",
        "timed out",
        "dns error",
        "failed to lookup address",
        "network is unreachable",
        "no route to host",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

pub(crate) fn proxy_unavailable_message(proxy: &str) -> String {
    format!(
        "代理不可用：无法连接 {proxy}。请确认代理已启动、端口正确，且允许面板服务器访问；\
         127.0.0.1 指面板服务器自身，不是你的电脑。可在面板设置中修改或清空代理。"
    )
}

/// Chinese, user-facing message for transport failures. When a proxy is
/// configured the panel can only reach the internet through it, so refused
/// connections and timeouts almost always mean the proxy is down.
pub(crate) fn network_hint(err: &anyhow::Error, proxy: Option<&str>) -> Option<String> {
    if !is_transport_failure(err) {
        return None;
    }
    Some(match proxy {
        Some(proxy) => proxy_unavailable_message(proxy),
        None => "无法连接 GitHub：请检查面板服务器的网络和 DNS，或配置更新代理后重试。".to_string(),
    })
}

/// Replaces a transport error with a Chinese message for the UI. The original
/// error chain (including the native io/ureq detail) goes to the log only.
pub(crate) fn with_network_hint(err: anyhow::Error, proxy: Option<&str>) -> anyhow::Error {
    match network_hint(&err, proxy) {
        Some(message) => {
            tracing::warn!(error = %format!("{err:#}"), "network request failed");
            anyhow::anyhow!(message)
        }
        None => err,
    }
}

/// Fetch the Xray release tags published on GitHub, newest first. Tags keep
/// their leading `v` because that is what the download URL expects.
pub async fn xray_versions(state: &AppState) -> Result<Vec<String>> {
    let proxy = update_proxy(state).await;
    let proxy_hint = proxy.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<Vec<String>> {
        let agent = agent(proxy.as_deref(), API_TIMEOUT)?;
        let mut response = agent
            .get(XRAY_RELEASES_API)
            .header("Accept", "application/vnd.github+json")
            .call()
            .context("获取 Xray 版本列表失败")?;
        if response.status().as_u16() != 200 {
            bail!("GitHub Release 接口返回 HTTP {}", response.status());
        }
        let body = read_limited(&mut response, 16 << 20)?;
        let releases: Vec<Value> =
            serde_json::from_slice(&body).context("解析 Xray 版本列表失败")?;
        let mut versions = Vec::new();
        for release in releases {
            let Some(tag) = release.get("tag_name").and_then(Value::as_str) else {
                continue;
            };
            if version_at_least(tag, 26, 6, 27) {
                versions.push(tag.to_string());
            }
        }
        Ok(versions)
    })
    .await
    .context("版本查询任务失败")?;
    result.map_err(|err| with_network_hint(err, proxy_hint.as_deref()))
}

fn version_at_least(tag: &str, major: u64, minor: u64, patch: u64) -> bool {
    let version = tag.trim_start_matches('v');
    let mut parts = version.split('.');
    let (Some(ma), Some(mi), Some(pa)) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let (Ok(ma), Ok(mi), Ok(pa)) = (ma.parse::<u64>(), mi.parse::<u64>(), pa.parse::<u64>()) else {
        return false;
    };
    (ma, mi, pa) >= (major, minor, patch)
}

fn xray_asset_name() -> Result<&'static str> {
    let os = match std::env::consts::OS {
        "linux" => "linux",
        "windows" => "windows",
        "macos" => "macos",
        other => bail!("不支持的操作系统：{other}"),
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "64",
        "aarch64" => "arm64-v8a",
        "x86" => "32",
        other => bail!("不支持的架构：{other}"),
    };
    Ok(match (os, arch) {
        ("linux", "64") => "Xray-linux-64.zip",
        ("linux", "arm64-v8a") => "Xray-linux-arm64-v8a.zip",
        ("linux", "32") => "Xray-linux-32.zip",
        ("windows", "64") => "Xray-windows-64.zip",
        ("windows", "32") => "Xray-windows-32.zip",
        ("macos", "64") => "Xray-macos-64.zip",
        ("macos", "arm64-v8a") => "Xray-macos-arm64-v8a.zip",
        _ => bail!("不支持的 Xray 平台"),
    })
}

fn parse_dgst(raw: &[u8]) -> Result<String> {
    for line in String::from_utf8_lossy(raw).lines() {
        if let Some(rest) = line.trim().strip_prefix("SHA2-256=") {
            let digest = rest.trim().to_ascii_lowercase();
            if digest.len() == 64 && digest.chars().all(|c| c.is_ascii_hexdigit()) {
                return Ok(digest);
            }
            bail!("Xray SHA2-256 摘要格式错误");
        }
    }
    bail!("Xray 摘要中没有 SHA2-256 条目")
}

pub async fn install_xray(state: &AppState, version: &str) -> Result<()> {
    let versions = xray_versions(state).await?;
    if !versions.iter().any(|tag| tag == version) {
        bail!("Xray 版本 {version} 不在发布列表中");
    }
    let asset = xray_asset_name()?;
    let url = format!("https://github.com/XTLS/Xray-core/releases/download/{version}/{asset}");
    let proxy = update_proxy(state).await;
    let proxy_hint = proxy.clone();
    let target = state.cfg.xray_bin.clone();
    let bin_dir = target
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    // Download before stopping Xray: the update proxy may be one of this
    // Xray's own inbounds, which would disappear with the process.
    let download = tokio::task::spawn_blocking(move || -> Result<PathBuf> {
        let agent = agent(proxy.as_deref(), DOWNLOAD_TIMEOUT)?;
        let archive = get_bytes(&agent, &url, MAX_ARCHIVE_BYTES)?;
        let digest_url = format!("{url}.dgst");
        let digest = parse_dgst(&get_bytes(&agent, &digest_url, MAX_DIGEST_BYTES)?)?;
        let got = hex::encode(Sha256::digest(&archive));
        if got != digest {
            bail!("Xray 压缩包 SHA-256 校验失败（期望 {digest}，实际 {got}）");
        }
        extract_xray_temp(&archive, &bin_dir)
    })
    .await;

    let staged = match download {
        Ok(Ok(path)) => path,
        Ok(Err(err)) => {
            return Err(with_network_hint(
                err.context("下载任务失败"),
                proxy_hint.as_deref(),
            ))
        }
        Err(err) => bail!("下载任务失败：{err}"),
    };

    state.xray.stop().await;
    if let Err(err) = replace_xray(&staged, &target) {
        let _ = std::fs::remove_file(&staged);
        if let Err(restart) = state.xray.restart().await {
            tracing::warn!(error = %restart, "xray restart after failed update failed");
        }
        return Err(err);
    }
    if let Err(err) = state.xray.restart().await {
        bail!("Xray 已安装但重启失败：{err}");
    }
    tracing::info!(version, "xray binary updated");
    Ok(())
}

fn extract_xray_temp(archive: &[u8], bin_dir: &Path) -> Result<PathBuf> {
    let cursor = std::io::Cursor::new(archive);
    let mut zip = zip::ZipArchive::new(cursor).context("打开 Xray 压缩包失败")?;
    let entry_name = if cfg!(windows) { "xray.exe" } else { "xray" };
    let mut entry = zip
        .by_name(entry_name)
        .with_context(|| format!("archive has no {entry_name}"))?;
    if entry.size() > MAX_ARCHIVE_BYTES {
        bail!("Xray 二进制超过大小限制");
    }
    std::fs::create_dir_all(bin_dir).ok();
    let temp = bin_dir.join(format!(".xray-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut output = std::fs::File::create(&temp).context("创建临时 Xray 二进制失败")?;
        let copied = std::io::copy(&mut entry.by_ref().take(MAX_ARCHIVE_BYTES + 1), &mut output)
            .context("解压 Xray 二进制失败")?;
        output.flush().ok();
        drop(output);
        if copied > MAX_ARCHIVE_BYTES {
            bail!("Xray 二进制超过大小限制");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o755))
                .context("设置 Xray 二进制权限失败")?;
        }
        Ok(())
    })();
    if let Err(err) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(err);
    }
    Ok(temp)
}

fn replace_xray(temp: &Path, target: &Path) -> Result<()> {
    #[cfg(windows)]
    if target.exists() {
        let _ = std::fs::remove_file(target);
    }
    std::fs::rename(temp, target).with_context(|| {
        let _ = std::fs::remove_file(temp);
        format!("replace {}", target.display())
    })?;
    Ok(())
}

/// Standard geodata assets offered by the Xray page. Xray's native `geodata`
/// block downloads these through its configured outbound.
pub fn standard_geodata_sources() -> Value {
    let mut list: Vec<Value> = GEOFILES
        .iter()
        .map(|(file, url, _)| json!({ "url": url, "file": file }))
        .collect();
    list.sort_by(|a, b| {
        a.get("file")
            .and_then(Value::as_str)
            .cmp(&b.get("file").and_then(Value::as_str))
    });
    Value::Array(list)
}

pub(crate) fn parse_sha256sum(raw: &[u8], asset: &str) -> Result<String> {
    for line in String::from_utf8_lossy(raw).lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() != 2 {
            continue;
        }
        let name = Path::new(fields[1].trim_start_matches('*'))
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if name != asset {
            continue;
        }
        let digest = fields[0].to_ascii_lowercase();
        if digest.len() == 64 && digest.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok(digest);
        }
        bail!("{asset} 的 SHA-256 条目格式错误");
    }
    bail!("校验文件缺少 {asset} 条目")
}

pub async fn update_geofile(state: &AppState, only: Option<&str>) -> Result<usize> {
    let wanted: Vec<(&str, &str, &str)> = match only {
        Some(name) => GEOFILES
            .iter()
            .filter(|(file, _, _)| *file == name)
            .copied()
            .collect(),
        None => GEOFILES.to_vec(),
    };
    if wanted.is_empty() {
        bail!("未知的 Geofile：{only:?}");
    }
    let proxy = update_proxy(state).await;
    let proxy_hint = proxy.clone();
    // Xray resolves geodata from XRAY_LOCATION_ASSET when set; otherwise the
    // conventional location is the binary's own folder.
    let bin_dir = std::env::var("XRAY_LOCATION_ASSET")
        .ok()
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .unwrap_or_else(|| {
            state
                .cfg
                .xray_bin
                .parent()
                .filter(|path| !path.as_os_str().is_empty())
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."))
        });
    let result = tokio::task::spawn_blocking(move || -> Result<usize> {
        let agent = agent(proxy.as_deref(), DOWNLOAD_TIMEOUT)?;
        let mut installed = 0;
        let mut errors = Vec::new();
        for (file, url, asset) in wanted {
            match install_one_geofile(&agent, &bin_dir, file, url, asset) {
                Ok(()) => installed += 1,
                Err(err) => errors.push(format!("{file}: {err:#}")),
            }
        }
        if !errors.is_empty() {
            bail!("{}", errors.join("; "));
        }
        Ok(installed)
    })
    .await
    .context("Geofile 任务失败")?;
    let installed = result.map_err(|err| with_network_hint(err, proxy_hint.as_deref()))?;
    if installed > 0 {
        state
            .xray
            .restart()
            .await
            .map_err(|err| anyhow::anyhow!("更新 Geofile 后重启 Xray 失败：{err}"))?;
        tracing::info!(installed, "geofiles updated");
    }
    Ok(installed)
}

fn install_one_geofile(
    agent: &ureq::Agent,
    bin_dir: &Path,
    file: &str,
    url: &str,
    asset: &str,
) -> Result<()> {
    let bytes = get_bytes(agent, url, MAX_GEOFILES_BYTES)?;
    let sums_url = format!("{url}.sha256sum");
    let sums = get_bytes(agent, &sums_url, MAX_DIGEST_BYTES)?;
    let expected = parse_sha256sum(&sums, asset)?;
    let got = hex::encode(Sha256::digest(&bytes));
    if got != expected {
        bail!("SHA-256 校验失败（期望 {expected}，实际 {got}）");
    }
    std::fs::create_dir_all(bin_dir).ok();
    let temp = bin_dir.join(format!(".{file}.{}.tmp", uuid::Uuid::new_v4()));
    std::fs::write(&temp, &bytes).with_context(|| format!("write {}", temp.display()))?;
    #[cfg(windows)]
    {
        let target = bin_dir.join(file);
        if target.exists() {
            let _ = std::fs::remove_file(&target);
        }
    }
    std::fs::rename(&temp, bin_dir.join(file))
        .with_context(|| format!("install {file} into {}", bin_dir.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_filter_matches_the_supported_floor() {
        assert!(version_at_least("v26.6.27", 26, 6, 27));
        assert!(version_at_least("v26.7.0", 26, 6, 27));
        assert!(version_at_least("v27.0.1", 26, 6, 27));
        assert!(!version_at_least("v26.6.26", 26, 6, 27));
        assert!(!version_at_least("v25.12.0", 26, 6, 27));
        assert!(!version_at_least("nightly", 26, 6, 27));
    }

    #[test]
    fn transport_errors_use_chinese_proxy_messages() {
        let err = anyhow::anyhow!(
            "fetch https://api.github.com/repos/x/releases/latest: io: Connection refused (os error 111)"
        );
        let hint = network_hint(&err, Some("http://127.0.0.1:10813")).unwrap();
        assert!(hint.contains("代理不可用"));
        assert!(hint.contains("http://127.0.0.1:10813"));
        assert!(hint.contains("面板服务器自身"));

        let direct = network_hint(&err, None).unwrap();
        assert!(direct.contains("无法连接 GitHub"));
    }

    #[test]
    fn network_hint_hides_the_native_error() {
        let err = anyhow::anyhow!(
            "fetch https://api.github.com/repos/x/releases/latest: io: Connection refused (os error 111)"
        );
        let message = format!(
            "{:#}",
            with_network_hint(err, Some("http://127.0.0.1:10813"))
        );
        assert!(message.contains("代理不可用"));
        assert!(!message.contains("Connection refused"));
        assert!(!message.contains("os error"));
    }

    #[test]
    fn parsing_errors_get_no_network_hint() {
        let err = anyhow::anyhow!("解析 GitHub Release 失败：数据格式错误");
        assert!(network_hint(&err, Some("http://127.0.0.1:10813")).is_none());
    }

    #[test]
    fn typed_io_errors_do_not_depend_on_localized_text() {
        let io = std::io::Error::from(std::io::ErrorKind::ConnectionRefused);
        let err = anyhow::Error::new(ureq::Error::Io(io));
        let hint = network_hint(&err, Some("http://127.0.0.1:10813")).unwrap();
        assert!(hint.contains("代理不可用"));
    }

    #[test]
    fn parses_the_published_digest_sidecar() {
        let raw = b"MD5= abcdef\nSHA2-256= ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789\n";
        assert_eq!(
            parse_dgst(raw).unwrap(),
            "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
        );
        assert!(parse_dgst(b"MD5= abc").is_err());
    }

    #[test]
    fn parses_geofile_checksums_by_asset_basename() {
        let raw = b"deadbeef  release/other.dat\n0123456789012345678901234567890123456789012345678901234567890123 *geoip.dat\n";
        assert_eq!(
            parse_sha256sum(raw, "geoip.dat").unwrap(),
            "0123456789012345678901234567890123456789012345678901234567890123"
        );
        assert!(parse_sha256sum(b"bad", "geoip.dat").is_err());
    }

    #[test]
    fn standard_sources_cover_every_allowlisted_file() {
        let sources = standard_geodata_sources();
        let files: Vec<&str> = sources
            .as_array()
            .unwrap()
            .iter()
            .map(|source| source["file"].as_str().unwrap())
            .collect();
        assert_eq!(files.len(), GEOFILES.len());
        assert!(files.contains(&"geoip.dat"));
        assert!(files.contains(&"geosite_RU.dat"));
    }

    #[test]
    fn extracts_the_xray_binary_from_a_release_zip() {
        let dir = std::env::temp_dir().join(format!("lumen-zip-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let zip_path = dir.join("xray.zip");
        let entry = if cfg!(windows) { "xray.exe" } else { "xray" };
        {
            let file = std::fs::File::create(&zip_path).unwrap();
            let mut writer = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            writer.start_file(entry, options).unwrap();
            writer.write_all(b"fake-xray-binary").unwrap();
            writer.finish().unwrap();
        }
        let archive = std::fs::read(&zip_path).unwrap();
        let target = dir.join(entry);
        let staged = extract_xray_temp(&archive, &dir).unwrap();
        assert!(!target.exists());
        replace_xray(&staged, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"fake-xray-binary");
        assert!(!staged.exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
