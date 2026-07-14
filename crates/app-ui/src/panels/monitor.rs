use crate::i18n;
use crate::metrics::{HostSnapshot, MetricsService};
use egui::{Align2, Color32, FontId, Layout, RichText, Sense, Ui, UiBuilder};
use egui_plot::{Line, Plot, PlotPoints};

const GAUGE_ROW_H: f32 = 22.0;
const GAUGE_BLOCK_H: f32 = GAUGE_ROW_H * 3.0;
const NET_HEADER_H: f32 = 22.0;
const NET_PLOT_H: f32 = 88.0;
const NET_BLOCK_H: f32 = NET_HEADER_H + NET_PLOT_H;
const DISK_BLOCK_H: f32 = 118.0;
const PROC_ROW_H: f32 = 18.0;
const GAP: f32 = 4.0;

pub fn show(ui: &mut Ui, metrics: &MetricsService, has_connection: bool) {
    if !has_connection {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new(i18n::t("monitor.no_connection")).weak());
        });
        return;
    }

    // Avoid automatic item_spacing eating our fixed-height budget (was clipping storage).
    ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);

    let w = ui.available_width().max(1.0);
    let total_h = ui.available_height().max(1.0);

    let snap = metrics.snapshot();
    let mut selected = metrics.selected_nic();

    let fixed_tail = NET_BLOCK_H + DISK_BLOCK_H + GAP * 2.0;
    let mid_h = (total_h - GAUGE_BLOCK_H - fixed_tail - GAP).max(48.0);

    place_block(ui, w, GAUGE_BLOCK_H, |ui| usage_bars(ui, &snap, w));
    ui.add_space(GAP);
    place_block(ui, w, mid_h, |ui| process_table(ui, &snap, w));
    ui.add_space(GAP);
    place_block(ui, w, NET_BLOCK_H, |ui| {
        network_section(ui, metrics, &snap, &mut selected, w);
    });
    ui.add_space(GAP);
    place_block(ui, w, DISK_BLOCK_H, |ui| storage_section(ui, &snap, w));
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
                egui::Label::new(RichText::new(title).strong().size(13.0)),
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

    // Columns sized from the actual body width so the rightmost "mem" is not clipped.
    let pid_w = 36.0;
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
                RichText::new(i18n::t("monitor.pid")).strong(),
                RichText::new(i18n::t("monitor.name")).strong(),
                RichText::new(i18n::t("monitor.cpu_pct")).strong(),
                RichText::new(i18n::t("monitor.mem")).strong(),
                true,
            );
            for (i, p) in snap.processes.iter().take(rows_fit).enumerate() {
                proc_row(
                    ui,
                    &col_w,
                    RichText::new(p.pid.to_string()),
                    RichText::new(truncate(&p.name, name_chars)),
                    RichText::new(format!("{:.1}", p.cpu)),
                    RichText::new(HostSnapshot::format_bytes(p.mem_bytes)),
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
    metrics: &MetricsService,
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
                    .selected_text(preview)
                    .show_ui(ui, |ui| {
                        for nic in &snap.nics {
                            if ui
                                .selectable_value(selected, Some(nic.name.clone()), &nic.name)
                                .clicked()
                            {
                                metrics.set_selected_nic(Some(nic.name.clone()));
                            }
                        }
                    });
            });
        },
    );

    let (plot_rect, _) = ui.allocate_exact_size(egui::vec2(w, NET_PLOT_H), Sense::hover());
    if let Some(name) = selected.clone() {
        if let Some(hist) = snap.net_history.get(&name) {
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
                    Plot::new("net_plot")
                        .width(plot_rect.width())
                        .height(plot_rect.height())
                        .allow_zoom(false)
                        .allow_scroll(false)
                        .allow_drag(false)
                        .include_y(0.0)
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
    let rows_fit = ((body.height() - PROC_ROW_H) / PROC_ROW_H).floor().max(0.0) as usize;
    let avail = body.width();

    let mount_w = (avail * 0.28).clamp(48.0, 86.0);
    let pct_w = 48.0;
    let cap_w = (avail - mount_w - pct_w).max(80.0);
    let col_w = [mount_w, cap_w, pct_w];
    let mount_chars = ((mount_w / 7.5).floor() as usize).clamp(4, 16);

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
                RichText::new(i18n::t("monitor.fs")).strong(),
                RichText::new(i18n::t("monitor.capacity")).strong().size(11.0),
                RichText::new(i18n::t("monitor.use_pct")).strong(),
                true,
            );
            for (i, d) in snap.disks.iter().take(rows_fit).enumerate() {
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

fn truncate(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{t}…")
    }
}
