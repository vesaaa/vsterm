use crate::i18n;
use crate::metrics::HostSnapshot;
use egui::{Color32, RichText, Ui};

/// Full-panel system information (fills the main column).
pub fn show_panel(ui: &mut Ui, snap: Option<&HostSnapshot>) {
    ui.heading(i18n::t("sysinfo.title"));
    ui.separator();

    let Some(snap) = snap else {
        ui.centered_and_justified(|ui| {
            ui.label(i18n::t("sysinfo.unavailable"));
        });
        return;
    };

    egui::ScrollArea::both()
        .id_salt("sysinfo_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("sysinfo_grid_panel")
                .num_columns(2)
                .spacing([20.0, 8.0])
                .striped(true)
                .min_col_width(120.0)
                .show(ui, |ui| {
                    row(ui, &i18n::t("sysinfo.hostname"), &snap.hostname);
                    row(
                        ui,
                        &i18n::t("sysinfo.os"),
                        &format!("{} {}", snap.os_name, snap.os_version),
                    );
                    row(ui, &i18n::t("sysinfo.kernel"), &snap.kernel);
                    row(ui, &i18n::t("sysinfo.kernel_ver"), &snap.kernel);
                    row(ui, &i18n::t("sysinfo.arch"), &snap.arch);
                    row(ui, &i18n::t("sysinfo.cpu_model"), &snap.cpu_model);
                    row(
                        ui,
                        &i18n::t("sysinfo.cpu_usage"),
                        &format!("{:.1}%", snap.cpu_usage),
                    );
                    row(
                        ui,
                        &i18n::t("monitor.memory"),
                        &format!(
                            "{} / {} ({:.0}%)",
                            HostSnapshot::format_bytes(snap.mem_used),
                            HostSnapshot::format_bytes(snap.mem_total),
                            snap.mem_pct()
                        ),
                    );
                    row(
                        ui,
                        &i18n::t("monitor.swap"),
                        &format!(
                            "{} / {} ({:.0}%)",
                            HostSnapshot::format_bytes(snap.swap_used),
                            HostSnapshot::format_bytes(snap.swap_total),
                            snap.swap_pct()
                        ),
                    );
                });

            ui.add_space(14.0);
            ui.label(RichText::new(i18n::t("sysinfo.nics")).heading());
            egui::Frame::group(ui.style()).show(ui, |ui| {
                egui::Grid::new("nic_grid")
                    .num_columns(3)
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(RichText::new(i18n::t("monitor.interface")).size(12.0).color(Color32::from_rgb(100, 105, 115)));
                        ui.label(RichText::new("RX").size(12.0).color(Color32::from_rgb(100, 105, 115)));
                        ui.label(RichText::new("TX").size(12.0).color(Color32::from_rgb(100, 105, 115)));
                        ui.end_row();
                        for nic in &snap.nics {
                            ui.label(&nic.name);
                            ui.label(HostSnapshot::format_bytes(nic.rx_bytes));
                            ui.label(HostSnapshot::format_bytes(nic.tx_bytes));
                            ui.end_row();
                        }
                    });
            });

            ui.add_space(14.0);
            ui.label(RichText::new(i18n::t("sysinfo.disks")).heading());
            egui::Frame::group(ui.style()).show(ui, |ui| {
                for d in &snap.disks {
                    let used = d.total.saturating_sub(d.available);
                    let pct = if d.total == 0 {
                        0.0
                    } else {
                        used as f64 / d.total as f64 * 100.0
                    };
                    ui.horizontal(|ui| {
                        ui.label(format!("{} ({})", d.mount, d.name));
                        ui.label(
                            RichText::new(format!(
                                "{} / {} · {:.0}% · {}",
                                HostSnapshot::format_bytes(used),
                                HostSnapshot::format_bytes(d.total),
                                pct,
                                d.fs
                            ))
                            .weak(),
                        );
                    });
                    ui.add(
                        egui::ProgressBar::new((pct as f32 / 100.0).clamp(0.0, 1.0))
                            .desired_width(ui.available_width()),
                    );
                }
            });
        });
}

fn row(ui: &mut Ui, key: &str, value: &str) {
    ui.label(RichText::new(key).size(12.0).color(Color32::from_rgb(100, 105, 115)));
    ui.label(value);
    ui.end_row();
}
