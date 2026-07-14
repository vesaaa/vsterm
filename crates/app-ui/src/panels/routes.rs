//! Routing table panel (local + remote via background fetch — never block UI).

use crate::i18n;
use connection_mgr::RemoteSession;
use egui::{Color32, RichText, Ui};
use std::path::Path;
use std::process::Command;
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use vault::Vault;

const HEADER_COLOR: Color32 = Color32::from_rgb(100, 105, 115);

#[derive(Debug, Clone, Default)]
pub struct RouteRow {
    pub destination: String,
    pub gateway: String,
    pub genmask: String,
    pub flags: String,
    pub iface: String,
}

#[derive(Default)]
struct RouteCache {
    rows: Vec<RouteRow>,
    raw: String,
    fetched_at: Option<Instant>,
    error: Option<String>,
    source_key: String,
    loading: bool,
    rx: Option<mpsc::Receiver<FetchResult>>,
}

struct FetchResult {
    source_key: String,
    result: Result<(Vec<RouteRow>, String), String>,
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

pub fn show_panel(ui: &mut Ui, remote: Option<&RemoteSession>, vault_path: Option<&Path>) {
    let source_key = remote
        .map(|r| format!("{}@{}", r.config.username, r.config.host))
        .unwrap_or_else(|| "local".into());

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
            if !guard.rows.is_empty() {
                egui::Grid::new("route_grid")
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
                        for r in &guard.rows {
                            ui.label(&r.destination);
                            ui.label(&r.gateway);
                            ui.label(&r.genmask);
                            ui.label(&r.flags);
                            ui.label(&r.iface);
                            ui.end_row();
                        }
                    });
            } else if guard.loading {
                ui.label(i18n::t("routes.loading"));
            } else if !guard.raw.is_empty() {
                ui.label(RichText::new(i18n::t("routes.raw")).weak());
                ui.add_space(6.0);
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.label(RichText::new(&guard.raw).size(12.0));
                });
            } else {
                ui.label(i18n::t("routes.empty"));
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
                Ok((rows, raw)) => {
                    guard.rows = rows;
                    guard.raw = raw;
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
        guard.rows.clear();
        guard.raw.clear();
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
            let result = if let Some(session) = remote {
                fetch_remote_routes(&session, vault_path.as_deref())
            } else {
                fetch_routes()
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

fn fetch_remote_routes(
    session: &RemoteSession,
    vault_path: Option<&Path>,
) -> Result<(Vec<RouteRow>, String), String> {
    let vault = vault_path.and_then(|p| Vault::open(p).ok());
    let cmd = "ip -4 route 2>/dev/null || route -n 2>/dev/null || cat /proc/net/route";
    let raw = session
        .run_command(vault.as_ref(), cmd)
        .map_err(|e| e.to_string())?;
    let cleaned = strip_ansi(&raw);
    let rows = parse_unix_route(&cleaned);
    Ok((rows, cleaned))
}

fn fetch_routes() -> Result<(Vec<RouteRow>, String), String> {
    #[cfg(windows)]
    {
        let output = Command::new("route")
            .arg("print")
            .output()
            .map_err(|e| e.to_string())?;
        let raw = decode_command_output(&output.stdout);
        let rows = parse_windows_route(&raw);
        Ok((rows, raw))
    }
    #[cfg(not(windows))]
    {
        let output = Command::new("sh")
            .args([
                "-c",
                "ip route 2>/dev/null || netstat -rn 2>/dev/null || route -n",
            ])
            .output()
            .map_err(|e| e.to_string())?;
        let raw = decode_command_output(&output.stdout);
        let rows = parse_unix_route(&raw);
        Ok((rows, raw))
    }
}

#[cfg(windows)]
fn parse_windows_route(raw: &str) -> Vec<RouteRow> {
    let mut rows = Vec::new();
    let mut in_ipv4 = false;
    for line in raw.lines() {
        let t = line.trim();
        if t.starts_with("Active Routes:")
            || t.contains("网络目标")
            || t.contains("Network Destination")
            || t.contains("活动路由")
        {
            in_ipv4 = true;
            continue;
        }
        if t.starts_with("Persistent Routes:")
            || t.starts_with("IPv6 Route Table")
            || t.contains("永久路由")
            || t.contains("IPv6 路由表")
        {
            in_ipv4 = false;
            continue;
        }
        if !in_ipv4
            || t.is_empty()
            || t.starts_with("Network Destination")
            || t.starts_with("网络目标")
            || t.starts_with("网络地址")
        {
            continue;
        }
        let parts: Vec<&str> = t.split_whitespace().collect();
        if parts.len() >= 5 && parts[0].contains('.') {
            rows.push(RouteRow {
                destination: parts[0].into(),
                gateway: parts.get(2).unwrap_or(&"").to_string(),
                genmask: parts.get(1).unwrap_or(&"").to_string(),
                flags: parts.get(3).unwrap_or(&"").to_string(),
                iface: parts
                    .get(parts.len().saturating_sub(1))
                    .unwrap_or(&"")
                    .to_string(),
            });
        }
    }
    rows
}

fn parse_unix_route(raw: &str) -> Vec<RouteRow> {
    let mut rows = Vec::new();
    for line in raw.lines() {
        let t = line.trim();
        if t.is_empty()
            || t.starts_with("Kernel")
            || t.starts_with("Destination")
            || t.starts_with("password")
            || t.contains("Permission denied")
        {
            continue;
        }
        let parts: Vec<&str> = t.split_whitespace().collect();
        if parts.len() >= 3
            && (parts[0] == "default" || parts[0].contains('/') || parts[0].contains('.'))
        {
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
            rows.push(RouteRow {
                destination: parts[0].into(),
                gateway,
                genmask: String::new(),
                flags: String::new(),
                iface,
            });
        }
    }
    rows
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
