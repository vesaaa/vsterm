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
/// BusyBox/OpenWrt/Merlin friendly: no GNU-only `ps`/`df -B1` assumptions.
/// Disks prefer `df` (reliable on Debian/BusyBox); `findmnt` is a fallback only.
/// Process list uses short `comm=` first so long `args` cannot pollute sections.
const METRICS_CMD: &str = r#"
export LC_ALL=C LANG=C
echo VSTERM_BEGIN
hostname 2>/dev/null || cat /proc/sys/kernel/hostname 2>/dev/null || echo unknown
uname -sr 2>/dev/null || echo Linux
uname -m 2>/dev/null || echo unknown
( grep -m1 -E 'model name|cpu model|Hardware|system type' /proc/cpuinfo 2>/dev/null || true ) | head -n 1 | sed 's/^[^:]*:[ \t]*//'
awk '/^cpu /{u=$2+$4;t=$2+$4+$5; if(t>0) print (u/t)*100; else print 0; exit}' /proc/stat 2>/dev/null || echo 0
echo __SEC_OSREL__
echo "UNAME_S=`uname -s 2>/dev/null || echo Linux`"
echo "UNAME_O=`uname -o 2>/dev/null || true`"
# WSL / Microsoft Linux subsystem (before distro ID so icon can be Windows).
if grep -qiE 'microsoft|wsl' /proc/version 2>/dev/null \
  || uname -r 2>/dev/null | grep -qiE 'microsoft|wsl' \
  || [ -n "${WSL_DISTRO_NAME:-}" ] \
  || [ -d /run/WSL ] 2>/dev/null; then
  echo WINDOWS=1
fi
if [ -r /etc/os-release ]; then
  grep -E '^(ID|ID_LIKE|PRETTY_NAME)=' /etc/os-release 2>/dev/null || true
elif [ -r /usr/lib/os-release ]; then
  grep -E '^(ID|ID_LIKE|PRETTY_NAME)=' /usr/lib/os-release 2>/dev/null || true
fi
for f in /etc/openwrt_release /rom/etc/openwrt_release; do
  [ -r "$f" ] || continue
  echo OPENWRT=1
  grep -E '^(DISTRIB_ID|DISTRIB_DESCRIPTION|DISTRIB_RELEASE)=' "$f" 2>/dev/null || true
done
# Asuswrt-Merlin / 官改 / Koolshare 梅林改版：通常无标准 os-release。
# 社区判定：uname -o 含 Merlin、/koolshare、或 nvram + productid/buildno。
case "`uname -o 2>/dev/null`" in *[Mm]erlin*) echo MERLIN=1 ;; esac
[ -d /koolshare ] && echo MERLIN=1
[ -d /jffs/koolshare ] && echo MERLIN=1
NVRAM=
if command -v nvram >/dev/null 2>&1; then
  NVRAM=nvram
elif [ -x /usr/sbin/nvram ]; then
  NVRAM=/usr/sbin/nvram
elif [ -x /sbin/nvram ]; then
  NVRAM=/sbin/nvram
elif [ -x /bin/nvram ]; then
  NVRAM=/bin/nvram
fi
if [ -n "$NVRAM" ]; then
  bn=`$NVRAM get buildno 2>/dev/null`
  en=`$NVRAM get extendno 2>/dev/null`
  pd=`$NVRAM get productid 2>/dev/null`
  [ -n "$bn" ] && echo "NVRAM_BUILDNO=$bn"
  [ -n "$en" ] && echo "NVRAM_EXTENDNO=$en"
  [ -n "$pd" ] && echo "NVRAM_PRODUCT=$pd"
  if [ -n "$bn" ] || [ -n "$pd" ]; then
    echo MERLIN=1
  fi
fi
for f in /etc/os-release /usr/lib/os-release /etc/openwrt_release /rom/etc/openwrt_release; do
  [ -r "$f" ] || continue
  grep -qiE 'merlin|asuswrt|koolshare' "$f" 2>/dev/null && echo MERLIN=1
done
echo __SEC_MEM__
grep -E '^(MemTotal|MemAvailable|MemFree|Buffers|Cached|SwapTotal|SwapFree):' /proc/meminfo 2>/dev/null || true
echo __SEC_PS__
ps_out=`ps -eo pid=,pcpu=,rss=,comm= --sort=-pcpu 2>/dev/null | head -n 20`
[ -n "$ps_out" ] || ps_out=`ps -o pid= -o pcpu= -o rss= -o comm= --sort=-pcpu 2>/dev/null | head -n 20`
if [ -n "$ps_out" ]; then
  printf '%s\n' "$ps_out"
