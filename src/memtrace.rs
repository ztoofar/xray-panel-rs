//! Per-request and per-job RSS deltas for the memory investigation.
//!
//! The memlog sampler explains *when* RSS grows; this module explains *which
//! path* grew it. Two `/proc/self/statm` reads per request are cheap at the
//! panel's traffic level, and only deltas at or above `PANEL_MEMTRACE_KB`
//! (default 128 KB) are written to the memory log. Every memlog interval also
//! drains a bounded per-path summary (top five by positive KB) so growth that
//! is spread across many sub-threshold requests is still attributable.
//!
//! `PANEL_MEMTRACE_KB=0` disables the probe. The summary map is capped at
//! [`MAX_PATHS`] entries, so a hostile or broken client cannot grow it.

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::Instant,
};

use axum::{
    extract::{Request, State},
    http::{header, Method},
    middleware::Next,
    response::Response,
};
use chrono::Utc;
use once_cell::sync::Lazy;

use crate::{
    panel_log::{format_china_time, PanelFileLog},
    state::AppState,
};

/// RSS deltas below this are allocator noise, not a call worth attributing.
const DEFAULT_MIN_DELTA_KB: i64 = 128;
const MAX_MIN_DELTA_KB: i64 = 1024 * 1024;
/// Distinct normalized paths kept per interval. Enough for routes plus a few
/// hashed asset names; a hard cap keeps a probe from becoming a memory leak.
const MAX_PATHS: usize = 256;
/// Request lines written per wall-clock minute. A path that trips the
/// threshold thousands of times would otherwise turn the memory log into a
/// disk writer.
const MAX_LINES_PER_MINUTE: u64 = 120;
const SUMMARY_TOP: usize = 5;
/// Background jobs run every 1–2 seconds; logging each 150 KB tick would bury
/// the memory file. Only spikes at 4× the request threshold get their own
/// line; everything else lands in the snapshot's `bg_top` aggregate.
const BG_LINE_MULTIPLIER: i64 = 4;
const MAX_BG_LABELS: usize = 32;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Entry {
    count: u64,
    up_kb: i64,
    max_kb: i64,
}

#[derive(Debug, Default)]
struct Stats {
    entries: HashMap<String, Entry>,
    total_requests: u64,
    logged: u64,
    suppressed: u64,
    minute: i64,
    minute_logged: u64,
}

static STATS: Lazy<Mutex<Stats>> = Lazy::new(|| Mutex::new(Stats::default()));
static BG_STATS: Lazy<Mutex<HashMap<String, Entry>>> = Lazy::new(|| Mutex::new(HashMap::new()));

fn parse_min_delta(raw: &str) -> i64 {
    raw.trim()
        .parse::<i64>()
        .map(|value| value.clamp(0, MAX_MIN_DELTA_KB))
        .unwrap_or(DEFAULT_MIN_DELTA_KB)
}

fn configured_min_delta_kb() -> i64 {
    static VALUE: OnceLock<i64> = OnceLock::new();
    *VALUE.get_or_init(|| {
        std::env::var("PANEL_MEMTRACE_KB")
            .ok()
            .map(|raw| parse_min_delta(&raw))
            .unwrap_or(DEFAULT_MIN_DELTA_KB)
    })
}

pub fn enabled() -> bool {
    configured_min_delta_kb() > 0
}

pub fn min_delta_kb() -> i64 {
    configured_min_delta_kb()
}

/// Resident set size from `/proc/self/statm` (field 2, in pages). Unlike the
/// full memlog snapshot this avoids parsing maps and /proc/self/status twice
/// per request.
pub fn rss_kb() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
        let resident: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
        static PAGE_KB: OnceLock<u64> = OnceLock::new();
        let page_kb = *PAGE_KB.get_or_init(|| {
            let size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
            if size > 0 {
                (size as u64 / 1024).max(1)
            } else {
                4
            }
        });
        Some(resident.saturating_mul(page_kb))
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

fn method_name(method: &Method) -> &'static str {
    match method.as_str() {
        "GET" => "GET",
        "POST" => "POST",
        "PUT" => "PUT",
        "PATCH" => "PATCH",
        "DELETE" => "DELETE",
        "HEAD" => "HEAD",
        "OPTIONS" => "OPTIONS",
        _ => "OTHER",
    }
}

