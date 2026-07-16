//! Routing table panel (local shell + remote SSH via background fetch — never block UI).
//!
//! Shows IPv4 + IPv6 routes. Policy rules (`ip rule`) appear only when the
//! platform actually returns them (typically Linux); Windows omits that section.
//!
//! Disconnected SSH tabs must not fall back to local Windows routes.

use crate::i18n;
use connection_mgr::RemoteSession;
use egui::{Color32, RichText, Ui};
use std::path::Path;
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use vault::Vault;

const HEADER_COLOR: Color32 = Color32::from_rgb(100, 105, 115);
const SECTION_COLOR: Color32 = Color32::from_rgb(70, 74, 82);

/// Where route data may be collected from. Never map a dead SSH tab to [`Local`].
pub enum RoutesSource<'a> {
    /// Connected SSH session — remote route table only.
    Remote(&'a RemoteSession),
    /// Connected local shell — this machine's routes.
    Local,
    /// Active tab is not Connected (gray/red status).
    Disconnected,
}

#[derive(Debug, Clone, Default)]
pub struct RouteRow {
    pub destination: String,
    pub gateway: String,
    pub genmask: String,
    pub flags: String,
    pub iface: String,
}

#[derive(Debug, Clone, Default)]
struct RoutesPayload {
    ipv4: Vec<RouteRow>,
    ipv6: Vec<RouteRow>,
    /// Empty → do not show the rules section (missing command / unsupported OS).
    rules: Vec<String>,
    raw: String,
}

#[derive(Default)]
struct RouteCache {
    payload: RoutesPayload,
    fetched_at: Option<Instant>,
    error: Option<String>,
    source_key: String,
    loading: bool,
    rx: Option<mpsc::Receiver<FetchResult>>,
}

struct FetchResult {
    source_key: String,
    result: Result<RoutesPayload, String>,
}

fn cache() -> &'static Mutex<RouteCache> {
    static CACHE: OnceLock<Mutex<RouteCache>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Mutex::new(RouteCache {
            source_key: "local".into(),
            ..Default::default()
        })
    })
}

pub fn show_panel(ui: &mut Ui, source: RoutesSource<'_>, vault_path: Option<&Path>) {
    if matches!(source, RoutesSource::Disconnected) {
        clear_disconnected_cache();
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new(i18n::t("routes.disconnected")).weak());
        });
        return;
    }

    let (source_key, remote) = match source {
        RoutesSource::Remote(session) => (session.display_key(), Some(session)),
        RoutesSource::Local => ("local".into(), None),
        RoutesSource::Disconnected => unreachable!(),
    };

    poll_fetch();
    ensure_fresh(false, remote, vault_path, &source_key);

    ui.horizontal(|ui| {
        if ui.button(i18n::t("routes.refresh")).clicked() {
            ensure_fresh(true, remote, vault_path, &source_key);
        }
        let loading = cache()
            .lock()
            .map(|g| g.loading)
            .unwrap_or(false);
        if loading {
            ui.spinner();
            ui.label(RichText::new(i18n::t("routes.loading")).weak());
        }
    });
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);

    let guard = cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(err) = &guard.error {
        ui.colored_label(egui::Color32::from_rgb(255, 85, 85), err);
    }

    egui::ScrollArea::both()
        .id_salt("routes_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let p = &guard.payload;
            let has_any = !p.ipv4.is_empty() || !p.ipv6.is_empty() || !p.rules.is_empty();
            let mut prior_section = false;

            if !p.ipv4.is_empty() {
                section_title(ui, &i18n::t("routes.ipv4"));
                route_grid(ui, "route_grid_v4", &p.ipv4);
                prior_section = true;
            }
            if !p.ipv6.is_empty() {
                if prior_section {
                    section_divider(ui);
                }
                section_title(ui, &i18n::t("routes.ipv6"));
                route_grid(ui, "route_grid_v6", &p.ipv6);
                prior_section = true;
            }
            if !p.rules.is_empty() {
                if prior_section {
                    section_divider(ui);
                }
                section_title(ui, &i18n::t("routes.rules"));
                for line in &p.rules {
                    ui.label(line);
                }
            }

            if !has_any {
                if guard.loading {
                    ui.label(i18n::t("routes.loading"));
                } else if !p.raw.is_empty() {
                    ui.label(RichText::new(i18n::t("routes.raw")).weak());
                    ui.add_space(6.0);
                    ui.label(RichText::new(&p.raw).size(12.0));
                } else {
                    ui.label(i18n::t("routes.empty"));
                }
            }
        });
}