else
  ps_out=`ps -o pid= -o rss= -o args= 2>/dev/null | head -n 40`
  if [ -n "$ps_out" ]; then
    printf '%s\n' "$ps_out" | awk 'NF>=3 {
      pid=$1; rss=$2; $1=""; $2=""; sub(/^ +/,"");
      printf "%s 0.0 %s %s\n", pid, rss, $0
    }' | sort -k3 -nr 2>/dev/null | head -n 20
  else
    for p in /proc/[0-9]*; do
      [ -r "$p/stat" ] || continue
      pid=${p##*/}
      comm=`cat "$p/comm" 2>/dev/null` || continue
      rss=`awk '/^VmRSS:/{print $2; exit}' "$p/status" 2>/dev/null`
      [ -n "$rss" ] || rss=0
      printf '%s 0.0 %s %s\n' "$pid" "$rss" "$comm"
    done 2>/dev/null | sort -k3 -nr 2>/dev/null | head -n 20
  fi
fi
echo __SEC_DF__
disk_out=
df_out=`df -P -k 2>/dev/null`
[ -n "$df_out" ] || df_out=`df -k 2>/dev/null`
[ -n "$df_out" ] || df_out=`df 2>/dev/null`
if [ -n "$df_out" ]; then
  disk_out=`printf '%s\n' "$df_out" | awk '
  BEGIN {
    while ((getline < "/proc/mounts") > 0) {
      mt=$2; gsub(/\\040/, " ", mt); types[mt]=$3
    }
    close("/proc/mounts")
  }
  NR==1 { next }
  NF>=6 {
    total=$2*1024; avail=$4*1024;
    mount=$6; for(i=7;i<=NF;i++) mount=mount " " $i;
    fs=types[mount]; if (fs=="") fs="-"
    if (total+0 <= 0) next
    print "DISK|" total "|" avail "|" fs "|" mount
  }'`
fi
if [ -z "$disk_out" ] && command -v findmnt >/dev/null 2>&1; then
  # util-linux: a single leading "no" negates the whole comma-separated type list.
  disk_out=`findmnt -bnro SIZE,AVAIL,FSTYPE,TARGET -t notmpfs,devtmpfs,devpts,proc,sysfs,cgroup,cgroup2,squashfs,nsfs,ramfs 2>/dev/null | awk '{
    total=$1; avail=$2; fs=$3;
    mount=$4; for(i=5;i<=NF;i++) mount=mount " " $i;
    if (mount=="" || total+0 <= 0) next;
    print "DISK|" total "|" avail "|" fs "|" mount
  }'`
fi
[ -n "$disk_out" ] && printf '%s\n' "$disk_out"
echo __SEC_NET__
awk 'NR>2 {
  n=$1; sub(":", "", n);
  gsub(/^[ \t]+|[ \t]+$/, "", n);
  if (n != "") print n, $2, $10
}' /proc/net/dev 2>/dev/null || true
echo __SEC_ROUTEIF__
( ip -4 route show default 2>/dev/null || ip route show default 2>/dev/null || ip route 2>/dev/null || route -n 2>/dev/null || true ) | awk '
  /^default/ || $1 == "0.0.0.0" {
    for (i=1;i<NF;i++) if ($i=="dev") { print $(i+1); exit }
    if ($1=="0.0.0.0" && NF>=8) { print $NF; exit }
  }'
echo VSTERM_END
true
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
    /// Per-host last view so tab switches keep charts instead of blanking.
    cache: HashMap<String, CachedHostView>,
    /// After a host switch, first successful poll only baselines counters (no bps).
    rate_baseline_pending: bool,
}

