//! Panel logging: an in-memory ring buffer for the live view plus daily files
//! for history.
//!
//! Xray's stdout/stderr already has a buffer in the supervisor; this module
//! mirrors the panel's tracing stream so both can be served without touching
//! journald or adding a log-file dependency.

use std::{
    collections::VecDeque,
    fs::{File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use chrono::{FixedOffset, NaiveDate, Utc};
use serde::Serialize;
use tracing_subscriber::fmt::{format::Writer, time::FormatTime};

pub type PanelLogBuffer = Arc<Mutex<VecDeque<String>>>;

const MAX_LINES: usize = 500;
const LOG_FILE_PREFIX: &str = "panel-";
const RETENTION_DAYS: i64 = 14;

pub fn buffer() -> PanelLogBuffer {
    Arc::new(Mutex::new(VecDeque::with_capacity(MAX_LINES)))
}

pub fn recent(buffer: &PanelLogBuffer, limit: usize) -> Vec<String> {
    let Ok(queue) = buffer.lock() else {
        return Vec::new();
    };
    let skip = queue.len().saturating_sub(limit);
    queue.iter().skip(skip).cloned().collect()
}

fn push(buffer: &PanelLogBuffer, line: &str) {
    if let Ok(mut queue) = buffer.lock() {
        if queue.len() >= MAX_LINES {
            queue.pop_front();
        }
        queue.push_back(line.to_string());
    }
}

// The panel reports Chinese wall-clock time regardless of the host timezone;
// operators almost always read these logs against China-time schedules.
fn china_offset() -> FixedOffset {
    FixedOffset::east_opt(8 * 3600).expect("UTC+8 is valid")
}

pub fn format_china_time(utc: chrono::DateTime<Utc>) -> String {
    utc.with_timezone(&china_offset())
        .format("%Y-%m-%d %H:%M:%S%.3f +08:00")
        .to_string()
}

#[derive(Clone, Copy, Debug)]
pub struct ChinaTime;

impl FormatTime for ChinaTime {
    fn format_time(&self, writer: &mut Writer<'_>) -> std::fmt::Result {
        write!(writer, "{}", format_china_time(Utc::now()))
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelLogFile {
    pub name: String,
    pub date: String,
    pub size: u64,
}

fn parse_log_date(name: &str) -> Option<NaiveDate> {
    parse_file_date(name, LOG_FILE_PREFIX)
}

fn parse_file_date(name: &str, prefix: &str) -> Option<NaiveDate> {
    let raw = name.strip_prefix(prefix)?.strip_suffix(".log")?;
    NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok()
}

/// Only names produced by this module are accepted, so the history viewer
/// cannot be used to read arbitrary files.
pub fn resolve_log_file(dir: &Path, name: &str) -> Option<PathBuf> {
    parse_log_date(name)?;
    Some(dir.join(name))
}

pub fn list_log_files(dir: &Path) -> Vec<PanelLogFile> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PanelLogFile> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            let date = parse_log_date(&name)?;
            let size = entry.metadata().ok()?.len();
            Some(PanelLogFile {
                name,
                date: date.to_string(),
                size,
            })
        })
        .collect();
    files.sort_by(|left, right| right.name.cmp(&left.name));
    files
}

fn prune_old_files(dir: &Path, prefix: &str) {
    let cutoff = Utc::now().with_timezone(&china_offset()).date_naive()
        - chrono::Duration::days(RETENTION_DAYS);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(date) = parse_file_date(&name, prefix) {
            if date < cutoff {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

struct OpenLogFile {
    date: String,
    file: File,
}

pub struct PanelFileLog {
    dir: PathBuf,
    prefix: &'static str,
    state: Mutex<Option<OpenLogFile>>,
}

impl PanelFileLog {
    pub fn new(dir: PathBuf) -> Self {
        Self::with_prefix(dir, LOG_FILE_PREFIX)
    }

    /// Same daily rotation and retention as the panel log, but with a distinct
    /// file prefix (the memory sampler writes `memory-YYYY-MM-DD.log`).
    pub fn with_prefix(dir: PathBuf, prefix: &'static str) -> Self {
        Self {
            dir,
            prefix,
            state: Mutex::new(None),
        }
    }

    pub fn write_line(&self, line: &str) {
        let date = Utc::now()
            .with_timezone(&china_offset())
            .format("%Y-%m-%d")
            .to_string();
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let rotated = guard.as_ref().is_none_or(|current| current.date != date);
        if rotated {
            if std::fs::create_dir_all(&self.dir).is_err() {
                return;
            }
            let path = self.dir.join(format!("{}{date}.log", self.prefix));
            let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) else {
                return;
            };
            *guard = Some(OpenLogFile { date, file });
        }
        if let Some(current) = guard.as_mut() {
            let _ = writeln!(current.file, "{line}");
        }
        drop(guard);
        if rotated {
            prune_old_files(&self.dir, self.prefix);
        }
    }
}

/// `MakeWriter` for a tracing `fmt` layer. Each event gets its own writer and
/// only reaches the shared buffer when dropped, so two events formatting at
/// the same time cannot interleave halfway through a line.
#[derive(Clone)]
pub struct PanelLogMakeWriter {
    buffer: PanelLogBuffer,
}

impl PanelLogMakeWriter {
    pub fn new(buffer: PanelLogBuffer) -> Self {
        Self { buffer }
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for PanelLogMakeWriter {
    type Writer = PanelLogEventWriter;

    fn make_writer(&'a self) -> Self::Writer {
        PanelLogEventWriter {
            buffer: self.buffer.clone(),
            pending: String::new(),
        }
    }
}

pub struct PanelLogEventWriter {
    buffer: PanelLogBuffer,
    pending: String,
}

impl Write for PanelLogEventWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.pending.push_str(&String::from_utf8_lossy(buf));
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for PanelLogEventWriter {
    fn drop(&mut self) {
        for line in self.pending.lines() {
            let line = line.trim_end();
            if !line.is_empty() {
                push(&self.buffer, line);
            }
        }
    }
}

/// File counterpart of [`PanelLogMakeWriter`]: one line per file, opened with
/// append and rotated by China-time date.
#[derive(Clone)]
pub struct PanelFileMakeWriter {
    log: Arc<PanelFileLog>,
}

impl PanelFileMakeWriter {
    pub fn new(log: Arc<PanelFileLog>) -> Self {
        Self { log }
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for PanelFileMakeWriter {
    type Writer = PanelFileEventWriter;

    fn make_writer(&'a self) -> Self::Writer {
        PanelFileEventWriter {
            log: self.log.clone(),
            pending: String::new(),
        }
    }
}

pub struct PanelFileEventWriter {
    log: Arc<PanelFileLog>,
    pending: String,
}

impl Write for PanelFileEventWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.pending.push_str(&String::from_utf8_lossy(buf));
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for PanelFileEventWriter {
    fn drop(&mut self) {
        for line in self.pending.lines() {
            let line = line.trim_end();
            if !line.is_empty() {
                self.log.write_line(line);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use tracing_subscriber::fmt::MakeWriter;

    #[test]
    fn buffer_keeps_the_newest_lines_and_caps_memory() {
        let buffer = buffer();
        for i in 0..(MAX_LINES + 5) {
            push(&buffer, &format!("line {i}"));
        }
        let lines = recent(&buffer, MAX_LINES + 100);
        assert_eq!(lines.len(), MAX_LINES);
        assert_eq!(lines.first().unwrap(), "line 5");
        assert_eq!(lines.last().unwrap(), &format!("line {}", MAX_LINES + 4));
    }

    #[test]
    fn writer_flushes_complete_event_on_drop() {
        let buffer = buffer();
        {
            let mut writer = PanelLogMakeWriter::new(buffer.clone()).make_writer();
            writer
                .write_all(b"2026/10/31 21:00:00  INFO hello\nsecond line\n")
                .unwrap();
        }
        assert_eq!(
            recent(&buffer, 10),
            vec!["2026/10/31 21:00:00  INFO hello", "second line"]
        );
    }

    #[test]
    fn china_timer_renders_utc_as_east_eight() {
        let utc = Utc.with_ymd_and_hms(2026, 11, 4, 4, 5, 6).single().unwrap();
        let formatted = format_china_time(utc);
        assert_eq!(formatted, "2026-11-04 12:05:06.000 +08:00");
    }

    #[test]
    fn file_writer_appends_and_lists_the_daily_file() {
        let dir = std::env::temp_dir().join(format!(
            "xray-panel-panel-log-{}-{}",
            std::process::id(),
            "daily"
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let log = Arc::new(PanelFileLog::new(dir.clone()));
        {
            let mut writer = PanelFileMakeWriter::new(log.clone()).make_writer();
            writer.write_all(b"first line\nsecond line\n").unwrap();
        }
        let files = list_log_files(&dir);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].date, files[0].name[6..16]);
        let content = std::fs::read_to_string(dir.join(&files[0].name)).unwrap();
        assert!(content.contains("first line"));
        assert!(content.contains("second line"));
        assert!(resolve_log_file(&dir, "../escape.log").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
