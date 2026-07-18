use crate::i18n;
use egui::Ui;

pub fn show(
    ui: &mut Ui,
    status: &str,
    conn_count: usize,
    software_renderer: bool,
    zmodem_busy: bool,
    zmodem_progress: Option<f32>,
    on_cancel_zmodem: impl FnOnce(),
) {
    ui.horizontal(|ui| {
        ui.label(status);
        if let Some(frac) = zmodem_progress {
            let bar = egui::ProgressBar::new(frac)
                .desired_width(120.0)
                .show_percentage();
            ui.add(bar);
        } else if zmodem_busy {
            // Indeterminate-ish: thin busy bar while waiting on a dialog.
            ui.add(
                egui::ProgressBar::new(0.0)
                    .desired_width(120.0)
                    .animate(true),
            );
        }
        if zmodem_busy {
            if ui
                .small_button(i18n::t("zmodem.cancel"))
                .on_hover_text(i18n::t("zmodem.cancel.tip"))
                .clicked()
            {
                on_cancel_zmodem();
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(format!("{} {conn_count}", i18n::t("status.connections")));
            ui.separator();
            ui.label(i18n::t("status.stage"));
            if software_renderer {
                ui.separator();
                ui.colored_label(
                    egui::Color32::from_rgb(184, 112, 24),
                    i18n::t("status.software_renderer"),
                )
                .on_hover_text(i18n::t("status.software_renderer_tip"));
            }
        });
    });
}