#[derive(Clone)]
struct CachedHostView {
    snapshot: HostSnapshot,
    selected_nic: Option<String>,
    last_error: Option<String>,
    last_tick: Instant,
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
            cache: HashMap::new(),
            rate_baseline_pending: false,
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
                                if let Some(key) = g.target.as_ref().map(|t| t.session.display_key())
                                {
                                    let mut snap = snap;
                                    let baseline = g.rate_baseline_pending;
                                    if baseline {
                                        // Keep prior chart history; do not invent bps from stale counters.
                                        for nic in &mut snap.nics {
                                            nic.rx_bps = 0.0;
                                            nic.tx_bps = 0.0;
                                        }
                                        snap.net_history =
                                            std::mem::take(&mut g.snapshot.net_history);
                                        g.rate_baseline_pending = false;
                                    } else {
                                        let elapsed =
                                            g.last_tick.elapsed().as_secs_f64().max(0.05);
                                        let prev: HashMap<String, (u64, u64)> = g
                                            .snapshot
                                            .nics
                                            .iter()
                                            .map(|n| {
                                                (n.name.clone(), (n.rx_bytes, n.tx_bytes))
                                            })
                                            .collect();
                                        let mut rates = HashMap::new();
                                        for nic in &mut snap.nics {
                                            if let Some(&(prx, ptx)) = prev.get(&nic.name) {
                                                nic.rx_bps = nic.rx_bytes.saturating_sub(prx)
                                                    as f64
                                                    / elapsed;
                                                nic.tx_bps = nic.tx_bytes.saturating_sub(ptx)
                                                    as f64
                                                    / elapsed;
                                            }
                                            rates.insert(nic.name.clone(), {
                                                let mut q = VecDeque::new();
                                                q.push_back((nic.rx_bps, nic.tx_bps));
                                                q
                                            });
                                        }
                                        merge_net_history(
                                            &mut g.snapshot.net_history,
                                            &rates,
                                        );
                                        snap.net_history =
                                            std::mem::take(&mut g.snapshot.net_history);
                                    }
                                    if g.selected_nic.is_none()
                                        || g.selected_nic.as_ref().is_some_and(|cur| {
                                            !snap.nics.iter().any(|n| n.name == *cur)
                                        })
                                    {
                                        g.selected_nic = HostSnapshot::prefer_primary_nic(
                                            &snap.nics,
                                            snap.default_if.as_deref(),
                                        );
                                    }
                                    g.snapshot = snap;
                                    g.last_tick = Instant::now();
                                    g.last_error = None;
                                    store_cache(&mut g, &key);
                                }
                            }
                            Err(err) => {
                                let mut g = worker.lock();
                                if let Some(key) = g.target.as_ref().map(|t| t.session.display_key())
                                {
                                    g.last_error = Some(err);
                                    store_cache(&mut g, &key);
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
            .map(|t| t.session.display_key());
        let next_key = remote.as_ref().map(|r| r.display_key());
        if prev_key == next_key {
            // Same host — keep live snapshot; only refresh target handle / vault path.
            g.target = remote.map(|session| RemoteTarget {
                session,
                vault_path: vault_path.unwrap_or_default(),
            });
            return;
        }

        if let Some(key) = prev_key.as_ref() {
            store_cache(&mut g, key);
        }

        g.target = remote.map(|session| RemoteTarget {
            session,
            vault_path: vault_path.unwrap_or_default(),
        });

        if let Some(key) = next_key.as_ref() {
            if let Some(cached) = g.cache.get(key).cloned() {
                g.snapshot = cached.snapshot;
                g.selected_nic = cached.selected_nic;
                g.last_error = cached.last_error;
                g.last_tick = Instant::now();
                g.rate_baseline_pending = true;
                return;
            }
        }

        // First visit to this host — blank until the worker fills it.
        g.snapshot = HostSnapshot::default();
        g.selected_nic = None;
        g.last_error = None;
        g.last_tick = Instant::now();
        g.rate_baseline_pending = true;
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
        let mut g = self.inner.lock();
        g.selected_nic = name;
        if let Some(key) = g.target.as_ref().map(|t| t.session.display_key()) {
            store_cache(&mut g, &key);
        }
    }
}

impl Drop for RemoteHostService {
    fn drop(&mut self) {
        self.stop();
    }
}

fn store_cache(g: &mut RemoteState, key: &str) {
    // Skip caching empty placeholders so a brief blank tab does not wipe a good view.
    if g.snapshot.hostname.is_empty()
        && g.snapshot.nics.is_empty()
        && g.snapshot.cpu_usage == 0.0
    {
        return;
    }
    g.cache.insert(
        key.to_string(),
        CachedHostView {
            snapshot: g.snapshot.clone(),
            selected_nic: g.selected_nic.clone(),
            last_error: g.last_error.clone(),
            last_tick: g.last_tick,
        },
    );
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
    let mut mem_free = 0u64;
    let mut buffers = 0u64;
    let mut cached = 0u64;
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
                    "MemFree" => mem_free = bytes,
                    "Buffers" => buffers = bytes,
                    "Cached" => cached = bytes,
                    "SwapTotal" => swap_total = bytes,
                    "SwapFree" => swap_free = bytes,
                    _ => {}
                }
            }
        }
    }
    if mem_avail == 0 && mem_total > 0 {
        mem_avail = mem_free.saturating_add(buffers).saturating_add(cached);
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

    let default_if = sections
        .get("ROUTEIF")
        .into_iter()
        .flatten()
        .map(|s| s.trim())
        .find(|s| !s.is_empty() && is_plausible_nic(s))
        .map(str::to_string);

    let osrel: Vec<&str> = sections
        .get("OSREL")
        .map(|v| v.iter().map(String::as_str).collect())
        .unwrap_or_default();
    let os_id = parse_osrel_section(&osrel);

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
        default_if,
        os_id,
    })
}

