use crate::i18n;
use crate::metrics::HostSnapshot;
use egui::{Color32, RichText, Ui};

/// Full-panel system information (fills the main column).
pub fn show_panel(ui: &mut Ui, snap: Option<&HostSnapshot>, fetch_error: Option<&str>) {
    let Some(snap) = snap else {
        ui.centered_and_justified(|ui| {
            ui.label(i18n::t("sysinfo.unavailable"));
        });
        return;
    };

    if snap.hostname.is_empty() {
        ui.label(RichText::new(i18n::t("monitor.loading")).weak());
        if let Some(err) = fetch_error {
            ui.add_space(8.0);
            ui.colored_label(Color32::from_rgb(200, 80, 80), err);
        }
        return;
    }

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
                    row(ui, &i18n::t("sysinfo.os"), &snap.os_name);
                    row(ui, &i18n::t("sysinfo.kernel"), &snap.kernel);
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

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(6.0);
            ui.label(
                RichText::new(i18n::t("sysinfo.nics"))
                    .size(13.0)
                    .color(Color32::from_rgb(70, 74, 82)),
            );
            ui.add_space(4.0);
            egui::Grid::new("nic_grid")
                .num_columns(3)
                .striped(true)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(i18n::t("monitor.interface"))
                            .size(12.0)
                            .color(Color32::from_rgb(100, 105, 115)),
                    );
                    ui.label(
                        RichText::new("RX")
                            .size(12.0)
                            .color(Color32::from_rgb(100, 105, 115)),
                    );
                    ui.label(
                        RichText::new("TX")
                            .size(12.0)
                            .color(Color32::from_rgb(100, 105, 115)),
                    );
                    ui.end_row();
                    for nic in &snap.nics {
                        ui.label(&nic.name);
                        ui.label(HostSnapshot::format_bytes(nic.rx_bytes));
                        ui.label(HostSnapshot::format_bytes(nic.tx_bytes));
                        ui.end_row();
                    }
                });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(6.0);
            ui.label(
                RichText::new(i18n::t("sysinfo.disks"))
                    .size(13.0)
                    .color(Color32::from_rgb(70, 74, 82)),
            );
            ui.add_space(4.0);

            // df -h style table
            egui::Grid::new("disk_grid")
                .num_columns(5)
                .striped(true)
                .spacing([14.0, 4.0])
                .show(ui, |ui| {
                    for h in [
                        i18n::t("sysinfo.disk.fs"),
                        i18n::t("sysinfo.disk.size"),
                        i18n::t("sysinfo.disk.used"),
                        i18n::t("sysinfo.disk.avail"),
                        i18n::t("sysinfo.disk.use"),
                    ] {
                        ui.label(
                            RichText::new(h)
                                .size(12.0)
                                .color(Color32::from_rgb(100, 105, 115)),
                        );
                    }
                    ui.end_row();
                    for d in &snap.disks {
                        let used = d.total.saturating_sub(d.available);
                        let pct = if d.total == 0 {
                            0.0
                        } else {
                            used as f64 / d.total as f64 * 100.0
                        };
                        ui.label(format!("{} ({})", d.mount, d.fs));
                        ui.label(HostSnapshot::format_bytes(d.total));
                        ui.label(HostSnapshot::format_bytes(used));
                        ui.label(HostSnapshot::format_bytes(d.available));
                        ui.label(format!("{pct:.0}%"));
                        ui.end_row();
                    }
                });
        });
}

fn row(ui: &mut Ui, key: &str, value: &str) {
    ui.label(RichText::new(key).size(12.0).color(Color32::from_rgb(100, 105, 115)));
    ui.label(value);
    ui.end_row();
}
