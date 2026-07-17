use crate::i18n;
use egui::Ui;

pub fn show(
    ui: &mut Ui,
    status: &str,
    conn_count: usize,
    software_renderer: bool,
    zmodem_busy: bool,
    on_cancel_zmodem: impl FnOnce(),
) {
    ui.horizontal(|ui| {
        ui.label(status);
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
