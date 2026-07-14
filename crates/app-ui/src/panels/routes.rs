//! Routing table panel.

use crate::i18n;
use egui::{RichText, Ui};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

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
}

fn cache() -> &'static Mutex<RouteCache> {
    static CACHE: OnceLock<Mutex<RouteCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(RouteCache::default()))
}

pub fn show_panel(ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.heading(i18n::t("routes.title"));
        if ui.button(i18n::t("routes.refresh")).clicked() {
            refresh(true);
        }
    });
    ui.separator();

    refresh(false);

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
                        ui.label(RichText::new(i18n::t("routes.destination")).strong());
                        ui.label(RichText::new(i18n::t("routes.gateway")).strong());
                        ui.label(RichText::new(i18n::t("routes.mask")).strong());
                        ui.label(RichText::new(i18n::t("routes.flags")).strong());
                        ui.label(RichText::new(i18n::t("routes.iface")).strong());
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
            } else if !guard.raw.is_empty() {
                ui.label(RichText::new(i18n::t("routes.raw")).weak());
                ui.add_space(6.0);
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.monospace(&guard.raw);
                });
            } else {
                ui.label(i18n::t("routes.empty"));
            }
        });
}

fn refresh(force: bool) {
    let mut guard = cache().lock().unwrap_or_else(|e| e.into_inner());
    if !force {
        if let Some(at) = guard.fetched_at {
            if at.elapsed() < Duration::from_secs(5) {
                return;
            }
        }
    }

    match fetch_routes() {
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

fn fetch_routes() -> Result<(Vec<RouteRow>, String), String> {
    #[cfg(windows)]
    {
        let output = Command::new("route")
            .arg("print")
            .output()
            .map_err(|e| e.to_string())?;
        let raw = String::from_utf8_lossy(&output.stdout).into_owned();
        let rows = parse_windows_route(&raw);
        Ok((rows, raw))
    }
    #[cfg(not(windows))]
    {
        let output = Command::new("sh")
            .args(["-c", "ip route 2>/dev/null || netstat -rn 2>/dev/null || route -n"])
            .output()
            .map_err(|e| e.to_string())?;
        let raw = String::from_utf8_lossy(&output.stdout).into_owned();
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
        if t.starts_with("Active Routes:") || t.contains("网络目标") || t.contains("Network Destination")
        {
            in_ipv4 = true;
            continue;
        }
        if t.starts_with("Persistent Routes:") || t.starts_with("IPv6 Route Table") {
            in_ipv4 = false;
            continue;
        }
        if !in_ipv4 || t.is_empty() || t.starts_with("Network Destination") || t.starts_with("网络目标")
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
                iface: parts.get(parts.len().saturating_sub(1)).unwrap_or(&"").to_string(),
            });
        }
    }
    rows
}

#[cfg(not(windows))]
fn parse_unix_route(raw: &str) -> Vec<RouteRow> {
    let mut rows = Vec::new();
    for line in raw.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with("Kernel") || t.starts_with("Destination") {
            continue;
        }
        let parts: Vec<&str> = t.split_whitespace().collect();
        if parts.len() >= 3 {
            // `ip route`: "default via 1.2.3.4 dev eth0"
            if parts[0] == "default" || parts[0].contains('/') || parts[0].contains('.') {
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
    }
    rows
}
