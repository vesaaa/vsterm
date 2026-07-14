use crate::i18n;
use crate::metrics::HostSnapshot;
use egui::{Align2, Color32, FontId, Layout, RichText, Sense, Ui, UiBuilder};
use egui_plot::{Line, Plot, PlotPoint, PlotPoints};

const GAUGE_ROW_H: f32 = 22.0;
const GAUGE_BLOCK_H: f32 = GAUGE_ROW_H * 3.0;
const NET_HEADER_H: f32 = 22.0;
const NET_PLOT_H: f32 = 88.0;
const NET_BLOCK_H: f32 = NET_HEADER_H + NET_PLOT_H;
const DISK_BLOCK_H: f32 = 118.0;
const PROC_ROW_H: f32 = 18.0;
const GAP: f32 = 4.0;
const LABEL_COLOR: Color32 = Color32::from_rgb(52, 56, 62);
const HEADER_COLOR: Color32 = Color32::from_rgb(100, 105, 115);

pub fn show(
    ui: &mut Ui,
    snap: &HostSnapshot,
    selected_nic: &mut Option<String>,
    has_connection: bool,
    fetch_error: Option<&str>,
) {
    if !has_connection {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new(i18n::t("monitor.no_connection")).weak());
        });
        return;
    }

    // Default to a primary NIC (ens*/eth*/…) so the chart is usable without picking.
    let nic_stale = selected_nic
        .as_ref()
        .is_some_and(|cur| !snap.nics.iter().any(|n| n.name == *cur));
    if selected_nic.is_none() || nic_stale {
        *selected_nic = HostSnapshot::prefer_primary_nic(&snap.nics, snap.default_if.as_deref());
    }

    // Avoid automatic item_spacing eating our fixed-height budget (was clipping storage).
    ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);

    let w = ui.available_width().max(1.0);
    let total_h = ui.available_height().max(1.0);

    if snap.hostname.is_empty() {
        ui.vertical(|ui| {
            ui.label(RichText::new(i18n::t("monitor.loading")).weak());
            if let Some(err) = fetch_error {
                ui.add_space(6.0);
                ui.colored_label(Color32::from_rgb(200, 80, 80), err);
            }
        });
        return;
    }

    let fixed_tail = NET_BLOCK_H + DISK_BLOCK_H + GAP * 2.0;
    let mid_h = (total_h - GAUGE_BLOCK_H - fixed_tail - GAP).max(48.0);

    place_block(ui, w, GAUGE_BLOCK_H, |ui| usage_bars(ui, snap, w));
    ui.add_space(GAP);
    place_block(ui, w, mid_h, |ui| process_table(ui, snap, w));
    ui.add_space(GAP);
    place_block(ui, w, NET_BLOCK_H, |ui| {
        network_section(ui, snap, selected_nic, w);
    });
    ui.add_space(GAP);
    place_block(ui, w, DISK_BLOCK_H, |ui| storage_section(ui, snap, w));
}

fn place_block(ui: &mut Ui, w: f32, h: f32, add_contents: impl FnOnce(&mut Ui)) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), Sense::hover());
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::top_down(egui::Align::Min)),
        |ui| {
            ui.set_clip_rect(rect);
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);
            ui.set_max_width(w);
            add_contents(ui);
        },
    );
}

fn usage_bars(ui: &mut Ui, snap: &HostSnapshot, w: f32) {
    resource_row(
        ui,
        &i18n::t("monitor.cpu"),
        snap.cpu_usage,
        format!("{:.0}%", snap.cpu_usage.clamp(0.0, 100.0)),
        Color32::from_rgb(50, 150, 220),
        w,
    );
    resource_row(
        ui,
        &i18n::t("monitor.memory"),
        snap.mem_pct(),
        HostSnapshot::format_ratio_compact(snap.mem_used, snap.mem_total),
        Color32::from_rgb(200, 80, 160),
        w,
    );
    resource_row(
        ui,
        &i18n::t("monitor.swap"),
        snap.swap_pct(),
        HostSnapshot::format_ratio_compact(snap.swap_used, snap.swap_total),
        Color32::from_rgb(220, 140, 40),
        w,
    );
}

