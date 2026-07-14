//! Local host metrics (sysinfo). Remote SSH collectors land in stage 4+.

use chrono::{DateTime, Local};
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::{Disks, Networks, System};

const NET_HISTORY: usize = 60;
pub(crate) const NET_HISTORY_LEN: usize = NET_HISTORY;

#[derive(Debug, Clone)]
pub struct ProcessRow {
    pub pid: u32,
    pub name: String,
    pub cpu: f32,
    pub mem_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct NicInfo {
    pub name: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_bps: f64,
    pub tx_bps: f64,
}

#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub name: String,
    pub mount: String,
    pub total: u64,
    pub available: u64,
    pub fs: String,
}

#[derive(Debug, Clone, Default)]
pub struct HostSnapshot {
    #[allow(dead_code)]
    pub collected_at: Option<DateTime<Local>>,
    pub hostname: String,
    pub os_name: String,
    pub os_version: String,
    pub kernel: String,
    pub arch: String,
    pub cpu_model: String,
    pub cpu_usage: f32,
    pub mem_used: u64,
    pub mem_total: u64,
    pub swap_used: u64,
    pub swap_total: u64,
    pub processes: Vec<ProcessRow>,
    pub nics: Vec<NicInfo>,
    pub disks: Vec<DiskInfo>,
    /// Per-nic (rx_bps, tx_bps) history for charts.
    pub net_history: HashMap<String, VecDeque<(f64, f64)>>,
    /// Interface carrying the default route (e.g. `ppp0` on Merlin WAN).
    pub default_if: Option<String>,
}

impl HostSnapshot {
    pub fn mem_pct(&self) -> f32 {
        if self.mem_total == 0 {
            0.0
        } else {
            (self.mem_used as f64 / self.mem_total as f64 * 100.0) as f32
        }
    }

    pub fn swap_pct(&self) -> f32 {
        if self.swap_total == 0 {
            0.0
        } else {
            (self.swap_used as f64 / self.swap_total as f64 * 100.0) as f32
        }
    }

    /// Prefer the default-route NIC when known; else ens*/eth*/wlan* over lo/veth/…
    pub fn prefer_primary_nic(nics: &[NicInfo], default_if: Option<&str>) -> Option<String> {
        if let Some(want) = default_if {
            if !want.is_empty() && nics.iter().any(|n| n.name == want) {
                return Some(want.to_string());
            }
        }
        const PREFERRED: &[&str] = &[
            "ppp", "wan", "ens", "eno", "enp", "enx", "wlan", "wlp", "wlx", "eth",
        ];
        for prefix in PREFERRED {
            if let Some(n) = nics
                .iter()
                .find(|n| n.name.starts_with(prefix) && n.name.len() > prefix.len())
            {
                return Some(n.name.clone());
            }
        }
        nics.iter()
            .find(|n| {
                let l = n.name.to_ascii_lowercase();
                l != "lo"
                    && !l.starts_with("docker")
                    && !l.starts_with("veth")
                    && !l.starts_with("br-")
                    && !l.starts_with("virbr")
                    && !l.contains("loop")
            })
            .or_else(|| nics.first())
            .map(|n| n.name.clone())
    }

    pub fn format_bytes(n: u64) -> String {
        const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
        let mut v = n as f64;
        let mut i = 0;
        while v >= 1024.0 && i < UNITS.len() - 1 {
            v /= 1024.0;
            i += 1;
        }
        if i == 0 {
            format!("{n} {}", UNITS[i])
        } else {
            format!("{v:.1} {}", UNITS[i])
        }
    }

    /// Compact form for narrow monitor columns, e.g. `12.5G/16G`.
    pub fn format_bytes_compact(n: u64) -> String {
        const UNITS: [&str; 5] = ["B", "K", "M", "G", "T"];
        let mut v = n as f64;
        let mut i = 0;
        while v >= 1024.0 && i < UNITS.len() - 1 {
            v /= 1024.0;
            i += 1;
        }
        if i == 0 {
            format!("{n}{}", UNITS[i])
        } else if v >= 100.0 {
            format!("{:.0}{}", v, UNITS[i])
        } else if v >= 10.0 {
            format!("{:.0}{}", v, UNITS[i])
        } else {
            format!("{:.1}{}", v, UNITS[i])
        }
    }

    pub fn format_ratio_compact(used: u64, total: u64) -> String {
        format!(
            "{}/{}",
            Self::format_bytes_compact(used),
            Self::format_bytes_compact(total)
        )
    }

    pub fn format_bps(bps: f64) -> String {
        Self::format_bytes(bps.max(0.0) as u64) + "/s"
    }
}

/// Background sampler for the local machine (used by local PTY sessions).
pub struct MetricsService {
    inner: Arc<Mutex<SamplerState>>,
    stop: Arc<AtomicBool>,
}