fn parse_osrel_section(lines: &[&str]) -> Option<String> {
    let mut uname_s = String::new();
    let mut uname_o = String::new();
    let mut id = String::new();
    let mut id_like = String::new();
    let mut pretty = String::new();
    let mut distrib_id = String::new();
    let mut distrib_desc = String::new();
    let mut openwrt = false;
    let mut merlin = false;
    let mut windows = false;
    let mut nvram_build = false;
    let mut nvram_product = false;
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line == "OPENWRT=1" {
            openwrt = true;
            continue;
        }
        if line == "MERLIN=1" {
            merlin = true;
            continue;
        }
        if line == "WINDOWS=1" {
            windows = true;
            continue;
        }
        // Legacy: first bare token was `uname -s`.
        if !line.contains('=') {
            if uname_s.is_empty() {
                uname_s = line.to_string();
            }
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let v = v.trim().trim_matches('"').trim_matches('\'');
        match k {
            "UNAME_S" => uname_s = v.to_string(),
            "UNAME_O" => uname_o = v.to_string(),
            "ID" => id = v.to_string(),
            "ID_LIKE" => id_like = v.to_string(),
            "PRETTY_NAME" => pretty = v.to_string(),
            "DISTRIB_ID" => distrib_id = v.to_string(),
            "DISTRIB_DESCRIPTION" => distrib_desc = v.to_string(),
            "NVRAM_BUILDNO" => {
                if !v.is_empty() {
                    nvram_build = true;
                }
            }
            "NVRAM_PRODUCT" => {
                if !v.is_empty() {
                    nvram_product = true;
                }
            }
            "NVRAM_EXTENDNO" => {
                let low = v.to_ascii_lowercase();
                if low.contains("koolshare") || low.contains("merlin") {
                    merlin = true;
                }
            }
            _ => {}
        }
    }
    if !merlin {
        let uo = uname_o.to_ascii_lowercase();
        if uo.contains("merlin") {
            merlin = true;
        }
    }
    if !merlin && (nvram_build || nvram_product) {
        merlin = true;
    }
    // Fold OpenWrt DISTRIB_* into the pretty/id signals used by the mapper.
    if pretty.is_empty() && !distrib_desc.is_empty() {
        pretty = distrib_desc.clone();
    }
    if id.is_empty() && !distrib_id.is_empty() {
        id = distrib_id;
    }
    if !pretty.is_empty() {
        let pl = pretty.to_ascii_lowercase();
        if pl.contains("merlin") || pl.contains("asuswrt") || pl.contains("koolshare") {
            merlin = true;
        }
    }
    crate::os_icon::detect_id_from_release(
        &uname_s, &id, &id_like, &pretty, openwrt, merlin, windows,
    )
    .map(str::to_string)
}