fn resource_row(ui: &mut Ui, title: &str, pct: f32, tail: String, color: Color32, w: f32) {
    let label_w = 36.0;
    let (row_rect, _) = ui.allocate_exact_size(egui::vec2(w, GAUGE_ROW_H), Sense::hover());
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(row_rect)
            .layout(Layout::left_to_right(egui::Align::Center)),
        |ui| {
            ui.set_clip_rect(row_rect);
            ui.add_sized(
                [label_w, GAUGE_ROW_H],
                egui::Label::new(RichText::new(title).size(12.0).color(LABEL_COLOR)),
            );
            let bar_w = (ui.available_width()).max(40.0);
            let (rect, _) =
                ui.allocate_exact_size(egui::vec2(bar_w, GAUGE_ROW_H - 4.0), Sense::hover());

            let bg = Color32::from_rgb(230, 232, 236);
            ui.painter().rect_filled(rect, 0.0, bg);

            let pct = pct.clamp(0.0, 100.0) / 100.0;
            if pct > 0.0 {
                let mut fill = rect;
                fill.max.x = rect.min.x + rect.width() * pct;
                ui.painter().rect_filled(fill, 0.0, color);
            }

            ui.painter().text(
                egui::pos2(rect.right() - 3.0, rect.center().y),
                Align2::RIGHT_CENTER,
                tail,
                FontId::monospace(10.5),
                Color32::from_rgb(30, 32, 38),
            );
        },
    );
}

fn process_table(ui: &mut Ui, snap: &HostSnapshot, w: f32) {
    let stroke = Color32::from_rgb(210, 214, 220);
    let (inner, _) = ui.allocate_exact_size(egui::vec2(w, ui.available_height()), Sense::hover());
    ui.painter().rect_stroke(
        inner,
        0.0,
        egui::Stroke::new(1.0_f32, stroke),
        egui::StrokeKind::Inside,
    );

    let pad = 4.0;
    let body = inner.shrink2(egui::vec2(pad, 2.0));
    let rows_fit = ((body.height() - PROC_ROW_H) / PROC_ROW_H).floor().max(0.0) as usize;
    let avail = body.width();

    // PID column: widen for Linux PIDs (often 5–7 digits); 36px only fit ~4.
    let pid_digits = snap
        .processes
        .iter()
        .map(|p| p.pid.to_string().len())
        .max()
        .unwrap_or(5)
        .clamp(5, 8);
    let pid_w = (pid_digits as f32) * 8.5 + 6.0;
    let cpu_w = 40.0;
    let mem_w = 72.0;
    let name_w = (avail - pid_w - cpu_w - mem_w).max(40.0);
    let col_w = [pid_w, name_w, cpu_w, mem_w];
    let name_chars = ((name_w / 7.5).floor() as usize).clamp(4, 24);

    ui.scope_builder(
        UiBuilder::new()
            .max_rect(body)
            .layout(Layout::top_down(egui::Align::Min)),
        |ui| {
            ui.set_clip_rect(body);
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            proc_row(
                ui,
                &col_w,
                header_text(i18n::t("monitor.pid")),
                header_text(i18n::t("monitor.name")),
                header_text(i18n::t("monitor.cpu_pct")),
                header_text(i18n::t("monitor.mem")),
                true,
            );
            for (i, p) in snap.processes.iter().take(rows_fit).enumerate() {
                proc_row(
                    ui,
                    &col_w,
                    RichText::new(p.pid.to_string()).size(12.0),
                    RichText::new(truncate(&p.name, name_chars)).size(12.0),
                    RichText::new(format!("{:.1}", p.cpu)).size(12.0),
                    RichText::new(HostSnapshot::format_bytes(p.mem_bytes)).size(12.0),
                    i % 2 == 1,
                );
            }
        },
    );
}

fn proc_row(
    ui: &mut Ui,
    col_w: &[f32; 4],
    a: RichText,
    b: RichText,
    c: RichText,
    d: RichText,
    stripe: bool,
) {
    let (row_rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), PROC_ROW_H),
        Sense::hover(),
    );
    if stripe {
        ui.painter()
            .rect_filled(row_rect, 0.0, Color32::from_rgb(242, 244, 247));
    }
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(row_rect)
            .layout(Layout::left_to_right(egui::Align::Center)),
        |ui| {
            ui.set_clip_rect(row_rect);
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            ui.add_sized([col_w[0], PROC_ROW_H], egui::Label::new(a).truncate());
            ui.add_sized([col_w[1], PROC_ROW_H], egui::Label::new(b).truncate());
            ui.add_sized([col_w[2], PROC_ROW_H], egui::Label::new(c).truncate());
            ui.add_sized([col_w[3], PROC_ROW_H], egui::Label::new(d).truncate());
        },
    );
}

