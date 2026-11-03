use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SysStats {
    pub cpu_percent: f64,
    pub cpu_speed_mhz: f64,
    pub mem_total: u64,
    pub mem_used: u64,
    pub mem_percent: f64,
    pub swap_total: u64,
    pub swap_used: u64,
    pub disk_total: u64,
    pub disk_used: u64,
    pub net_tx_bytes: u64,
    pub net_rx_bytes: u64,
    pub net_tx_rate: f64,
    pub net_rx_rate: f64,
    pub tcp_count: u64,
    pub udp_count: u64,
    pub process_rss: u64,
    pub threads: u64,
    pub load1: f64,
    pub load5: f64,
    pub load15: f64,
    pub uptime: u64,
    pub hostname: String,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Debug, Default, PartialEq)]
struct MemInfo {
    total: u64,
    available: u64,
    swap_total: u64,
    swap_free: u64,
}

#[cfg(any(target_os = "linux", test))]
fn parse_meminfo(text: &str) -> MemInfo {
    let mut info = MemInfo::default();
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("MemTotal:") {
            info.total = parse_kb(value);
        } else if let Some(value) = line.strip_prefix("MemAvailable:") {
            info.available = parse_kb(value);
        } else if let Some(value) = line.strip_prefix("SwapTotal:") {
            info.swap_total = parse_kb(value);
        } else if let Some(value) = line.strip_prefix("SwapFree:") {
            info.swap_free = parse_kb(value);
        }
    }
    info
}

// /proc/net/dev columns: iface: rx_bytes ... (7 more) tx_bytes ...
#[cfg(any(target_os = "linux", test))]
fn parse_net_dev(text: &str) -> (u64, u64) {
    let mut rx = 0u64;
    let mut tx = 0u64;
    for line in text.lines() {
        let Some((iface, rest)) = line.split_once(':') else {
            continue;
        };
        if iface.trim() == "lo" {
            continue;
        }
        let fields = rest.split_whitespace().collect::<Vec<_>>();
        if fields.len() < 9 {
            continue;
        }
        rx = rx.saturating_add(fields[0].parse().unwrap_or(0));
        tx = tx.saturating_add(fields[8].parse().unwrap_or(0));
    }
    (rx, tx)
}

// /proc/self/status carries "VmRSS: 1234 kB" and "Threads: 5".
#[cfg(any(target_os = "linux", test))]
fn parse_status_value(text: &str, key: &str) -> Option<u64> {
    text.lines().find_map(|line| {
        line.strip_prefix(key)?
            .split_whitespace()
            .next()?
            .parse()
            .ok()
    })
}

#[cfg(any(target_os = "linux", test))]
fn count_sockets(text: &str) -> u64 {
    text.lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .count() as u64
}

#[cfg(any(target_os = "linux", test))]
fn rate(previous: u64, current: u64, seconds: f64) -> f64 {
    if seconds <= 0.0 {
        return 0.0;
    }
    current.saturating_sub(previous) as f64 / seconds
}

#[cfg(unix)]
fn disk_usage(path: &str) -> (u64, u64) {
    use std::ffi::CString;

    let Ok(path) = CString::new(path) else {
        return (0, 0);
    };
    let mut buffer = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    let result = unsafe { libc::statvfs(path.as_ptr(), buffer.as_mut_ptr()) };
    if result != 0 {
        return (0, 0);
    }
    let stat = unsafe { buffer.assume_init() };
    let block_size = stat.f_frsize as u64;
    let total = (stat.f_blocks as u64).saturating_mul(block_size);
    let free = (stat.f_bfree as u64).saturating_mul(block_size);
    (total.saturating_sub(free), total)
}

#[cfg(not(unix))]
#[allow(dead_code)]
fn disk_usage(_path: &str) -> (u64, u64) {
    (0, 0)
}