struct SamplerState {
    snapshot: HostSnapshot,
    last_tick: Instant,
    selected_nic: Option<String>,
}

impl MetricsService {
    pub fn start() -> Self {
        let inner = Arc::new(Mutex::new(SamplerState {
            snapshot: HostSnapshot::default(),
            last_tick: Instant::now(),
            selected_nic: None,
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let worker = Arc::clone(&inner);
        let stop_flag = Arc::clone(&stop);
        std::thread::Builder::new()
            .name("vsterm-metrics".into())
            .spawn(move || {
                let mut sys = System::new_all();
                sys.refresh_all();
                std::thread::sleep(Duration::from_millis(200));
                while !stop_flag.load(Ordering::SeqCst) {
                    tick(&mut sys, &worker);
                    // Sleep in small slices so shutdown reacts quickly.
                    for _ in 0..20 {
                        if stop_flag.load(Ordering::SeqCst) {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
            })
            .ok();
        Self { inner, stop }
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    pub fn snapshot(&self) -> HostSnapshot {
        self.inner.lock().snapshot.clone()
    }

    pub fn selected_nic(&self) -> Option<String> {
        self.inner.lock().selected_nic.clone()
    }

    pub fn set_selected_nic(&self, name: Option<String>) {
        self.inner.lock().selected_nic = name;
    }
}

impl Drop for MetricsService {
    fn drop(&mut self) {
        self.stop();
    }
}

fn tick(sys: &mut System, state: &Arc<Mutex<SamplerState>>) {
    sys.refresh_cpu_all();
    sys.refresh_memory();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let disks = Disks::new_with_refreshed_list();
    let mut networks = Networks::new_with_refreshed_list();
    std::thread::sleep(Duration::from_millis(50));
    networks.refresh(true);

    let cpu_usage = sys.global_cpu_usage();
    let cpu_model = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "Unknown".into());

    let mut processes: Vec<ProcessRow> = sys
        .processes()
        .iter()
        .map(|(pid, p)| ProcessRow {
            pid: pid.as_u32(),
            name: p.name().to_string_lossy().into_owned(),
            cpu: p.cpu_usage(),
            mem_bytes: p.memory(),
        })
        .collect();
    processes.sort_by(|a, b| {
        b.cpu
            .partial_cmp(&a.cpu)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    processes.truncate(40);

    let disk_rows: Vec<DiskInfo> = disks
        .iter()
        .map(|d| DiskInfo {
            name: d.name().to_string_lossy().into_owned(),
            mount: d.mount_point().to_string_lossy().into_owned(),
            total: d.total_space(),
            available: d.available_space(),
            fs: d.file_system().to_string_lossy().into_owned(),
        })
        .collect();

    let mut guard = state.lock();
    let elapsed = guard.last_tick.elapsed().as_secs_f64().max(0.05);
    guard.last_tick = Instant::now();

    let mut nics = Vec::new();
    for (name, data) in networks.iter() {
        let rx_bytes = data.total_received();
        let tx_bytes = data.total_transmitted();
        let rx_bps = data.received() as f64 / elapsed;
        let tx_bps = data.transmitted() as f64 / elapsed;
        let hist = guard
            .snapshot
            .net_history
            .entry(name.clone())
            .or_insert_with(VecDeque::new);
        hist.push_back((rx_bps, tx_bps));
        while hist.len() > NET_HISTORY {
            hist.pop_front();
        }
        nics.push(NicInfo {
            name: name.clone(),
            rx_bytes,
            tx_bytes,
            rx_bps,
            tx_bps,
        });
    }
    nics.sort_by(|a, b| a.name.cmp(&b.name));

    if guard.selected_nic.is_none() {
        guard.selected_nic = HostSnapshot::prefer_primary_nic(&nics, None);
    } else if let Some(cur) = &guard.selected_nic {
        if !nics.iter().any(|n| n.name == *cur) {
            guard.selected_nic = HostSnapshot::prefer_primary_nic(&nics, None);
        }
    }

    guard.snapshot = HostSnapshot {
        collected_at: Some(Local::now()),
        hostname: System::host_name().unwrap_or_else(|| "unknown".into()),
        os_name: System::name().unwrap_or_else(|| std::env::consts::OS.into()),
        os_version: System::os_version().unwrap_or_default(),
        kernel: System::kernel_version().unwrap_or_default(),
        arch: std::env::consts::ARCH.into(),
        cpu_model,
        cpu_usage,
        mem_used: sys.used_memory(),
        mem_total: sys.total_memory(),
        swap_used: sys.used_swap(),
        swap_total: sys.total_swap(),
        processes,
        nics,
        disks: disk_rows,
        net_history: std::mem::take(&mut guard.snapshot.net_history),
        default_if: None,
    };
}