fn network_section(
    ui: &mut Ui,
    snap: &HostSnapshot,
    selected: &mut Option<String>,
    w: f32,
) {
    let (hdr, _) = ui.allocate_exact_size(egui::vec2(w, NET_HEADER_H), Sense::hover());
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(hdr)
            .layout(Layout::left_to_right(egui::Align::Center)),
        |ui| {
            ui.set_clip_rect(hdr);
            if let Some(name) = selected.as_ref() {
                if let Some(nic) = snap.nics.iter().find(|n| n.name == *name) {
                    ui.label(
                        RichText::new(format!("↓ {}", HostSnapshot::format_bps(nic.rx_bps)))
                            .color(Color32::from_rgb(40, 140, 80))
                            .size(12.0),
                    );
                    ui.label(
                        RichText::new(format!("↑ {}", HostSnapshot::format_bps(nic.tx_bps)))
                            .color(Color32::from_rgb(40, 110, 190))
                            .size(12.0),
                    );
                }
            }

            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                let preview = selected
                    .as_deref()
                    .map(|s| truncate(s, 12))
                    .unwrap_or_default();
                let combo_w = (w * 0.40).clamp(72.0, 130.0);
                egui::ComboBox::from_id_salt("nic_combo")
                    .width(combo_w)
                    .selected_text(RichText::new(preview).size(12.0).color(LABEL_COLOR))
                    .show_ui(ui, |ui| {
                        for nic in &snap.nics {
                            if ui
                                .selectable_value(
                                    selected,
                                    Some(nic.name.clone()),
                                    RichText::new(&nic.name).size(12.0),
                                )
                                .clicked()
                            {
                                *selected = Some(nic.name.clone());
                            }
                        }
                    });
            });
        },
    );

    let (plot_rect, _) = ui.allocate_exact_size(egui::vec2(w, NET_PLOT_H), Sense::hover());
    if let Some(name) = selected.clone() {
        if let Some(hist) = snap.net_history.get(&name) {
            // Plot units: KB/s
            let y_peak = hist
                .iter()
                .map(|(rx, tx)| rx.max(*tx) / 1024.0)
                .fold(0.0_f64, f64::max);
            let rx: PlotPoints = hist
                .iter()
                .enumerate()
                .map(|(i, (rx, _))| [i as f64, *rx / 1024.0])
                .collect();
            let tx: PlotPoints = hist
                .iter()
                .enumerate()
                .map(|(i, (_, tx))| [i as f64, *tx / 1024.0])
                .collect();

            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(plot_rect)
                    .layout(Layout::top_down(egui::Align::Min)),
                |ui| {
                    ui.set_clip_rect(plot_rect);
                    let plot_resp = Plot::new("net_plot")
                        .width(plot_rect.width())
                        .height(plot_rect.height())
                        .allow_zoom(false)
                        .allow_scroll(false)
                        .allow_drag(false)
                        .allow_boxed_zoom(false)
                        .show_axes(false)
                        .show_grid(true)
                        .set_margin_fraction(egui::vec2(0.0, 0.02))
                        .include_y(0.0)
                        .include_y(y_peak.max(1.0))
                        .show(ui, |plot_ui| {
                            plot_ui.line(
                                Line::new(rx)
                                    .name("↓")
                                    .color(Color32::from_rgb(40, 160, 90)),
                            );
                            plot_ui.line(
                                Line::new(tx)
                                    .name("↑")
                                    .color(Color32::from_rgb(40, 120, 200)),
                            );
                        });

                    // Y labels inside the plot, just to the right of the left edge
                    // (top + mid only) — keeps the chart full-bleed without an outer axis gutter.
                    let bounds = plot_resp.transform.bounds();
                    let [xmin, ymin] = bounds.min();
                    let [_, ymax] = bounds.max();
                    let y_mid = (ymin + ymax) * 0.5;
                    let top_pos = plot_resp
                        .transform
                        .position_from_point(&PlotPoint::new(xmin, ymax));
                    let mid_pos = plot_resp
                        .transform
                        .position_from_point(&PlotPoint::new(xmin, y_mid));
                    let label_color = Color32::from_rgb(90, 95, 105);
                    let font = FontId::proportional(10.0);
                    let x = plot_rect.min.x + 3.0;
                    ui.painter().text(
                        egui::pos2(x, top_pos.y + 1.0),
                        Align2::LEFT_TOP,
                        HostSnapshot::format_bps(ymax * 1024.0),
                        font.clone(),
                        label_color,
                    );
                    ui.painter().text(
                        egui::pos2(x, mid_pos.y),
                        Align2::LEFT_CENTER,
                        HostSnapshot::format_bps(y_mid * 1024.0),
                        font,
                        label_color,
                    );
                },
            );
        }
    }
}