/// Drop any previously fetched table so a gray-light tab cannot show stale or local data.
fn clear_disconnected_cache() {
    let mut guard = cache().lock().unwrap_or_else(|e| e.into_inner());
    if guard.source_key == "disconnected" && !guard.loading && guard.payload.ipv4.is_empty() {
        return;
    }
    guard.payload = RoutesPayload::default();
    guard.error = None;
    guard.fetched_at = None;
    guard.source_key = "disconnected".into();
    guard.loading = false;
    guard.rx = None;
}

fn section_divider(ui: &mut Ui) {
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(6.0);
}

fn section_title(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(13.0).color(SECTION_COLOR));
    ui.add_space(4.0);
}

fn route_grid(ui: &mut Ui, id: &str, rows: &[RouteRow]) {
    egui::Grid::new(id)
        .num_columns(5)
        .striped(true)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            header_cell(ui, &i18n::t("routes.destination"));
            header_cell(ui, &i18n::t("routes.gateway"));
            header_cell(ui, &i18n::t("routes.mask"));
            header_cell(ui, &i18n::t("routes.flags"));
            header_cell(ui, &i18n::t("routes.iface"));
            ui.end_row();
            for r in rows {
                ui.label(&r.destination);
                ui.label(&r.gateway);
                ui.label(&r.genmask);
                ui.label(&r.flags);
                ui.label(&r.iface);
                ui.end_row();
            }
        });
}

fn header_cell(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(12.0).color(HEADER_COLOR));
}

fn poll_fetch() {
    let mut guard = cache().lock().unwrap_or_else(|e| e.into_inner());
    let Some(rx) = guard.rx.as_ref() else {
        return;
    };
    match rx.try_recv() {
        Ok(FetchResult {
            source_key,
            result,
        }) => {
            guard.rx = None;
            guard.loading = false;
            if source_key != guard.source_key {
                return;
            }
            match result {
                Ok(payload) => {
                    guard.payload = payload;
                    guard.error = None;
                    guard.fetched_at = Some(Instant::now());
                }
                Err(err) => {
                    guard.error = Some(err);
                    guard.fetched_at = Some(Instant::now());
                }
            }
        }
        Err(mpsc::TryRecvError::Empty) => {}
        Err(mpsc::TryRecvError::Disconnected) => {
            guard.rx = None;
            guard.loading = false;
            guard.error = Some(i18n::t("routes.fetch_failed"));
            guard.fetched_at = Some(Instant::now());
        }
    }
}

fn ensure_fresh(
    force: bool,
    remote: Option<&RemoteSession>,
    vault_path: Option<&Path>,
    source_key: &str,
) {
    let mut guard = cache().lock().unwrap_or_else(|e| e.into_inner());
    if guard.source_key != source_key {
        guard.payload = RoutesPayload::default();
        guard.error = None;
        guard.fetched_at = None;
        guard.source_key = source_key.to_string();
        guard.loading = false;
        guard.rx = None;
    }
    if guard.loading {
        return;
    }
    if !force {
        if let Some(at) = guard.fetched_at {
            if at.elapsed() < Duration::from_secs(8) {
                return;
            }
        }
    }

    let remote = remote.cloned();
    let vault_path = vault_path.map(Path::to_path_buf);
    let key = source_key.to_string();
    let (tx, rx) = mpsc::channel();
    guard.loading = true;
    guard.rx = Some(rx);
    drop(guard);

    std::thread::Builder::new()
        .name("vsterm-routes-fetch".into())
        .spawn(move || {
            // `remote == None` is only used for RoutesSource::Local (connected local shell).
            let result = if let Some(session) = remote {
                fetch_remote_routes(&session, vault_path.as_deref())
            } else {
                fetch_local_routes()
            };
            let _ = tx.send(FetchResult {
                source_key: key,
                result,
            });
        })
        .ok();
}