fn parse_disk_line(line: &str) -> Option<DiskInfo> {
    if let Some(rest) = line.strip_prefix("DISK|") {
        return parse_disk_pipe(rest);
    }
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
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 6 {
        return None;
    }
    let cap_idx = parts.iter().rposition(|p| p.ends_with('%'))?;
    let after_fs = cap_idx;
    let has_type = after_fs >= 5;
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
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains(' ') {
        return false;
    }
    // Strip VLAN/peer suffix (eth0@if2, ens18.100).
    let base = name.split(['@', ':']).next().unwrap_or(name);
    let lower = base.to_ascii_lowercase();
    if lower.is_empty()
        || !lower
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
    {
        return false;
    }
    if !lower
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return false;
    }
    // Accept well-known prefixes and generic kernel names (vmbr*, sit*, etc.).
    lower == "lo"
        || lower.starts_with("eth")
        || lower.starts_with("ens")
        || lower.starts_with("enp")
        || lower.starts_with("eno")
        || lower.starts_with("enx")
        || lower.starts_with("wl")
        || lower.starts_with("wlan")
        || lower.starts_with("bond")
        || lower.starts_with("br")
        || lower.starts_with("docker")
        || lower.starts_with("veth")
        || lower.starts_with("virbr")
        || lower.starts_with("tun")
        || lower.starts_with("tap")
        || lower.starts_with("wg")
        || lower.starts_with("ppp")
        || lower.starts_with("wan")
        || lower.starts_with("vlan")
        || lower.starts_with("usb")
        || lower.starts_with("apcli")
        || lower.starts_with("vmnet")
        || lower.starts_with("vmbr")
        || lower.starts_with("nic")
        || lower.starts_with("net")
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
    s.trim().to_string()
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
        assert!(parse_disk_line("DISK|100|50|tmpfs|/run/user/0").is_none());
    }

    #[test]
    fn parse_debian_metrics_with_ens18() {
        let raw = r#"
VSTERM_BEGIN
debian
Linux 6.1.0-37-amd64
x86_64
Intel(R) Xeon(R)
12.5
__SEC_MEM__
MemTotal:        2048000 kB
MemAvailable:    1024000 kB
MemFree:          512000 kB
Buffers:           64000 kB
Cached:           256000 kB
SwapTotal:             0 kB
SwapFree:              0 kB
__SEC_PS__
1 0.0 1024 systemd
__SEC_DF__
DISK|21474836480|10737418240|ext4|/
DISK|1073741824|536870912|ext4|/boot
__SEC_NET__
lo 1000 1000
ens18 2048000 1024000
__SEC_ROUTEIF__
ens18
VSTERM_END
"#;
        let snap = parse_metrics(raw).expect("parse");
        assert_eq!(snap.hostname, "debian");
        assert_eq!(snap.default_if.as_deref(), Some("ens18"));
        assert!(snap.nics.iter().any(|n| n.name == "ens18"));
        assert!(snap.disks.iter().any(|d| d.mount == "/"));
        assert!(snap.disks.iter().any(|d| d.mount == "/boot"));
    }

    #[test]
    fn plausible_nic_accepts_ens_and_vmbr() {
        assert!(is_plausible_nic("ens18"));
        assert!(is_plausible_nic("vmbr0"));
        assert!(is_plausible_nic("eth0@if2"));
        assert!(!is_plausible_nic("/run/foo"));
    }

    #[test]
    fn osrel_detects_merlin_from_uname_o() {
        let lines = [
            "UNAME_S=Linux",
            "UNAME_O=Merlin",
            "MERLIN=1",
        ];
        assert_eq!(parse_osrel_section(&lines).as_deref(), Some("merlin"));
    }

    #[test]
    fn osrel_detects_merlin_from_nvram_product() {
        let lines = [
            "UNAME_S=Linux",
            "UNAME_O=GNU/Linux",
            "NVRAM_BUILDNO=386.7",
            "NVRAM_PRODUCT=RT-AX86U",
            "MERLIN=1",
        ];
        assert_eq!(parse_osrel_section(&lines).as_deref(), Some("merlin"));
    }

    #[test]
    fn osrel_detects_merlin_before_openwrt() {
        let lines = [
            "UNAME_S=Linux",
            "OPENWRT=1",
            "DISTRIB_ID=OpenWrt",
            "DISTRIB_DESCRIPTION=Koolshare Merlin",
            "MERLIN=1",
        ];
        assert_eq!(parse_osrel_section(&lines).as_deref(), Some("merlin"));
    }

    #[test]
    fn osrel_detects_plain_openwrt() {
        let lines = [
            "UNAME_S=Linux",
            "OPENWRT=1",
            "DISTRIB_ID=OpenWrt",
            "DISTRIB_DESCRIPTION=\"OpenWrt SNAPSHOT\"",
        ];
        assert_eq!(parse_osrel_section(&lines).as_deref(), Some("openwrt"));
    }

    #[test]
    fn osrel_detects_ubuntu() {
        let lines = [
            "UNAME_S=Linux",
            "ID=ubuntu",
            "ID_LIKE=debian",
            "PRETTY_NAME=\"Ubuntu 22.04.4 LTS\"",
        ];
        assert_eq!(parse_osrel_section(&lines).as_deref(), Some("ubuntu"));
    }

    #[test]
    fn osrel_detects_wsl_as_windows() {
        let lines = [
            "UNAME_S=Linux",
            "WINDOWS=1",
            "ID=ubuntu",
            "PRETTY_NAME=\"Ubuntu 22.04.4 LTS\"",
        ];
        assert_eq!(parse_osrel_section(&lines).as_deref(), Some("windows"));
    }

    #[test]
    fn osrel_detects_openeuler() {
        let lines = [
            "UNAME_S=Linux",
            "ID=openEuler",
            "PRETTY_NAME=\"openEuler 22.03\"",
        ];
        assert_eq!(parse_osrel_section(&lines).as_deref(), Some("openeuler"));
    }
}