fn storage_section(ui: &mut Ui, snap: &HostSnapshot, w: f32) {
    let stroke = Color32::from_rgb(210, 214, 220);
    let (inner, _) = ui.allocate_exact_size(egui::vec2(w, ui.available_height()), Sense::hover());
    ui.painter().rect_stroke(
        inner,
        0.0,
        egui::Stroke::new(1.0_f32, stroke),
        egui::StrokeKind::Inside,
    );

    let pad = 4.0;
    let body = inner.shrink2(egui::vec2(pad, 2.0));
    let avail = body.width();

    // df -h style: Mount | Used/Total | Use%
    let mount_w = (avail * 0.30).clamp(52.0, 96.0);
    let pct_w = 42.0;
    let cap_w = (avail - mount_w - pct_w).max(72.0);
    let col_w = [mount_w, cap_w, pct_w];
    let mount_chars = ((mount_w / 7.0).floor() as usize).clamp(5, 18);

    ui.scope_builder(
        UiBuilder::new()
            .max_rect(body)
            .layout(Layout::top_down(egui::Align::Min)),
        |ui| {
            ui.set_clip_rect(body);
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            disk_row(
                ui,
                &col_w,
                header_text(i18n::t("monitor.fs")),
                header_text(i18n::t("monitor.capacity")).size(11.0),
                header_text(i18n::t("monitor.use_pct")),
                true,
            );
            let list_h = (body.height() - PROC_ROW_H).max(PROC_ROW_H);
            egui::ScrollArea::vertical()
                .id_salt("monitor_disk_scroll")
                .max_height(list_h)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if snap.disks.is_empty() {
                        ui.label(
                            RichText::new(i18n::t("monitor.disk_empty"))
                                .size(11.0)
                                .weak(),
                        );
                        return;
                    }
                    for (i, d) in snap.disks.iter().enumerate() {
                        let used = d.total.saturating_sub(d.available);
                        let pct = if d.total == 0 {
                            0.0
                        } else {
                            used as f64 / d.total as f64 * 100.0
                        };
                        disk_row(
                            ui,
                            &col_w,
                            RichText::new(truncate(&d.mount, mount_chars)),
                            RichText::new(HostSnapshot::format_ratio_compact(used, d.total))
                                .monospace()
                                .size(11.0),
                            RichText::new(format!("{pct:.0}%")),
                            i % 2 == 1,
                        );
                    }
                });
        },
    );
}

fn disk_row(
    ui: &mut Ui,
    col_w: &[f32; 3],
    a: RichText,
    b: RichText,
    c: RichText,
    stripe: bool,
) {
    let (row_rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), PROC_ROW_H),
        Sense::hover(),
    );
    if stripe {
        ui.painter()
            .rect_filled(row_rect, 0.0, Color32::from_rgb(242, 244, 247));
    }
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(row_rect)
            .layout(Layout::left_to_right(egui::Align::Center)),
        |ui| {
            ui.set_clip_rect(row_rect);
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            ui.add_sized([col_w[0], PROC_ROW_H], egui::Label::new(a).truncate());
            ui.add_sized([col_w[1], PROC_ROW_H], egui::Label::new(b).truncate());
            ui.add_sized([col_w[2], PROC_ROW_H], egui::Label::new(c).truncate());
        },
    );
}

fn header_text(text: impl Into<String>) -> RichText {
    RichText::new(text).size(12.0).color(HEADER_COLOR)
}

fn truncate(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{t}…")
    }
}
