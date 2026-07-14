use crate::commands::CommandBook;
use crate::i18n;
use crate::ui_icon::{self, Icon};
use egui::{Color32, RichText, Ui};

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

/// Vertical space the bottom strip needs (tabs + separator + body).
pub fn reserved_height(state: &BottomPanelState) -> f32 {
    // Tab row (~22) + separator (~6) + body. Keep in sync with `show`.
    28.0 + 6.0 + state.height
}

/// Renders bottom tools into the caller's allocated region (must already be non-overlapping).
pub fn show(ui: &mut Ui, state: &mut BottomPanelState, book: &CommandBook) -> Option<String> {
    let mut send_cmd = None;
    let fixed_h = state.height;
    ui.set_clip_rect(ui.max_rect());

    ui.horizontal(|ui| {
        let files = ui.selectable_value(&mut state.tab, BottomTab::Files, i18n::t("tab.files"));
        let cmds =
            ui.selectable_value(&mut state.tab, BottomTab::Commands, i18n::t("tab.commands"));
        if files.has_focus() {
            files.surrender_focus();
        }
        if cmds.has_focus() {
            cmds.surrender_focus();
        }
    });
    ui.separator();

    let body = egui::Frame::NONE.show(ui, |ui| {
        ui.set_min_height(fixed_h);
        ui.set_max_height(fixed_h);
        ui.set_clip_rect(ui.max_rect());
        match state.tab {
            BottomTab::Files => {
                ui.label(RichText::new(i18n::t("bottom.files.hint")).weak().small());
                ui.add_space(4.0);
                let list_h = (fixed_h - 70.0).max(60.0);
                ui.columns(2, |cols| {
                    cols[0].group(|ui| {
                        ui.label(
                            RichText::new(i18n::t("bottom.files.local"))
                                .size(12.0)
                                .color(Color32::from_rgb(100, 105, 115)),
                        );
                        ui.text_edit_singleline(&mut state.local_path);
                        list_dir_preview(ui, &state.local_path, list_h - 40.0);
                    });
                    cols[1].group(|ui| {
                        ui.label(
                            RichText::new(i18n::t("bottom.files.remote"))
                                .size(12.0)
                                .color(Color32::from_rgb(100, 105, 115)),
                        );
                        ui.text_edit_singleline(&mut state.remote_path);
                        ui.set_min_height(list_h - 40.0);
                        ui.label(RichText::new("SFTP …").weak());
                    });
                });
                ui.horizontal(|ui| {
                    let up = ui.button(i18n::t("bottom.files.upload"));
                    let down = ui.button(i18n::t("bottom.files.download"));
                    if up.has_focus() {
                        up.surrender_focus();
                    }
                    if down.has_focus() {
                        down.surrender_focus();
                    }
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
                                    let resp = ui
                                        .add(
                                            egui::Button::new(&cmd.name)
                                                .min_size([140.0, 24.0].into()),
                                        )
                                        .on_hover_text(
                                            cmd.description
                                                .clone()
                                                .unwrap_or_else(|| cmd.command.clone()),
                                        );
                                    if resp.clicked_by(egui::PointerButton::Primary) {
                                        send_cmd = Some(cmd.command.clone());
                                    }
                                    if resp.has_focus() {
                                        resp.surrender_focus();
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
                        ui_icon::labeled(
                            ui,
                            if is_dir { Icon::Folder } else { Icon::File },
                            &name,
                            13.0,
                            ui_icon::COLOR_MUTED,
                        );
                    }
                }
                Err(err) => {
                    ui.label(RichText::new(err.to_string()).weak());
                }
            }
        });
}