fn decode_command_output(bytes: &[u8]) -> String {
    #[cfg(windows)]
    {
        use encoding_rs::GBK;
        let (decoded, _, _) = GBK.decode(bytes);
        decoded.into_owned()
    }
    #[cfg(not(windows))]
    {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

/// Sectioned remote collect — markers survive BusyBox / missing tools.
const REMOTE_ROUTES_CMD: &str = r#"{
  export LC_ALL=C LANG=C
  echo __VSTERM_ROUTES4__
  out4=`ip -4 route show 2>/dev/null`
  [ -n "$out4" ] || out4=`ip -4 route 2>/dev/null`
  [ -n "$out4" ] || out4=`ip route show 2>/dev/null`
  [ -n "$out4" ] || out4=`ip route 2>/dev/null`
  [ -n "$out4" ] || out4=`route -n 2>/dev/null`
  [ -n "$out4" ] || out4=`cat /proc/net/route 2>/dev/null`
  printf '%s\n' "$out4"
  echo __VSTERM_ROUTES6__
  out6=`ip -6 route show 2>/dev/null`
  [ -n "$out6" ] || out6=`ip -6 route 2>/dev/null`
  [ -n "$out6" ] || out6=`route -A inet6 -n 2>/dev/null`
  [ -n "$out6" ] || out6=`cat /proc/net/ipv6_route 2>/dev/null`
  printf '%s\n' "$out6"
  echo __VSTERM_RULES__
  rules=`ip rule show 2>/dev/null`
  [ -n "$rules" ] || rules=`ip rule 2>/dev/null`
  printf '%s\n' "$rules"
}; true"#;

fn fetch_remote_routes(
    session: &RemoteSession,
    vault_path: Option<&Path>,
) -> Result<RoutesPayload, String> {
    let vault = vault_path.and_then(|p| Vault::open(p).ok());
    let raw = session
        .run_command(vault.as_ref(), REMOTE_ROUTES_CMD)
        .map_err(|e| e.to_string())?;
    let cleaned = strip_ansi(&raw);
    Ok(parse_sectioned_unix(&cleaned))
}

fn fetch_local_routes() -> Result<RoutesPayload, String> {
    #[cfg(windows)]
    {
        let output = connection_mgr::gui_command("route")
            .arg("print")
            .output()
            .map_err(|e| e.to_string())?;
        let raw = decode_command_output(&output.stdout);
        let (ipv4, ipv6) = parse_windows_route(&raw);
        // Windows has no `ip rule`; policy routing uses a different model — omit section.
        Ok(RoutesPayload {
            ipv4,
            ipv6,
            rules: Vec::new(),
            raw,
        })
    }
    #[cfg(not(windows))]
    {
        let output = connection_mgr::gui_command("sh")
            .args(["-c", REMOTE_ROUTES_CMD])
            .output()
            .map_err(|e| e.to_string())?;
        let raw = decode_command_output(&output.stdout);
        let mut payload = parse_sectioned_unix(&raw);
        // Fallback when `ip` is missing (e.g. macOS): netstat often has both families.
        if payload.ipv4.is_empty() && payload.ipv6.is_empty() {
            let ns = connection_mgr::gui_command("netstat")
                .args(["-rn"])
                .output()
                .ok();
            if let Some(out) = ns {
                let ns_raw = decode_command_output(&out.stdout);
                let (v4, v6) = parse_netstat_rn(&ns_raw);
                payload.ipv4 = v4;
                payload.ipv6 = v6;
                if payload.raw.trim().is_empty() {
                    payload.raw = ns_raw;
                }
            }
        }
        Ok(payload)
    }
}

fn parse_sectioned_unix(raw: &str) -> RoutesPayload {
    let sections = split_marked_sections(raw);
    let v4_raw = sections.get("ROUTES4").cloned().unwrap_or_default();
    let v6_raw = sections.get("ROUTES6").cloned().unwrap_or_default();
    let rules_raw = sections.get("RULES").cloned().unwrap_or_default();

    let ipv4 = parse_unix_route(&v4_raw, RouteFamily::V4);
    let mut ipv6 = parse_unix_route(&v6_raw, RouteFamily::V6);
    if ipv6.is_empty() {
        ipv6 = parse_proc_ipv6_route(&v6_raw);
    }
    let rules = parse_rule_lines(&rules_raw);

    RoutesPayload {
        ipv4,
        ipv6,
        rules,
        raw: raw.to_string(),
    }
}

fn split_marked_sections(raw: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let mut cur = String::new();
    let mut buf = String::new();
    for line in raw.lines() {
        let t = line.trim();
        if let Some(name) = t.strip_prefix("__VSTERM_") {
            if let Some(name) = name.strip_suffix("__") {
                if !cur.is_empty() {
                    map.insert(cur.clone(), buf.trim().to_string());
                }
                cur = name.to_string();
                buf.clear();
                continue;
            }
        }
        if !cur.is_empty() {
            buf.push_str(line);
            buf.push('\n');
        }
    }
    if !cur.is_empty() {
        map.insert(cur, buf.trim().to_string());
    }
    map
}

fn parse_rule_lines(raw: &str) -> Vec<String> {
    raw.lines()
        .map(str::trim)
        .filter(|l| {
            !l.is_empty()
                && !l.contains("not found")
                && !l.contains("No such file")
                && !l.starts_with("password")
                && !l.contains("Permission denied")
        })
        .map(str::to_string)
        .collect()
}

#[derive(Clone, Copy)]
enum RouteFamily {
    V4,
    V6,
}

#[cfg(windows)]
fn parse_windows_route(raw: &str) -> (Vec<RouteRow>, Vec<RouteRow>) {
    let mut ipv4 = Vec::new();
    let mut ipv6 = Vec::new();
    let mut section = WinSection::None;
    let mut in_active = false;

    for line in raw.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if t.starts_with("IPv4 Route Table")
            || t.contains("IPv4 路由表")
            || t == "IPv4 Route Table"
        {
            section = WinSection::V4;
            in_active = false;
            continue;
        }
        if t.starts_with("IPv6 Route Table") || t.contains("IPv6 路由表") {
            section = WinSection::V6;
            in_active = false;
            continue;
        }
        if t.starts_with("Active Routes:")
            || t.contains("活动路由")
            || t.starts_with("Active Routes")
        {
            in_active = true;
            continue;
        }
        if t.starts_with("Persistent Routes:") || t.contains("永久路由") {
            in_active = false;
            continue;
        }
        if !in_active {
            // Older English `route print` may print IPv4 active table without an IPv4 header.
            if section == WinSection::None
                && (t.starts_with("Network Destination")
                    || t.starts_with("网络目标")
                    || t.starts_with("网络地址"))
            {
                section = WinSection::V4;
                in_active = true;
                continue;
            }
            continue;
        }
        if t.starts_with("Network Destination")
            || t.starts_with("网络目标")
            || t.starts_with("网络地址")
            || t.starts_with("If Metric")
            || t.starts_with("接口 跃点数")
            || t.starts_with("=")
        {
            continue;
        }

        let parts: Vec<&str> = t.split_whitespace().collect();
        match section {
            WinSection::V4 | WinSection::None => {
                // Destination Netmask Gateway Interface Metric
                if parts.len() >= 5 && parts[0].contains('.') {
                    ipv4.push(RouteRow {
                        destination: parts[0].into(),
                        genmask: parts[1].into(),
                        gateway: parts[2].into(),
                        iface: parts[3].into(),
                        flags: parts.get(4).copied().unwrap_or("").into(),
                    });
                }
            }
            WinSection::V6 => {
                // If Metric Network Destination Gateway
                if parts.len() >= 4 && looks_like_ipv6_dest(parts[2]) {
                    ipv6.push(RouteRow {
                        destination: parts[2].into(),
                        gateway: parts[3..].join(" "),
                        genmask: String::new(),
                        flags: parts[1].into(), // metric
                        iface: parts[0].into(), // interface index
                    });
                }
            }
        }
    }
    (ipv4, ipv6)
}

