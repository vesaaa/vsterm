//! Remote host metrics via SSH (bound to active connection).

use crate::metrics::{DiskInfo, HostSnapshot, NicInfo, ProcessRow, NET_HISTORY_LEN};
use chrono::Local;
use connection_mgr::RemoteSession;
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use vault::Vault;

/// Sectioned remote script — markers keep df / ps / net from mixing.
/// Disks are emitted as `DISK|total|avail|fstype|mount` so parsing is unambiguous.
const METRICS_CMD: &str = r#"
echo VSTERM_BEGIN
hostname
uname -sr
uname -m
grep -m1 model /proc/cpuinfo 2>/dev/null | head -1 | cut -d: -f2-
awk '/^cpu /{u=$2+$4;t=$2+$4+$5; if(t>0) print (u/t)*100; else print 0; exit}' /proc/stat 2>/dev/null
echo __SEC_MEM__
grep -E '^(MemTotal|MemAvailable|SwapTotal|SwapFree):' /proc/meminfo 2>/dev/null
echo __SEC_PS__
ps -eo pid=,pcpu=,rss=,comm= --sort=-pcpu 2>/dev/null | head -20
echo __SEC_DF__
if command -v findmnt >/dev/null 2>&1; then
  findmnt -bnro SIZE,AVAIL,FSTYPE,TARGET -t notmpfs,devtmpfs,devpts,proc,sysfs,cgroup,cgroup2,squashfs,nsfs,ramfs 2>/dev/null | awk '{
    total=$1; avail=$2; fs=$3;
    mount=$4; for(i=5;i<=NF;i++) mount=mount " " $i;
    if (mount=="") next;
    print "DISK|" total "|" avail "|" fs "|" mount
  }'
else
  df -PB1 2>/dev/null | awk '
  BEGIN {
    while ((getline < "/proc/mounts") > 0) {
      mt=$2; gsub(/\\040/, " ", mt); types[mt]=$3
    }
    close("/proc/mounts")
  }
  NR==1 { next }
  {
    total=$2; avail=$4;
    mount=$6; for(i=7;i<=NF;i++) mount=mount " " $i;
    fs=types[mount]; if (fs=="") fs="-"
    print "DISK|" total "|" avail "|" fs "|" mount
  }'
fi
echo __SEC_NET__
awk 'NR>2{n=$1; sub(":","",n); print n,$2,$10}' /proc/net/dev 2>/dev/null
echo VSTERM_END
"#;

const SKIP_FS: &[&str] = &[
    "tmpfs",
    "devtmpfs",
    "squashfs",
    "proc",
    "sysfs",
    "devpts",
    "cgroup",
    "cgroup2",
    "pstore",
    "bpf",
    "debugfs",
    "tracefs",
    "securityfs",
    "hugetlbfs",
    "mqueue",
    "fusectl",
    "configfs",
    "rpc_pipefs",
    "binfmt_misc",
    "autofs",
    "efivarfs",
    "nsfs",
    "ramfs",
];

pub struct RemoteHostService {
    inner: Arc<Mutex<RemoteState>>,
    stop: Arc<AtomicBool>,
}

struct RemoteState {
    target: Option<RemoteTarget>,
    snapshot: HostSnapshot,
    last_tick: Instant,
    selected_nic: Option<String>,
    last_error: Option<String>,
}

#[derive(Clone)]
struct RemoteTarget {
    session: RemoteSession,
    vault_path: PathBuf,
}

