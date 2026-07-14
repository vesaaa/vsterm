use crate::i18n;
use egui::Ui;

pub fn show(ui: &mut Ui, status: &str, conn_count: usize) {
    ui.horizontal(|ui| {
        ui.label(status);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(format!("{} {conn_count}", i18n::t("status.connections")));
            ui.separator();
            ui.label(i18n::t("status.stage"));
        });
    });
}