#[cfg(windows)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum WinSection {
    None,
    V4,
    V6,
}

#[cfg(windows)]
fn looks_like_ipv6_dest(s: &str) -> bool {
    s == "default"
        || s.contains(':')
        || s.eq_ignore_ascii_case("on-link")
        || (s.contains('/') && !s.contains('.'))
}

fn parse_unix_route(raw: &str, family: RouteFamily) -> Vec<RouteRow> {
    let mut rows = Vec::new();
    for line in raw.lines() {
        let t = line.trim();
        if t.is_empty()
            || t.starts_with("Kernel")
            || t.starts_with("Destination")
            || t.starts_with("Iface")
            || t.starts_with("password")
            || t.contains("Permission denied")
            || t.starts_with("__VSTERM_")
        {
            continue;
        }
        let parts: Vec<&str> = t.split_whitespace().collect();

        // `/proc/net/route`: Iface Destination Gateway Flags RefCnt Use Metric Mask …
        if matches!(family, RouteFamily::V4)
            && parts.len() >= 8
            && parts[1].len() == 8
            && parts[1].chars().all(|c| c.is_ascii_hexdigit())
        {
            if let (Some(dest), Some(gw), Some(mask)) = (
                hex_ipv4_le(parts[1]),
                hex_ipv4_le(parts[2]),
                hex_ipv4_le(parts[7]),
            ) {
                rows.push(RouteRow {
                    destination: dest,
                    gateway: gw,
                    genmask: mask,
                    flags: parts[3].into(),
                    iface: parts[0].into(),
                });
            }
            continue;
        }

        let dest_ok = match family {
            RouteFamily::V4 => {
                parts[0] == "default"
                    || parts[0].contains('/')
                    || parts[0].contains('.')
                    || parts[0] == "0.0.0.0"
            }
            RouteFamily::V6 => {
                parts[0] == "default"
                    || parts[0].contains(':')
                    || (parts[0].contains('/') && !parts[0].contains('.'))
                    || parts[0].eq_ignore_ascii_case("unspecified")
            }
        };
        if parts.len() >= 2 && dest_ok {
            let gateway = parts
                .iter()
                .position(|p| *p == "via")
                .and_then(|i| parts.get(i + 1))
                .unwrap_or(&"")
                .to_string();
            let iface = parts
                .iter()
                .position(|p| *p == "dev")
                .and_then(|i| parts.get(i + 1))
                .copied()
                .unwrap_or("")
                .to_string();
            // `route -n` / `route -A inet6 -n`: Destination Gateway Genmask Flags Metric Ref Use Iface
            let (gateway, genmask, flags, iface) = if iface.is_empty() && parts.len() >= 8 {
                (
                    parts[1].to_string(),
                    parts[2].to_string(),
                    parts[3].to_string(),
                    parts[7].to_string(),
                )
            } else {
                (gateway, String::new(), String::new(), iface)
            };
            rows.push(RouteRow {
                destination: parts[0].into(),
                gateway,
                genmask,
                flags,
                iface,
            });
        }
    }
    rows
}

