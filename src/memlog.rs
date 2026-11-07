//! Periodic process-memory snapshots for the low-memory constraint.
//!
//! The panel is expected to stay in the low tens of megabytes on a small VPS,
//! so a slow RSS climb is a regression that needs evidence. One line per
//! interval is appended to `memory-YYYY-MM-DD.log` next to the daily panel
//! logs. The line mixes `/proc` figures (RSS, anonymous/file split, high-water
//! mark, threads, file descriptors, CPU, mapping counts) with the sizes of the
//! panel's long-lived containers, so growth can be attributed either to the
//! allocator or to a specific map.
//!
//! `PANEL_MEMLOG_SECS` overrides the interval (default 300s); `0` disables the
//! sampler. Non-Linux builds still log the container counters with `rss_kb=-`,
//! which keeps the code testable on the Windows dev machine.

use std::{sync::Arc, time::Duration};

use chrono::Utc;

use crate::{
    memtrace,
    panel_log::{format_china_time, PanelFileLog},
    state::AppState,
};

/// File prefix for the daily memory snapshots (kept out of the panel log
/// history list, which only accepts the `panel-` prefix).
pub const LOG_PREFIX: &str = "memory-";
const DEFAULT_INTERVAL_SECS: u64 = 300;
// Keep an accidental `PANEL_MEMLOG_SECS=1` from turning the log into a disk
// writer; a leak is still visible at one sample per minute.
const MIN_INTERVAL_SECS: u64 = 15;
const MAX_INTERVAL_SECS: u64 = 3600;
/// An anonymous mapping at least this large is worth counting separately: it
/// is where a stray `Vec`/`String` or an allocator group shows up first.
#[cfg(any(target_os = "linux", test))]
const BIG_ANON_BYTES: u64 = 1 << 20;

pub fn spawn(state: AppState, log: Arc<PanelFileLog>) {
    let secs = std::env::var("PANEL_MEMLOG_SECS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_INTERVAL_SECS);
    if secs == 0 {
        return;
    }
    let interval = Duration::from_secs(secs.clamp(MIN_INTERVAL_SECS, MAX_INTERVAL_SECS));
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            // The first tick fires immediately: record a startup baseline.
            ticker.tick().await;
            log.write_line(&snapshot(&state).await);
        }
    });
}

async fn snapshot(state: &AppState) -> String {
    let mut line = format_china_time(Utc::now());
    match process_memory() {
        Some(memory) => line.push_str(&memory.format()),
        None => line.push_str(" rss_kb=-"),
    }

    // Long-lived panel containers. Their sizes are the cheapest way to tell a
    // real map leak from allocator retention: if these stay flat while RSS
    // climbs, the memory is not reachable from `AppState`.
    let ws_subs = state.ws_events.receiver_count();
    let online = state.online_clients.lock().await.len();
    let online_seen = state.online_last_seen.lock().await.len();
    let active_inbounds = state.active_inbounds.lock().await.len();
    let active_seen = state.active_inbound_last_seen.lock().await.len();
    let history = state.traffic_history.lock().await.len();
    let (speed_clients, speed_inbounds) = {
        let speeds = state.traffic_speeds.lock().await;
        (speeds.clients.len(), speeds.inbounds.len())
    };
    let panel_log_lines = state
        .panel_logs
        .lock()
        .map(|queue| queue.len())
        .unwrap_or(0);
    let xray_log_lines = state.xray_logs.lock().await.len();
    let (limiter_ips, limiter_hits) = {
        let limiter = state.login_limiter.lock().await;
        (
            limiter.len(),
            limiter
                .values()
                .map(|attempts| attempts.len())
                .sum::<usize>(),
        )
    };
    let outbound_results = state
        .outbound_check
        .lock()
        .await
        .as_ref()
        .map(|run| run.results.len())
        .unwrap_or(0);
    let (db_pages, db_free_pages) = db_pages(state).await;

    line.push_str(&format!(
        " ws_subs={ws_subs} online={online} online_seen={online_seen} \
         active_inbounds={active_inbounds} active_seen={active_seen} \
         history={history} speed_clients={speed_clients} \
         speed_inbounds={speed_inbounds} panel_log={panel_log_lines} \
         xray_log={xray_log_lines} limiter_ips={limiter_ips} \
         limiter_hits={limiter_hits} outbound_results={outbound_results} \
         db_pages={db_pages} db_free={db_free_pages}"
    ));
    // Drains the per-path request summary for this interval. `PANEL_MEMTRACE_KB=0`
    // leaves it empty, keeping the snapshot line identical to 0.2.39.
    if let Some(summary) = memtrace::summary_line() {
        line.push(' ');
        line.push_str(&summary);
    }
    line
}