impl RemoteHostService {
    pub fn start() -> Self {
        let inner = Arc::new(Mutex::new(RemoteState {
            target: None,
            snapshot: HostSnapshot::default(),
            last_tick: Instant::now(),
            selected_nic: None,
            last_error: None,
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let worker = Arc::clone(&inner);
        let stop_flag = Arc::clone(&stop);
        std::thread::Builder::new()
            .name("vsterm-remote-metrics".into())
            .spawn(move || {
                while !stop_flag.load(Ordering::SeqCst) {
                    let should_poll = {
                        let g = worker.lock();
                        g.target.is_some()
                    };
                    if should_poll {
                        match poll_once(&worker) {
                            Ok(snap) => {
                                let mut g = worker.lock();
                                if g.target.is_some() {
                                    merge_net_history(
                                        &mut g.snapshot.net_history,
                                        &snap.net_history,
                                    );
                                    if g.selected_nic.is_none()
                                        || g.selected_nic.as_ref().is_some_and(|cur| {
                                            !snap.nics.iter().any(|n| n.name == *cur)
                                        })
                                    {
                                        g.selected_nic =
                                            HostSnapshot::prefer_primary_nic(&snap.nics);
                                    }
                                    g.snapshot = snap;
                                    g.last_tick = Instant::now();
                                    g.last_error = None;
                                }
                            }
                            Err(err) => {
                                let mut g = worker.lock();
                                if g.target.is_some() {
                                    g.last_error = Some(err);
                                }
                            }
                        }
                    }
                    for _ in 0..40 {
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
        self.inner.lock().target = None;
        self.stop.store(true, Ordering::SeqCst);
    }

    pub fn bind(&self, remote: Option<RemoteSession>, vault_path: Option<PathBuf>) {
        let mut g = self.inner.lock();
        let prev_key = g
            .target
            .as_ref()
            .map(|t| format!("{}@{}", t.session.config.username, t.session.config.host));
        let next_key = remote
            .as_ref()
            .map(|r| format!("{}@{}", r.config.username, r.config.host));
        g.target = remote.map(|session| RemoteTarget {
            session,
            vault_path: vault_path.unwrap_or_default(),
        });
        if g.target.is_none() || prev_key != next_key {
            g.snapshot = HostSnapshot::default();
            g.selected_nic = None;
            g.last_error = None;
        }
    }

    pub fn snapshot(&self) -> Option<HostSnapshot> {
        let g = self.inner.lock();
        if g.target.is_none() {
            return None;
        }
        Some(g.snapshot.clone())
    }

    pub fn last_error(&self) -> Option<String> {
        self.inner.lock().last_error.clone()
    }

    pub fn selected_nic(&self) -> Option<String> {
        self.inner.lock().selected_nic.clone()
    }

    pub fn set_selected_nic(&self, name: Option<String>) {
        self.inner.lock().selected_nic = name;
    }
}

impl Drop for RemoteHostService {
    fn drop(&mut self) {
        self.stop();
    }
}

fn poll_once(state: &Arc<Mutex<RemoteState>>) -> Result<HostSnapshot, String> {
    let target = {
        let g = state.lock();
        g.target
            .clone()
            .ok_or_else(|| "no remote target".to_string())?
    };
    let vault = if target.vault_path.as_os_str().is_empty() {
        None
    } else {
        Vault::open(&target.vault_path).ok()
    };
    let raw = target
        .session
        .run_command(vault.as_ref(), METRICS_CMD)
        .map_err(|e| format!("ssh exec: {e}"))?;
    let cleaned = strip_ansi(&raw);
    parse_metrics(&cleaned).ok_or_else(|| {
        let preview: String = cleaned.chars().take(240).collect();
        format!("parse failed, output preview: {preview}")
    })
}

fn merge_net_history(
    dst: &mut HashMap<String, VecDeque<(f64, f64)>>,
    src: &HashMap<String, VecDeque<(f64, f64)>>,
) {
    for (k, v) in src {
        let entry = dst.entry(k.clone()).or_insert_with(VecDeque::new);
        if let Some(last) = v.back() {
            entry.push_back(*last);
            while entry.len() > NET_HISTORY_LEN {
                entry.pop_front();
            }
        }
    }
}

fn parse_metrics(raw: &str) -> Option<HostSnapshot> {
    let body = extract_block(raw, "VSTERM_BEGIN", "VSTERM_END")?;
    let sections = split_sections(&body);
    let header = sections.get("").map(Vec::as_slice).unwrap_or(&[]);
    if header.len() < 3 {
        return None;
    }

    let hostname = clean_field(&header[0]);
    let uname = clean_field(&header[1]);
    let arch = clean_field(&header[2]);
    let cpu_model = header
        .get(3)
        .map(|s| clean_field(s))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into());
    let cpu_usage = header
        .get(4)
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(0.0)
        .clamp(0.0, 100.0);

    let mut mem_total = 0u64;
    let mut mem_avail = 0u64;
    let mut swap_total = 0u64;
    let mut swap_free = 0u64;
    for line in sections.get("MEM").into_iter().flatten() {
        if let Some((k, v)) = line.split_once(':') {
            if let Some(kb) = v
                .trim()
                .split_whitespace()
                .next()
                .and_then(|x| x.parse::<u64>().ok())
            {
                let bytes = kb * 1024;
                match k.trim() {
                    "MemTotal" => mem_total = bytes,
                    "MemAvailable" => mem_avail = bytes,
                    "SwapTotal" => swap_total = bytes,
                    "SwapFree" => swap_free = bytes,
                    _ => {}
                }
            }
        }
    }

    let mut processes = Vec::new();
    for line in sections.get("PS").into_iter().flatten() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }
        let Ok(pid) = parts[0].parse::<u32>() else {
            continue;
        };
        let Ok(cpu) = parts[1].parse::<f32>() else {
            continue;
        };
        let Ok(rss_kb) = parts[2].parse::<u64>() else {
            continue;
        };
        processes.push(ProcessRow {
            pid,
            name: parts[3..].join(" "),
            cpu,
            mem_bytes: rss_kb * 1024,
        });
    }

    let mut disks = Vec::new();
    for line in sections.get("DF").into_iter().flatten() {
        if let Some(d) = parse_disk_line(line) {
            disks.push(d);
        }
    }
    disks.sort_by(|a, b| match (a.mount.as_str(), b.mount.as_str()) {
        ("/", _) => std::cmp::Ordering::Less,
        (_, "/") => std::cmp::Ordering::Greater,
        (am, bm) => am.cmp(bm),
    });

    let mut nics = Vec::new();
    for line in sections.get("NET").into_iter().flatten() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let name = parts[0].trim();
        if name.is_empty() || !is_plausible_nic(name) {
            continue;
        }
        let rx = parts[1].parse().unwrap_or(0);
        let tx = parts[2].parse().unwrap_or(0);
        nics.push(NicInfo {
            name: name.into(),
            rx_bytes: rx,
            tx_bytes: tx,
            rx_bps: 0.0,
            tx_bps: 0.0,
        });
    }

    let os_name = uname
        .split_whitespace()
        .next()
        .unwrap_or("Linux")
        .to_string();
    let mut net_history = HashMap::new();
    for n in &nics {
        net_history
            .entry(n.name.clone())
            .or_insert_with(VecDeque::new)
            .push_back((0.0, 0.0));
    }

    Some(HostSnapshot {
        collected_at: Some(Local::now()),
        hostname,
        os_name,
        os_version: uname.clone(),
        kernel: uname,
        arch,
        cpu_model,
        cpu_usage,
        mem_used: mem_total.saturating_sub(mem_avail),
        mem_total,
        swap_used: swap_total.saturating_sub(swap_free),
        swap_total,
        processes,
        nics,
        disks,
        net_history,
    })
}

fn parse_disk_line(line: &str) -> Option<DiskInfo> {
    if let Some(rest) = line.strip_prefix("DISK|") {
        return parse_disk_pipe(rest);
    }
    // Legacy fallback: raw `df -PT -B1` rows (if an older agent still emits them).
    parse_df_legacy(line)
}

fn parse_disk_pipe(rest: &str) -> Option<DiskInfo> {
    let mut parts = rest.splitn(4, '|');
    let total: u64 = parts.next()?.trim().parse().ok()?;
    let available: u64 = parts.next()?.trim().parse().ok()?;
    let fs = parts.next()?.trim().to_string();
    let mount = parts.next()?.trim().to_string();
    if mount.is_empty() || skip_mount(&mount) {
        return None;
    }
    if !fs.is_empty() && fs != "-" && SKIP_FS.iter().any(|s| *s == fs) {
        return None;
    }
    if total == 0 {
        return None;
    }
    Some(DiskInfo {
        name: mount.clone(),
        mount,
        total,
        available,
        fs: if fs.is_empty() || fs == "-" {
            String::new()
        } else {
            fs
        },
    })
}

fn parse_df_legacy(line: &str) -> Option<DiskInfo> {
    // df -PT -B1: Filesystem Type Size Used Avail Use% Mount
    // df -PB1:    Filesystem Size Used Avail Use% Mount
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 6 {
        return None;
    }
    let cap_idx = parts.iter().rposition(|p| p.ends_with('%'))?;
    // With Type: indices … Type Size Used Avail Use% …
    // Without:    … Size Used Avail Use% …  (Size is always three fields before Use%)
    let after_fs = cap_idx; // number of fields from start used before mount; Use% at cap_idx
    let has_type = after_fs >= 5; // fs + type + size + used + avail + use% → index of use% >= 5
    let (fs, total_s, avail_s) = if has_type {
        let fs = parts[1];
        if SKIP_FS.iter().any(|s| *s == fs) {
            return None;
        }
        (fs.to_string(), parts[cap_idx - 3], parts[cap_idx - 1])
    } else if after_fs >= 4 {
        (String::new(), parts[cap_idx - 3], parts[cap_idx - 1])
    } else {
        return None;
    };
    let mount = parts[cap_idx + 1..].join(" ");
    if mount.is_empty() || skip_mount(&mount) {
        return None;
    }
    let total: u64 = total_s.parse().ok()?;
    let available: u64 = avail_s.parse().ok()?;
    if total == 0 {
        return None;
    }
    Some(DiskInfo {
        name: parts[0].into(),
        mount,
        total,
        available,
        fs,
    })
}

fn skip_mount(mount: &str) -> bool {
    mount.starts_with("/run/")
        || mount.starts_with("/sys/")
        || mount.starts_with("/proc/")
        || mount.starts_with("/dev/")
        || mount.starts_with("/snap/")
        || mount == "/run"
        || mount == "/dev"
}

fn is_plausible_nic(name: &str) -> bool {
    if name.contains('/') || name.contains('\\') {
        return false;
    }
    let lower = name.to_ascii_lowercase();
    // Real-ish interface names; reject mount points / process leftovers.
    lower == "lo"
        || lower.starts_with("eth")
        || lower.starts_with("ens")
        || lower.starts_with("enp")
        || lower.starts_with("eno")
        || lower.starts_with("wlan")
        || lower.starts_with("wlp")
        || lower.starts_with("wlx")
        || lower.starts_with("bond")
        || lower.starts_with("br")
        || lower.starts_with("docker")
        || lower.starts_with("veth")
        || lower.starts_with("virbr")
        || lower.starts_with("tun")
        || lower.starts_with("tap")
        || lower.starts_with("wg")
        || lower.starts_with("ppp")
        || lower.starts_with("vmnet")
}

fn split_sections(body: &str) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    let mut cur = String::new();
    for line in body.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if let Some(name) = t.strip_prefix("__SEC_") {
            if let Some(name) = name.strip_suffix("__") {
                cur = name.to_string();
                map.entry(cur.clone()).or_default();
                continue;
            }
        }
        map.entry(cur.clone()).or_default().push(t.to_string());
    }
    map
}

fn extract_block(raw: &str, begin: &str, end: &str) -> Option<String> {
    let start = raw.find(begin)? + begin.len();
    let rest = &raw[start..];
    let end_pos = rest.find(end)?;
    Some(rest[..end_pos].trim().to_string())
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for c2 in chars.by_ref() {
                    if c2.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        if c.is_control() && c != '\n' && c != '\r' && c != '\t' {
            continue;
        }
        out.push(c);
    }
    out
}

fn clean_field(s: &str) -> String {
    s.trim()
        .trim_matches(|c: char| c == '\0' || c.is_control())
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_disk_pipe_keeps_boot_and_home() {
        let root = parse_disk_line("DISK|100000000000|50000000000|ext4|/").unwrap();
        assert_eq!(root.mount, "/");
        let boot = parse_disk_line("DISK|1073741824|800000000|ext4|/boot").unwrap();
        assert_eq!(boot.mount, "/boot");
        let home = parse_disk_line("DISK|500000000000|100000000000|xfs|/home").unwrap();
        assert_eq!(home.mount, "/home");
        assert!(parse_disk_line("DISK|100|50|tmpfs|/run/user/0").is_none());
    }

    #[test]
    fn parse_legacy_df_pt_lines() {
        let line = "/dev/sda2 ext4 105555582976 12345678901 87654321098 13% /";
        let d = parse_disk_line(line).unwrap();
        assert_eq!(d.mount, "/");
        assert_eq!(d.fs, "ext4");
        let boot = "/dev/sda1 ext4 1073741824 200000000 800000000 20% /boot";
        assert_eq!(parse_disk_line(boot).unwrap().mount, "/boot");
    }
}