/// Kernel `/proc/net/ipv6_route` (hex fields). Best-effort; prefer `ip -6 route`.
fn parse_proc_ipv6_route(raw: &str) -> Vec<RouteRow> {
    let mut rows = Vec::new();
    for line in raw.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        // dest prefix_len src src_prefix gw metric refcnt use flags iface
        if parts.len() < 10 {
            continue;
        }
        if parts[0].len() != 32 || !parts[0].chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let Some(dest) = hex_ipv6(parts[0]) else {
            continue;
        };
        let prefix = u8::from_str_radix(parts[1], 16).unwrap_or(0);
        let gw = hex_ipv6(parts[4]).unwrap_or_default();
        let iface = parts[9].to_string();
        rows.push(RouteRow {
            destination: format!("{dest}/{prefix}"),
            gateway: if gw == "::" {
                String::new()
            } else {
                gw
            },
            genmask: String::new(),
            flags: parts.get(8).copied().unwrap_or("").into(),
            iface,
        });
    }
    rows
}

fn hex_ipv6(hex: &str) -> Option<String> {
    if hex.len() != 32 {
        return None;
    }
    let mut groups = Vec::with_capacity(8);
    for i in 0..8 {
        let g = &hex[i * 4..i * 4 + 4];
        let n = u16::from_str_radix(g, 16).ok()?;
        groups.push(format!("{n:x}"));
    }
    // Compress longest zero run naively by relying on std Display of parsed addr when possible.
    let joined = groups.join(":");
    joined
        .parse::<std::net::Ipv6Addr>()
        .ok()
        .map(|a| a.to_string())
        .or(Some(joined))
}