async fn db_pages(state: &AppState) -> (i64, i64) {
    let pages = sqlx::query_scalar::<_, i64>("PRAGMA page_count")
        .fetch_one(&state.db)
        .await
        .unwrap_or(-1);
    let free = sqlx::query_scalar::<_, i64>("PRAGMA freelist_count")
        .fetch_one(&state.db)
        .await
        .unwrap_or(-1);
    (pages, free)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ProcessMemory {
    rss_kb: u64,
    anon_kb: u64,
    file_kb: u64,
    hwm_kb: u64,
    peak_kb: u64,
    threads: u64,
    fds: u64,
    cpu_ticks: u64,
    maps: u64,
    big_anon_maps: u64,
    big_anon_kb: u64,
}

impl ProcessMemory {
    fn format(&self) -> String {
        let cpu_seconds = self.cpu_ticks as f64 / clock_ticks_per_second();
        format!(
            " rss_kb={} anon_kb={} file_kb={} hwm_kb={} peak_kb={} \
             threads={} fds={} cpu_s={:.1} maps={} big_anon_maps={} big_anon_kb={}",
            self.rss_kb,
            self.anon_kb,
            self.file_kb,
            self.hwm_kb,
            self.peak_kb,
            self.threads,
            self.fds,
            cpu_seconds,
            self.maps,
            self.big_anon_maps,
            self.big_anon_kb,
        )
    }
}

fn clock_ticks_per_second() -> f64 {
    #[cfg(target_os = "linux")]
    {
        let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
        if ticks > 0 {
            return ticks as f64;
        }
    }
    100.0
}

#[cfg(target_os = "linux")]
fn process_memory() -> Option<ProcessMemory> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let mut memory = parse_status(&status);
    memory.fds = std::fs::read_dir("/proc/self/fd")
        .map(|entries| entries.count() as u64)
        .unwrap_or(0);
    memory.cpu_ticks = read_cpu_ticks().unwrap_or(0);
    if let Ok(maps) = std::fs::read_to_string("/proc/self/maps") {
        let (total, big, big_kb) = count_maps(&maps);
        memory.maps = total;
        memory.big_anon_maps = big;
        memory.big_anon_kb = big_kb;
    }
    Some(memory)
}

#[cfg(not(target_os = "linux"))]
fn process_memory() -> Option<ProcessMemory> {
    None
}

#[cfg(target_os = "linux")]
fn read_cpu_ticks() -> Option<u64> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    // The command name may contain spaces and parentheses, so start after the
    // last `)`; a plain split from the beginning would misalign every field.
    let rest = stat.get(stat.rfind(')')? + 1..)?;
    let fields: Vec<&str> = rest.split_whitespace().collect();
    // After the command name the fields start at state (field 3); utime and
    // stime are fields 14 and 15.
    let user = fields.get(11)?.parse::<u64>().ok()?;
    let system = fields.get(12)?.parse::<u64>().ok()?;
    Some(user.saturating_add(system))
}

#[cfg(any(target_os = "linux", test))]
fn parse_status(text: &str) -> ProcessMemory {
    let mut memory = ProcessMemory::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value
            .split_whitespace()
            .next()
            .and_then(|raw| raw.parse::<u64>().ok())
            .unwrap_or(0);
        match key.trim() {
            "VmRSS" => memory.rss_kb = value,
            "VmHWM" => memory.hwm_kb = value,
            "VmPeak" => memory.peak_kb = value,
            "RssAnon" => memory.anon_kb = value,
            "RssFile" => memory.file_kb = value,
            "Threads" => memory.threads = value,
            _ => {}
        }
    }
    // Kernels before 4.5 do not report RssAnon/RssFile; derive the split from
    // RSS so the line stays meaningful.
    if memory.anon_kb == 0 && memory.rss_kb > 0 {
        memory.anon_kb = memory.rss_kb.saturating_sub(memory.file_kb);
    }
    memory
}