fn clip(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

/// Keep the path cheap to aggregate: numeric segments become `:id`, very long
/// segments (hashed asset names) are clipped, and the whole line is capped.
fn normalize_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len().min(160));
    for (index, segment) in path.split('/').enumerate() {
        if index > 0 {
            out.push('/');
        }
        if !segment.is_empty() && segment.bytes().all(|byte| byte.is_ascii_digit()) {
            out.push_str(":id");
        } else if segment.chars().count() > 64 {
            out.push_str(&clip(segment, 64));
            out.push('~');
        } else {
            out.push_str(segment);
        }
        if out.chars().count() >= 160 {
            break;
        }
    }
    clip(&out, 160)
}

fn record(path: &str, delta_kb: i64) {
    let Ok(mut stats) = STATS.lock() else {
        return;
    };
    stats.total_requests = stats.total_requests.saturating_add(1);
    if !stats.entries.contains_key(path) && stats.entries.len() >= MAX_PATHS {
        return;
    }
    let entry = stats.entries.entry(path.to_string()).or_default();
    entry.count = entry.count.saturating_add(1);
    if delta_kb > 0 {
        entry.up_kb = entry.up_kb.saturating_add(delta_kb);
        entry.max_kb = entry.max_kb.max(delta_kb);
    }
}

/// Returns true when the caller may write a request line this minute and
/// records the suppression count otherwise.
fn reserve_line() -> bool {
    let Ok(mut stats) = STATS.lock() else {
        return false;
    };
    let minute = Utc::now().timestamp() / 60;
    if stats.minute != minute {
        stats.minute = minute;
        stats.minute_logged = 0;
    }
    if stats.minute_logged >= MAX_LINES_PER_MINUTE {
        stats.suppressed = stats.suppressed.saturating_add(1);
        return false;
    }
    stats.minute_logged += 1;
    stats.logged = stats.logged.saturating_add(1);
    true
}

fn format_top(entries: &HashMap<String, Entry>) -> String {
    let mut rows: Vec<(&String, &Entry)> = entries.iter().collect();
    rows.sort_by(|left, right| {
        right
            .1
            .up_kb
            .cmp(&left.1.up_kb)
            .then_with(|| right.1.max_kb.cmp(&left.1.max_kb))
            .then_with(|| right.1.count.cmp(&left.1.count))
    });
    let top: Vec<String> = rows
        .into_iter()
        .take(SUMMARY_TOP)
        .map(|(path, entry)| {
            format!(
                "{}:{}:+{}kb/max{}kb",
                clip(path, 70),
                entry.count,
                entry.up_kb,
                entry.max_kb
            )
        })
        .collect();
    if top.is_empty() {
        "-".to_string()
    } else {
        top.join("|")
    }
}

fn format_summary(
    entries: &HashMap<String, Entry>,
    bg_entries: &HashMap<String, Entry>,
    total_requests: u64,
    logged: u64,
    suppressed: u64,
) -> Option<String> {
    if total_requests == 0 && bg_entries.is_empty() {
        return None;
    }
    Some(format!(
        "req_total={total_requests} req_logged={logged} req_suppressed={suppressed} \
         req_top={} bg_top={}",
        format_top(entries),
        format_top(bg_entries)
    ))
}

fn record_bg(label: &str, delta_kb: i64) {
    let Ok(mut entries) = BG_STATS.lock() else {
        return;
    };
    if !entries.contains_key(label) && entries.len() >= MAX_BG_LABELS {
        return;
    }
    let entry = entries.entry(label.to_string()).or_default();
    entry.count = entry.count.saturating_add(1);
    if delta_kb > 0 {
        entry.up_kb = entry.up_kb.saturating_add(delta_kb);
        entry.max_kb = entry.max_kb.max(delta_kb);
    }
}

/// Drain the current interval for the memlog snapshot. The maps are bounded by
/// [`MAX_PATHS`]/[`MAX_BG_LABELS`] and cleared each interval so long-run memory
/// stays flat.
pub fn summary_line() -> Option<String> {
    let Ok(mut stats) = STATS.lock() else {
        return None;
    };
    let Ok(mut bg) = BG_STATS.lock() else {
        return None;
    };
    let summary = format_summary(
        &stats.entries,
        &bg,
        stats.total_requests,
        stats.logged,
        stats.suppressed,
    );
    stats.entries.clear();
    stats.total_requests = 0;
    stats.logged = 0;
    stats.suppressed = 0;
    bg.clear();
    summary
}

