//! Load of the system that runs TalkOps: CPU use, load average, memory and
//! uptime from `/proc`. In a container, `/proc/stat` and `/proc/loadavg`
//! show the host; the memory is the container's cgroup when it has a limit.
//! Linux only; other systems report zeros.

use std::sync::Mutex;

use serde::Serialize;

/// `MemTotal` and `MemAvailable` of `/proc/meminfo` in bytes.
pub fn parse_meminfo(info: &str) -> (u64, u64) {
    let value = |key: &str| {
        info.lines()
            .find_map(|l| l.strip_prefix(key))
            .and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok())
            .map_or(0, |kb| kb * 1024)
    };
    (value("MemTotal:"), value("MemAvailable:"))
}

/// Idle and total CPU ticks of the `cpu` line of `/proc/stat`.
pub fn parse_cpu_ticks(stat: &str) -> Option<(u64, u64)> {
    let line = stat.lines().find(|l| l.starts_with("cpu "))?;
    let ticks: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|v| v.parse().ok())
        .collect();
    // user nice system idle iowait irq softirq steal (guest is in user).
    let idle = ticks.get(3)? + ticks.get(4).copied().unwrap_or(0);
    let total = ticks.iter().take(8).sum();
    Some((idle, total))
}

/// Load averages of 1, 5 and 15 minutes from `/proc/loadavg`.
pub fn parse_loadavg(loadavg: &str) -> [f32; 3] {
    let mut values = loadavg.split_whitespace().map(|v| v.parse().unwrap_or(0.0));
    [(); 3].map(|_| values.next().unwrap_or(0.0))
}

fn read(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// Memory use and limit in bytes: the container's cgroup when it has a
/// limit, else the host.
pub fn memory() -> (u64, u64) {
    let (total, available) = read("/proc/meminfo")
        .map(|s| parse_meminfo(&s))
        .unwrap_or_default();
    let limit = read("/sys/fs/cgroup/memory.max").and_then(|s| s.trim().parse::<u64>().ok());
    let current = read("/sys/fs/cgroup/memory.current").and_then(|s| s.trim().parse::<u64>().ok());
    match (limit, current) {
        (Some(limit), Some(current)) if limit < total || total == 0 => (current, limit),
        _ => (total.saturating_sub(available), total),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, utoipa::ToSchema)]
pub struct SystemLoad {
    /// CPU use of all cores since the previous call in percent (0–100).
    /// `None` on the first call.
    pub cpu_percent: Option<f32>,
    pub cpus: u32,
    pub load1: f32,
    pub load5: f32,
    pub load15: f32,
    pub mem_used_bytes: u64,
    pub mem_total_bytes: u64,
    pub uptime_secs: u64,
}

/// CPU ticks of the previous call, for the CPU use in between.
static LAST_TICKS: Mutex<Option<(u64, u64)>> = Mutex::new(None);

/// Measures the system load now.
pub fn sample() -> SystemLoad {
    let ticks = read("/proc/stat").and_then(|s| parse_cpu_ticks(&s));
    let cpu_percent = {
        let mut last = LAST_TICKS.lock().unwrap_or_else(|e| e.into_inner());
        let percent = match (*last, ticks) {
            (Some((idle0, total0)), Some((idle1, total1))) if total1 > total0 => {
                let busy = (total1 - total0).saturating_sub(idle1.saturating_sub(idle0));
                Some((busy as f32 / (total1 - total0) as f32 * 100.0).clamp(0.0, 100.0))
            }
            _ => None,
        };
        if ticks.is_some() {
            *last = ticks;
        }
        percent
    };
    let [load1, load5, load15] = read("/proc/loadavg")
        .map(|s| parse_loadavg(&s))
        .unwrap_or_default();
    let (mem_used_bytes, mem_total_bytes) = memory();
    SystemLoad {
        cpu_percent,
        cpus: std::thread::available_parallelism().map_or(1, |n| n.get() as u32),
        load1,
        load5,
        load15,
        mem_used_bytes,
        mem_total_bytes,
        uptime_secs: read("/proc/uptime")
            .and_then(|s| s.split_whitespace().next()?.parse::<f64>().ok())
            .map_or(0, |s| s as u64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_proc_files() {
        assert_eq!(
            parse_meminfo("MemTotal:  8000 kB\nMemFree: 100 kB\nMemAvailable: 6000 kB\n"),
            (8000 * 1024, 6000 * 1024)
        );
        assert_eq!(
            parse_cpu_ticks("cpu  100 5 50 800 20 3 2 0 0 0\ncpu0 50 2 25 400 10 1 1 0 0 0\n"),
            Some((820, 980))
        );
        assert_eq!(parse_cpu_ticks("intr 1 2 3\n"), None);
        assert_eq!(
            parse_loadavg("0.42 0.31 0.20 2/345 6789\n"),
            [0.42, 0.31, 0.20]
        );
        assert_eq!(parse_loadavg(""), [0.0; 3]);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn samples_this_system() {
        let first = sample();
        assert!(first.mem_total_bytes > 0);
        assert!(first.cpus >= 1);
        let second = sample();
        if let Some(cpu) = second.cpu_percent {
            assert!((0.0..=100.0).contains(&cpu));
        }
    }
}