#[cfg(target_os = "linux")]
pub fn snapshot() -> SysStats {
    use std::sync::Mutex;
    use std::time::Instant;

    static CPU_SAMPLE: once_cell::sync::Lazy<Mutex<(u64, u64)>> =
        once_cell::sync::Lazy::new(|| Mutex::new((0, 0)));
    // instant, rx bytes, tx bytes, last rx rate, last tx rate
    type NetSample = (Instant, u64, u64, f64, f64);
    static NET_SAMPLE: once_cell::sync::Lazy<Mutex<Option<NetSample>>> =
        once_cell::sync::Lazy::new(|| Mutex::new(None));

    let mut stats = SysStats {
        hostname: std::fs::read_to_string("/proc/sys/kernel/hostname")
            .map(|value| value.trim().to_string())
            .unwrap_or_default(),
        ..SysStats::default()
    };

    if let Some((total, idle)) = cpu_times() {
        let mut previous = CPU_SAMPLE.lock().unwrap_or_else(|err| err.into_inner());
        if previous.0 > 0 && total > previous.0 {
            let total_delta = (total - previous.0) as f64;
            let idle_delta = (idle - previous.1) as f64;
            stats.cpu_percent = ((total_delta - idle_delta) / total_delta * 1000.0).round() / 10.0;
        }
        *previous = (total, idle);
    }

    if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
        stats.cpu_speed_mhz = cpuinfo
            .lines()
            .find_map(|line| {
                let value = line.strip_prefix("cpu MHz")?;
                value.split_once(':')?.1.trim().parse().ok()
            })
            .unwrap_or(0.0);
    }

    if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
        let info = parse_meminfo(&meminfo);
        if info.total > 0 {
            stats.mem_total = info.total * 1024;
            stats.mem_used = info.total.saturating_sub(info.available) * 1024;
            stats.mem_percent =
                ((stats.mem_used as f64 / stats.mem_total as f64) * 1000.0).round() / 10.0;
        }
        stats.swap_total = info.swap_total * 1024;
        stats.swap_used = info.swap_total.saturating_sub(info.swap_free) * 1024;
    }

    let (disk_used, disk_total) = disk_usage("/");
    stats.disk_used = disk_used;
    stats.disk_total = disk_total;

    if let Ok(net_dev) = std::fs::read_to_string("/proc/net/dev") {
        let (rx, tx) = parse_net_dev(&net_dev);
        stats.net_rx_bytes = rx;
        stats.net_tx_bytes = tx;
        let mut previous = NET_SAMPLE.lock().unwrap_or_else(|err| err.into_inner());
        if let Some((at, last_rx, last_tx, last_rx_rate, last_tx_rate)) = *previous {
            let seconds = at.elapsed().as_secs_f64();
            if seconds >= 0.2 {
                stats.net_rx_rate = rate(last_rx, rx, seconds);
                stats.net_tx_rate = rate(last_tx, tx, seconds);
            } else {
                stats.net_rx_rate = last_rx_rate;
                stats.net_tx_rate = last_tx_rate;
            }
        }
        *previous = Some((Instant::now(), rx, tx, stats.net_rx_rate, stats.net_tx_rate));
    }

    for path in ["/proc/net/tcp", "/proc/net/tcp6"] {
        if let Ok(table) = std::fs::read_to_string(path) {
            stats.tcp_count = stats.tcp_count.saturating_add(count_sockets(&table));
        }
    }
    for path in ["/proc/net/udp", "/proc/net/udp6"] {
        if let Ok(table) = std::fs::read_to_string(path) {
            stats.udp_count = stats.udp_count.saturating_add(count_sockets(&table));
        }
    }

    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        stats.process_rss = parse_status_value(&status, "VmRSS:").unwrap_or(0) * 1024;
        stats.threads = parse_status_value(&status, "Threads:").unwrap_or(0);
    }

    if let Ok(loadavg) = std::fs::read_to_string("/proc/loadavg") {
        let mut parts = loadavg.split_whitespace();
        stats.load1 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
        stats.load5 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
        stats.load15 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    }

    if let Ok(uptime) = std::fs::read_to_string("/proc/uptime") {
        stats.uptime = uptime
            .split_whitespace()
            .next()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0) as u64;
    }

    stats
}

#[cfg(not(target_os = "linux"))]
pub fn snapshot() -> SysStats {
    SysStats {
        hostname: std::env::var("COMPUTERNAME").unwrap_or_default(),
        ..SysStats::default()
    }
}

#[cfg(target_os = "linux")]
fn cpu_times() -> Option<(u64, u64)> {
    let stat = std::fs::read_to_string("/proc/stat").ok()?;
    let line = stat.lines().next()?;
    let values = line
        .split_whitespace()
        .skip(1)
        .filter_map(|value| value.parse::<u64>().ok())
        .collect::<Vec<_>>();
    if values.len() < 4 {
        return None;
    }
    let total: u64 = values.iter().sum();
    let idle = values[3] + values.get(4).copied().unwrap_or(0);
    Some((total, idle))
}

#[cfg(any(target_os = "linux", test))]
fn parse_kb(value: &str) -> u64 {
    value
        .split_whitespace()
        .next()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_meminfo_swap() {
        let info = parse_meminfo(
            "MemTotal:       16000000 kB\nMemAvailable:    8000000 kB\n\
             SwapTotal:       2000000 kB\nSwapFree:        1500000 kB\n",
        );
        assert_eq!(info.total, 16_000_000);
        assert_eq!(info.available, 8_000_000);
        assert_eq!(info.swap_total, 2_000_000);
        assert_eq!(info.swap_free, 1_500_000);
    }

    #[test]
    fn sums_net_dev_without_loopback() {
        let text = "Inter-|   Receive ...\n face |bytes ...\n\
             lo: 1000 0 0 0 0 0 0 0 2000 0 0 0 0 0 0 0\n\
             eth0: 5000 0 0 0 0 0 0 0 7000 0 0 0 0 0 0 0\n\
             eth1: 100 0 0 0 0 0 0 0 200 0 0 0 0 0 0 0\n";
        assert_eq!(parse_net_dev(text), (5100, 7200));
    }

    #[test]
    fn parses_process_rss_and_threads() {
        let text = "Name:\txray-panel-rs\nThreads:\t7\nVmRSS:\t   12345 kB\n";
        assert_eq!(parse_status_value(text, "VmRSS:"), Some(12345));
        assert_eq!(parse_status_value(text, "Threads:"), Some(7));
        assert_eq!(parse_status_value(text, "Missing:"), None);
    }

    #[test]
    fn counts_socket_table_rows() {
        let text = "  sl  local_address rem_address   st\n 0: 00000000:0016 00000000:0000 0A\n 1: 0100007F:1F90 0100007F:9C40 01\n";
        assert_eq!(count_sockets(text), 2);
        assert_eq!(count_sockets("  sl  local_address\n"), 0);
    }

    #[test]
    fn computes_rate_and_handles_reset() {
        assert_eq!(rate(100, 1100, 2.0), 500.0);
        assert_eq!(rate(1000, 500, 1.0), 0.0);
        assert_eq!(rate(0, 100, 0.0), 0.0);
    }
}