/// Record a background operation's RSS delta (traffic tick, billing, outbound
/// check, subscription refresh, maintenance). Every call lands in the
/// interval's `bg_top` aggregate; only a spike of at least
/// [`BG_LINE_MULTIPLIER`] × the request threshold gets its own line, because
/// the 1–2 second ticks would otherwise dominate the file.
pub fn log_delta(log: &PanelFileLog, label: &str, before: Option<u64>, after: Option<u64>) {
    if !enabled() {
        return;
    }
    let (Some(before), Some(after)) = (before, after) else {
        return;
    };
    let delta = after as i64 - before as i64;
    record_bg(label, delta);
    let line_threshold = min_delta_kb().saturating_mul(BG_LINE_MULTIPLIER);
    if delta.abs() < line_threshold {
        return;
    }
    log.write_line(&format!(
        "{} bg {label} rss={before}->{after} delta_kb={delta:+}",
        format_china_time(Utc::now())
    ));
}

pub async fn middleware(State(state): State<AppState>, req: Request, next: Next) -> Response {
    if !enabled() {
        return next.run(req).await;
    }
    let method = method_name(req.method());
    let path = normalize_path(req.uri().path());
    let before = rss_kb();
    let started = Instant::now();
    let response = next.run(req).await;
    let after = rss_kb();
    let status = response.status().as_u16();
    let length = response
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let elapsed_ms = started.elapsed().as_millis();
    if let (Some(before), Some(after)) = (before, after) {
        let delta = after as i64 - before as i64;
        record(&path, delta);
        if delta.abs() >= min_delta_kb() && reserve_line() {
            state.mem_log.write_line(&format!(
                "{} req {method} {path} status={status} rss={before}->{after} \
                 delta_kb={delta:+} len={length} dur_ms={elapsed_ms}",
                format_china_time(Utc::now())
            ));
        }
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_delta_defaults_and_clamps() {
        assert_eq!(parse_min_delta(""), DEFAULT_MIN_DELTA_KB);
        assert_eq!(parse_min_delta("abc"), DEFAULT_MIN_DELTA_KB);
        assert_eq!(parse_min_delta("0"), 0);
        assert_eq!(parse_min_delta(" 64 "), 64);
        assert_eq!(parse_min_delta("-5"), 0);
        assert_eq!(parse_min_delta("99999999"), MAX_MIN_DELTA_KB);
    }

    #[test]
    fn paths_collapse_numeric_ids_and_clip_assets() {
        assert_eq!(
            normalize_path("/morphine/panel/api/inbounds/123/update"),
            "/morphine/panel/api/inbounds/:id/update"
        );
        let long = format!("/assets/{}-abcdef.js", "x".repeat(80));
        let normalized = normalize_path(&long);
        assert!(normalized.starts_with("/assets/"));
        assert!(normalized.ends_with('~'));
        assert!(normalized.chars().count() <= 160);
    }

    #[test]
    fn summary_ranks_positive_growth_and_reports_counts() {
        let mut entries = HashMap::new();
        entries.insert(
            "/a".to_string(),
            Entry {
                count: 3,
                up_kb: 300,
                max_kb: 200,
            },
        );
        entries.insert(
            "/b".to_string(),
            Entry {
                count: 10,
                up_kb: 50,
                max_kb: 10,
            },
        );
        let mut background = HashMap::new();
        background.insert(
            "traffic_tick".to_string(),
            Entry {
                count: 30,
                up_kb: 4500,
                max_kb: 180,
            },
        );
        let summary = format_summary(&entries, &background, 13, 2, 1).expect("summary");
        assert!(summary.contains("req_total=13"));
        assert!(summary.contains("req_logged=2"));
        assert!(summary.contains("req_suppressed=1"));
        assert!(summary.contains("bg_top=traffic_tick:30:+4500kb/max180kb"));
        let a = summary.find("/a:3:+300kb").expect("/a first");
        let b = summary.find("/b:10:+50kb").expect("/b second");
        assert!(a < b, "positive growth must rank first: {summary}");
    }

    #[test]
    fn summary_is_none_without_requests_or_background_work() {
        assert!(format_summary(&HashMap::new(), &HashMap::new(), 0, 0, 0).is_none());
    }

    #[test]
    fn rss_is_available_on_linux() {
        #[cfg(target_os = "linux")]
        assert!(rss_kb().is_some_and(|value| value > 0));
        #[cfg(not(target_os = "linux"))]
        assert!(rss_kb().is_none());
    }
}