#[cfg(not(windows))]
fn parse_netstat_rn(raw: &str) -> (Vec<RouteRow>, Vec<RouteRow>) {
    let mut ipv4 = Vec::new();
    let mut ipv6 = Vec::new();
    let mut family = RouteFamily::V4;
    for line in raw.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with("Routing tables") || t.starts_with("Destination") {
            continue;
        }
        if t.starts_with("Internet:") || t.starts_with("Internet (IPv4)") {
            family = RouteFamily::V4;
            continue;
        }
        if t.starts_with("Internet6:") || t.starts_with("Internet (IPv6)") {
            family = RouteFamily::V6;
            continue;
        }
        let parts: Vec<&str> = t.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let dest = parts[0];
        let is_v6 = dest.contains(':') || (matches!(family, RouteFamily::V6) && !dest.contains('.'));
        let row = RouteRow {
            destination: dest.into(),
            gateway: parts[1].into(),
            genmask: String::new(),
            flags: parts.get(2).copied().unwrap_or("").into(),
            iface: parts.last().copied().unwrap_or("").into(),
        };
        if is_v6 || matches!(family, RouteFamily::V6) {
            ipv6.push(row);
        } else {
            ipv4.push(row);
        }
    }
    (ipv4, ipv6)
}

fn hex_ipv4_le(hex: &str) -> Option<String> {
    if hex.len() != 8 {
        return None;
    }
    let n = u32::from_str_radix(hex, 16).ok()?;
    let b = n.to_le_bytes();
    Some(format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3]))
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

#[cfg(test)]
mod route_parse_tests {
    use super::*;

    #[test]
    fn parse_debian_ip_route_onlink_ens18() {
        let raw = "\
default via 192.168.1.1 dev ens18 onlink \n\
192.168.1.0/24 dev ens18 proto kernel scope link src 192.168.1.22 \n";
        let rows = parse_unix_route(raw, RouteFamily::V4);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].destination, "default");
        assert_eq!(rows[0].gateway, "192.168.1.1");
        assert_eq!(rows[0].iface, "ens18");
        assert_eq!(rows[1].destination, "192.168.1.0/24");
        assert_eq!(rows[1].iface, "ens18");
    }

    #[test]
    fn parse_ip6_route_default_and_lla() {
        let raw = "\
::1 dev lo proto kernel metric 256 pref medium\n\
fe80::/64 dev eth0 proto kernel metric 256 pref medium\n\
default via fe80::1 dev eth0 metric 1024 pref medium\n";
        let rows = parse_unix_route(raw, RouteFamily::V6);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[2].destination, "default");
        assert_eq!(rows[2].gateway, "fe80::1");
        assert_eq!(rows[2].iface, "eth0");
    }

    #[test]
    fn parse_sectioned_with_rules() {
        let raw = "\
__VSTERM_ROUTES4__
default via 10.0.0.1 dev eth0
__VSTERM_ROUTES6__
default via fe80::1 dev eth0
__VSTERM_RULES__
0:	from all lookup local
32766:	from all lookup main
32767:	from all lookup default
";
        let p = parse_sectioned_unix(raw);
        assert_eq!(p.ipv4.len(), 1);
        assert_eq!(p.ipv6.len(), 1);
        assert_eq!(p.rules.len(), 3);
        assert!(p.rules[0].contains("lookup local"));
    }

    #[test]
    fn empty_rules_when_missing() {
        let raw = "__VSTERM_ROUTES4__\ndefault via 1.1.1.1 dev eth0\n__VSTERM_ROUTES6__\n__VSTERM_RULES__\n";
        let p = parse_sectioned_unix(raw);
        assert!(p.rules.is_empty());
        assert_eq!(p.ipv4.len(), 1);
    }
}