/// Count mappings and the anonymous writable ones at least 1 MiB. Thread
/// stacks are included in the big-anonymous count; `threads=` makes them easy
/// to subtract.
#[cfg(any(target_os = "linux", test))]
fn count_maps(maps: &str) -> (u64, u64, u64) {
    let mut total = 0u64;
    let mut big = 0u64;
    let mut big_kb = 0u64;
    for line in maps.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 5 {
            continue;
        }
        total += 1;
        // An anonymous mapping has exactly five fields; named ones ([heap],
        // [stack], files, …) carry a sixth.
        if fields.len() != 5 || fields[1] != "rw-p" {
            continue;
        }
        let Some((start, end)) = fields[0].split_once('-') else {
            continue;
        };
        let (Ok(start), Ok(end)) = (u64::from_str_radix(start, 16), u64::from_str_radix(end, 16))
        else {
            continue;
        };
        let bytes = end.saturating_sub(start);
        if bytes >= BIG_ANON_BYTES {
            big += 1;
            big_kb += bytes / 1024;
        }
    }
    (total, big, big_kb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_parses_rss_and_the_anonymous_split() {
        let status = "\
Name:\txray-panel-rs
VmPeak:\t  170572 kB
VmHWM:\t  150300 kB
VmRSS:\t  134596 kB
RssAnon:\t  128028 kB
RssFile:\t    6568 kB
Threads:\t5
";
        let memory = parse_status(status);
        assert_eq!(memory.rss_kb, 134596);
        assert_eq!(memory.anon_kb, 128028);
        assert_eq!(memory.file_kb, 6568);
        assert_eq!(memory.hwm_kb, 150300);
        assert_eq!(memory.peak_kb, 170572);
        assert_eq!(memory.threads, 5);
    }

    #[test]
    fn status_without_the_split_derives_anonymous_from_rss() {
        let memory = parse_status("VmRSS:\t  1000 kB\nRssFile:\t  200 kB\n");
        assert_eq!(memory.anon_kb, 800);
    }

    #[test]
    fn maps_count_only_large_anonymous_writable_regions() {
        let maps = "\
1000000-2000000 rw-p 00000000 00:00 0
7f0000000000-7f0000300000 rw-p 00000000 00:00 0
7f0000400000-7f0000401000 rw-p 00000000 00:00 0
7f0000500000-7f0000600000 r--p 00000000 00:00 0
7f0000600000-7f0000800000 rw-p 00000000 00:00 0 /usr/local/bin/xray-panel-rs
7f0000800000-7f0000a00000 rw-p 00000000 00:00 0 [heap]
";
        let (total, big, big_kb) = count_maps(maps);
        assert_eq!(total, 6);
        // 16 MiB and 3 MiB anonymous writable regions; the 4 KiB one is under
        // the threshold and named mappings are ignored.
        assert_eq!(big, 2);
        assert_eq!(big_kb, 19 * 1024);
    }

    #[test]
    fn formatted_line_keeps_the_expected_keys() {
        let memory = ProcessMemory {
            rss_kb: 1,
            anon_kb: 2,
            file_kb: 3,
            hwm_kb: 4,
            peak_kb: 5,
            threads: 6,
            fds: 7,
            cpu_ticks: 150,
            maps: 8,
            big_anon_maps: 9,
            big_anon_kb: 10,
        };
        let line = memory.format();
        for key in [
            "rss_kb=1",
            "anon_kb=2",
            "file_kb=3",
            "hwm_kb=4",
            "peak_kb=5",
            "threads=6",
            "fds=7",
            "maps=8",
            "big_anon_maps=9",
            "big_anon_kb=10",
        ] {
            assert!(line.contains(key), "missing {key} in {line}");
        }
        assert!(line.contains("cpu_s="), "missing cpu in {line}");
    }
}
