use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::{panel_log, service, state::AppState};

// Access logs can be large; read at most about a megabyte from the tail so a
// huge file never becomes a large allocation.
const MAX_TAIL_BYTES: u64 = 1024 * 1024;

pub fn panel_lines(state: &AppState, limit: usize) -> Vec<String> {
    panel_log::recent(&state.panel_logs, limit)
}

/// Resolve `log.error` / `log.access` from the generated Xray extra config and
/// return the lines currently in that file.
///
/// Xray resolves relative paths against its working directory, and the panel
/// starts it without changing the cwd, so `current_dir()` is the same base.
/// When the error log is not configured, fall back to the process output the
/// supervisor already captures.
pub async fn xray_payload(state: &AppState, file: &str, limit: usize) -> Value {
    let kind = if file.eq_ignore_ascii_case("access") {
        "access"
    } else {
        "error"
    };
    let extra = service::load_extra(state).await.unwrap_or(Value::Null);
    let raw = extra
        .get("log")
        .and_then(|log| log.get(kind))
        .and_then(Value::as_str);
    let path = resolve_log_path(raw);
    let configured = path.is_some();
    let mut exists = false;
    let mut fallback = false;

    let lines = match path.as_ref() {
        Some(path) => match tail_lines(path, limit).await {
            Some(lines) => {
                exists = true;
                lines
            }
            None => Vec::new(),
        },
        None if kind == "error" => {
            fallback = true;
            process_lines(state, limit).await
        }
        None => Vec::new(),
    };

    json!({
        "kind": kind,
        "configured": configured,
        "path": path.as_ref().map(|path| path.display().to_string()),
        "exists": exists,
        "fallback": fallback,
        "lines": lines,
    })
}

async fn process_lines(state: &AppState, limit: usize) -> Vec<String> {
    let buffer = state.xray_logs.lock().await;
    let mut lines: Vec<String> = buffer
        .iter()
        .filter(|line| line.starts_with("[stdout]") || line.starts_with("[stderr]"))
        .cloned()
        .collect();
    let skip = lines.len().saturating_sub(limit);
    lines.split_off(skip)
}

pub(crate) fn resolve_log_path(raw: Option<&str>) -> Option<PathBuf> {
    let raw = raw?.trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case("none") {
        return None;
    }
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        Some(path)
    } else {
        std::env::current_dir().ok().map(|cwd| cwd.join(path))
    }
}

/// Last `limit` lines of a file, reading backwards in 16 KiB chunks. Returns
/// `None` when the file cannot be opened (missing/unreadable), so callers can
/// distinguish "empty log" from "no log file".
pub(crate) async fn tail_lines(path: &Path, limit: usize) -> Option<Vec<String>> {
    use tokio::io::{AsyncReadExt, AsyncSeekExt};

    let mut file = tokio::fs::File::open(path).await.ok()?;
    let length = file.metadata().await.ok()?.len();
    if length == 0 {
        return Some(Vec::new());
    }

    const CHUNK: u64 = 16 * 1024;
    let mut position = length;
    let mut buffer: Vec<u8> = Vec::new();
    let mut newlines = 0usize;
    let mut consumed = 0u64;
    while position > 0 && newlines <= limit && consumed < MAX_TAIL_BYTES {
        let start = position.saturating_sub(CHUNK);
        let mut chunk = vec![0u8; (position - start) as usize];
        file.seek(std::io::SeekFrom::Start(start)).await.ok()?;
        file.read_exact(&mut chunk).await.ok()?;
        consumed += chunk.len() as u64;
        newlines += chunk.iter().filter(|byte| **byte == b'\n').count();
        chunk.extend_from_slice(&buffer);
        buffer = chunk;
        position = start;
    }

    let text = String::from_utf8_lossy(&buffer);
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let keep_from = lines.len().saturating_sub(limit);
    Some(lines.split_off(keep_from))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn log_path_handles_none_relative_and_absolute_values() {
        assert_eq!(resolve_log_path(None), None);
        assert_eq!(resolve_log_path(Some("")), None);
        assert_eq!(resolve_log_path(Some(" none ")), None);
        // Use a platform-absolute path so this also runs on the Windows dev
        // machine; production paths are Unix-style.
        let absolute = if cfg!(windows) {
            r"C:\xray\access.log"
        } else {
            "/var/log/xray/access.log"
        };
        assert_eq!(
            resolve_log_path(Some(absolute)),
            Some(PathBuf::from(absolute))
        );
        let relative = resolve_log_path(Some("./access.log")).expect("relative path");
        assert!(relative.is_absolute());
        assert!(relative.ends_with("access.log"));
    }

    #[tokio::test]
    async fn tail_returns_only_the_last_lines() {
        let path = std::env::temp_dir().join(format!(
            "xray-panel-tail-{}-{}.log",
            std::process::id(),
            "last"
        ));
        {
            let mut file = std::fs::File::create(&path).unwrap();
            for i in 0..10 {
                writeln!(file, "line {i}").unwrap();
            }
        }
        let lines = tail_lines(&path, 3).await.unwrap();
        assert_eq!(lines, vec!["line 7", "line 8", "line 9"]);
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn tail_missing_file_returns_none() {
        let path = std::env::temp_dir().join(format!(
            "xray-panel-tail-{}-{}.log",
            std::process::id(),
            "missing"
        ));
        let _ = std::fs::remove_file(&path);
        assert!(tail_lines(&path, 10).await.is_none());
    }
}
