use crate::commands::CommandBook;
use crate::i18n;
use egui::{RichText, Ui};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BottomTab {
    #[default]
    Files,
    Commands,
}

pub struct BottomPanelState {
    pub tab: BottomTab,
    /// Fixed content height shared by Files / Commands.
    pub height: f32,
    pub local_path: String,
    pub remote_path: String,
}

impl Default for BottomPanelState {
    fn default() -> Self {
        Self {
            tab: BottomTab::Files,
            height: 180.0,
            local_path: std::env::var("USERPROFILE")
                .or_else(|_| std::env::var("HOME"))
                .unwrap_or_else(|_| ".".into()),
            remote_path: "/".into(),
        }
    }
}

/// Renders bottom tools into a fixed-height region. Height is NOT switched with tabs.
pub fn show(ui: &mut Ui, state: &mut BottomPanelState, book: &CommandBook) -> Option<String> {
    let mut send_cmd = None;
    let fixed_h = state.height;

    ui.horizontal(|ui| {
        ui.selectable_value(&mut state.tab, BottomTab::Files, i18n::t("tab.files"));
        ui.selectable_value(&mut state.tab, BottomTab::Commands, i18n::t("tab.commands"));
    });
    ui.separator();

    // Allocate a consistent body height (Files as reference).
    let body = egui::Frame::NONE.show(ui, |ui| {
        ui.set_min_height(fixed_h);
        ui.set_max_height(fixed_h);
        match state.tab {
            BottomTab::Files => {
                ui.label(RichText::new(i18n::t("bottom.files.hint")).weak().small());
                ui.add_space(4.0);
                let list_h = (fixed_h - 70.0).max(60.0);
                ui.columns(2, |cols| {
                    cols[0].group(|ui| {
                        ui.label(RichText::new(i18n::t("bottom.files.local")).strong());
                        ui.text_edit_singleline(&mut state.local_path);
                        list_dir_preview(ui, &state.local_path, list_h - 40.0);
                    });
                    cols[1].group(|ui| {
                        ui.label(RichText::new(i18n::t("bottom.files.remote")).strong());
                        ui.text_edit_singleline(&mut state.remote_path);
                        ui.set_min_height(list_h - 40.0);
                        ui.label(RichText::new("SFTP …").weak());
                    });
                });
                ui.horizontal(|ui| {
                    let _ = ui.button(i18n::t("bottom.files.upload"));
                    let _ = ui.button(i18n::t("bottom.files.download"));
                });
            }
            BottomTab::Commands => {
                ui.label(RichText::new(i18n::t("bottom.commands.hint")).weak().small());
                if book.commands.is_empty() {
                    ui.label(i18n::t("bottom.commands.empty"));
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("commands_scroll")
                        .max_height(fixed_h - 28.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for cmd in &book.commands {
                                ui.horizontal(|ui| {
                                    if ui
                                        .add(
                                            egui::Button::new(&cmd.name)
                                                .min_size([140.0, 24.0].into()),
                                        )
                                        .on_hover_text(
                                            cmd.description
                                                .clone()
                                                .unwrap_or_else(|| cmd.command.clone()),
                                        )
                                        .clicked()
                                    {
                                        send_cmd = Some(cmd.command.clone());
                                    }
                                    ui.label(
                                        RichText::new(cmd.command.trim_end())
                                            .small()
                                            .weak()
                                            .monospace(),
                                    );
                                });
                            }
                        });
                }
            }
        }
        // Consume remaining space so Commands tab keeps same footprint as Files.
        ui.allocate_space(egui::vec2(ui.available_width(), 0.0));
    });
    let _ = body;

    send_cmd
}

fn list_dir_preview(ui: &mut Ui, path: &str, max_h: f32) {
    egui::ScrollArea::vertical()
        .id_salt(("dir_preview", path.to_owned()))
        .max_height(max_h)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            match std::fs::read_dir(path) {
                Ok(rd) => {
                    for entry in rd.flatten().take(40) {
                        let name = entry.file_name().to_string_lossy().into_owned();
                        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                        ui.label(if is_dir {
                            format!("📁 {name}")
                        } else {
                            format!("📄 {name}")
                        });
                    }
                }
                Err(err) => {
                    ui.label(RichText::new(err.to_string()).weak());
                }
            }
        });
}
