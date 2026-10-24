//! Resource usage of the worker for the transcription page: CPU time and
//! memory of the worker and its Whisper processes, the memory of the
//! container (cgroup v2) or the host, and the load average. Linux only;
//! other systems report zeros.

use std::collections::HashMap;
use std::time::Instant;

use talkops_core::system_load::memory;

/// Clock ticks per second of `/proc/<pid>/stat` (USER_HZ, 100 on Linux).
const TICKS: f32 = 100.0;

/// CPU time (user + system, in ticks) from a `/proc/<pid>/stat` line, and
/// the parent PID. The name in brackets can contain spaces.
pub(crate) fn parse_stat(stat: &str) -> Option<(u32, u64, u64)> {
    let rest = &stat[stat.rfind(')')? + 2..];
    let fields: Vec<&str> = rest.split_whitespace().collect();
    // After the name: state(0) ppid(1) … utime(11) stime(12) cutime(13) cstime(14).
    let ppid = fields.get(1)?.parse().ok()?;
    let own = fields.get(11)?.parse::<u64>().ok()? + fields.get(12)?.parse::<u64>().ok()?;
    let children = fields.get(13)?.parse::<u64>().ok()? + fields.get(14)?.parse::<u64>().ok()?;
    Some((ppid, own, children))
}

/// `VmRSS` of `/proc/<pid>/status` in bytes.
pub(crate) fn parse_rss(status: &str) -> u64 {
    status
        .lines()
        .find_map(|l| l.strip_prefix("VmRSS:"))
        .and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok())
        .map_or(0, |kb| kb * 1024)
}

fn read(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// One measurement of the process tree.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Sample {
    pub cpu_percent: f32,
    pub rss_bytes: u64,
    pub mem_used_bytes: u64,
    pub mem_total_bytes: u64,
    pub load1: f32,
    pub cpus: u32,
}

/// Measures CPU use between two calls.
pub(crate) struct Sampler {
    last: Option<(Instant, u64)>,
}

impl Sampler {
    pub fn new() -> Self {
        Self { last: None }
    }

    /// CPU ticks and memory of this process and its child processes.
    fn tree() -> (u64, u64) {
        let me = std::process::id();
        let mut ticks = 0;
        let mut rss = 0;
        let mut parents: HashMap<u32, u32> = HashMap::new();
        let mut usage: HashMap<u32, (u64, u64)> = HashMap::new();
        if let Ok(dir) = std::fs::read_dir("/proc") {
            for entry in dir.flatten() {
                let Some(pid) = entry
                    .file_name()
                    .to_str()
                    .and_then(|n| n.parse::<u32>().ok())
                else {
                    continue;
                };
                let Some((ppid, own, children)) =
                    read(&format!("/proc/{pid}/stat")).and_then(|s| parse_stat(&s))
                else {
                    continue;
                };
                parents.insert(pid, ppid);
                // A finished child's time moves into the parent's `cutime`.
                let mine = if pid == me { own + children } else { own };
                usage.insert(pid, (mine, 0));
            }
        }
        let in_tree = |mut pid: u32| {
            for _ in 0..16 {
                if pid == me {
                    return true;
                }
                match parents.get(&pid) {
                    Some(&p) if p != 0 => pid = p,
                    _ => return false,
                }
            }
            false
        };
        for (&pid, &(t, _)) in &usage {
            if in_tree(pid) {
                ticks += t;
                rss += read(&format!("/proc/{pid}/status")).map_or(0, |s| parse_rss(&s));
            }
        }
        (ticks, rss)
    }

    pub fn sample(&mut self) -> Sample {
        let now = Instant::now();
        let (ticks, rss_bytes) = Self::tree();
        let cpu_percent = match self.last {
            Some((at, before)) if ticks >= before => {
                let secs = now.duration_since(at).as_secs_f32().max(0.001);
                (ticks - before) as f32 / TICKS / secs * 100.0
            }
            _ => 0.0,
        };
        self.last = Some((now, ticks));
        let (mem_used_bytes, mem_total_bytes) = memory();
        Sample {
            cpu_percent,
            rss_bytes,
            mem_used_bytes,
            mem_total_bytes,
            load1: read("/proc/loadavg")
                .and_then(|s| s.split_whitespace().next()?.parse().ok())
                .unwrap_or(0.0),
            cpus: std::thread::available_parallelism().map_or(1, |n| n.get() as u32),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_proc_files() {
        let stat =
            "4242 (whisper cli) S 4200 4242 4242 0 -1 4194560 100 0 0 0 1500 250 30 20 20 0 9 0";
        assert_eq!(parse_stat(stat), Some((4200, 1750, 50)));
        assert_eq!(
            parse_rss("Name:\twhisper\nVmRSS:\t  2048 kB\nThreads:\t9\n"),
            2048 * 1024
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn samples_this_process() {
        let mut sampler = Sampler::new();
        let first = sampler.sample();
        assert!(first.rss_bytes > 0);
        assert!(first.mem_total_bytes > 0);
        assert!(first.cpus >= 1);
        let mut x = 0u64;
        for i in 0..2_000_000u64 {
            x = x.wrapping_add(i * i);
        }
        std::hint::black_box(x);
        assert!(sampler.sample().cpu_percent >= 0.0);
    }
}
