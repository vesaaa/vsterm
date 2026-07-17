//! Add / edit SSH session dialog.

use crate::i18n;
use egui::{Color32, RichText, Ui};
use session_tree::{AuthConfig, AuthType, SessionConfig, SessionTree};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Add,
    Edit,
}

#[derive(Debug, Clone)]
pub struct SessionEditorState {
    pub mode: EditorMode,
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: String,
    pub username: String,
    pub auth_type: AuthType,
    pub shell_integration: bool,
    pub password: String,
    pub save_password: bool,
    /// Edit mode: already has a vault password ref.
    pub has_saved_password: bool,
    pub private_key_path: String,
    pub passphrase: String,
    pub save_passphrase: bool,
    pub has_saved_passphrase: bool,
    pub color_tag: Option<String>,
    /// OS icon id; `None` = auto-detect after connect.
    pub icon: Option<String>,
    /// `None` = root; `Some(folder_id)` = into that folder.
    pub folder_id: Option<String>,
    pub error: Option<String>,
    pub focus_name: bool,
}

impl SessionEditorState {
    pub fn new_add(folder_id: Option<String>) -> Self {
        Self {
            mode: EditorMode::Add,
            id: String::new(),
            name: String::new(),
            host: String::new(),
            port: "22".into(),
            username: String::new(),
            auth_type: AuthType::Password,
            shell_integration: true,
            password: String::new(),
            save_password: false,
            has_saved_password: false,
            private_key_path: String::new(),
            passphrase: String::new(),
            save_passphrase: false,
            has_saved_passphrase: false,
            color_tag: None,
            icon: None,
            folder_id,
            error: None,
            focus_name: true,
        }
    }

    pub fn from_config(cfg: &SessionConfig, folder_id: Option<String>) -> Self {
        let (auth_type, password_ref, key_path, pass_ref) = match &cfg.auth {
            AuthConfig::Password { password_ref } => (
                AuthType::Password,
                password_ref.clone(),
                String::new(),
                None,
            ),
            AuthConfig::Publickey {
                private_key_path,
                passphrase_ref,
            } => (
                AuthType::Publickey,
                None,
                private_key_path.to_string_lossy().into_owned(),
                passphrase_ref.clone(),
            ),
        };
        Self {
            mode: EditorMode::Edit,
            id: cfg.id.clone(),
            name: cfg.name.clone(),
            host: cfg.host.clone(),
            port: cfg.port.to_string(),
            username: cfg.username.clone(),
            auth_type,
            shell_integration: cfg.shell_integration,
            password: String::new(),
            save_password: password_ref
                .as_ref()
                .is_some_and(|r| !r.trim().is_empty()),
            has_saved_password: password_ref
                .as_ref()
                .is_some_and(|r| !r.trim().is_empty()),
            private_key_path: key_path,
            passphrase: String::new(),
            save_passphrase: pass_ref.as_ref().is_some_and(|r| !r.trim().is_empty()),
            has_saved_passphrase: pass_ref.as_ref().is_some_and(|r| !r.trim().is_empty()),
            color_tag: cfg.color_tag.clone(),
            icon: cfg.icon.clone(),
            folder_id,
            error: None,
            focus_name: true,
        }
    }
}

pub enum EditorAction {
    Save(SessionEditorState),
    Cancel,
}

const COLOR_PRESETS: &[(&str, &str)] = &[
    ("#e74c3c", "红"),
    ("#27ae60", "绿"),
    ("#2980b9", "蓝"),
    ("#f39c12", "橙"),
    ("#8e44ad", "紫"),
    ("#16a085", "青"),
];

/// Returns `Some` when the dialog should close (save or cancel).
pub fn show(
    ctx: &egui::Context,
    state: &mut SessionEditorState,
    tree: &SessionTree,
) -> Option<EditorAction> {
    let mut action = None;
    let mut open = true;
    let title = match state.mode {
        EditorMode::Add => i18n::t("dialog.session.add_title"),
        EditorMode::Edit => i18n::t("dialog.session.edit_title"),
    };

    let _title_bar = crate::dialog_chrome::CompactTitleBar::push(ctx);
    egui::Window::new(crate::dialog_chrome::title(title))
        .id(egui::Id::new("session_editor"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(420.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            egui::Grid::new("session_editor_grid")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    ui.label(i18n::t("dialog.session.name"));
                    let name_resp = ui.add(
                        egui::TextEdit::singleline(&mut state.name)
                            .desired_width(f32::INFINITY),
                    );
                    if state.focus_name {
                        name_resp.request_focus();
                        state.focus_name = false;
                    }
                    ui.end_row();

                    ui.label(i18n::t("dialog.session.host"));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.host).desired_width(f32::INFINITY),
                    );
                    ui.end_row();

                    ui.label(i18n::t("dialog.session.port"));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.port)
                            .desired_width(80.0)
                            .char_limit(5),
                    );
                    ui.end_row();

                    ui.label(i18n::t("dialog.session.username"));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.username)
                            .desired_width(f32::INFINITY)
                            .hint_text(i18n::t("dialog.session.username_hint")),
                    );
                    ui.end_row();

                    ui.label(i18n::t("dialog.session.folder"));
                    folder_combo(ui, state, tree);
                    ui.end_row();

                    ui.label(i18n::t("dialog.session.auth"));
                    ui.horizontal(|ui| {
                        ui.selectable_value(
                            &mut state.auth_type,
                            AuthType::Password,
                            i18n::t("dialog.session.auth_password"),
                        );
                        ui.selectable_value(
                            &mut state.auth_type,
                            AuthType::Publickey,
                            i18n::t("dialog.session.auth_key"),
                        );
                    });
                    ui.end_row();

                    ui.label(i18n::t("dialog.session.shell_integration"));
                    ui.checkbox(
                        &mut state.shell_integration,
                        i18n::t("dialog.session.shell_integration_enable"),
                    );
                    ui.end_row();
                });
                ui.label(
                    RichText::new(i18n::t("dialog.session.shell_integration_hint"))
                        .size(11.0)
                        .color(Color32::from_rgb(100, 105, 115)),
                );

            ui.add_space(6.0);
            match state.auth_type {
                AuthType::Password => {
                    if state.has_saved_password && state.password.is_empty() {
                        ui.label(
                            RichText::new(i18n::t("dialog.session.password_saved"))
                                .size(12.0)
                                .color(Color32::from_rgb(80, 120, 80)),
                        );
                    }
                    ui.label(i18n::t("dialog.session.password"));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.password)
                            .password(true)
                            .desired_width(f32::INFINITY)
                            .hint_text(if state.has_saved_password {
                                i18n::t("dialog.session.password_keep_hint")
                            } else {
                                i18n::t("dialog.session.password_hint")
                            }),
                    );
                    ui.checkbox(
                        &mut state.save_password,
                        i18n::t("dialog.session.save_password"),
                    );
                    if state.save_password && state.password.is_empty() && !state.has_saved_password
                    {
                        ui.label(
                            RichText::new(i18n::t("dialog.session.save_password_need_value"))
                                .size(11.0)
                                .color(Color32::from_rgb(180, 80, 60)),
                        );
                    }
                }
                AuthType::Publickey => {
                    ui.label(i18n::t("dialog.session.key_path"));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.private_key_path)
                            .desired_width(f32::INFINITY)
                            .hint_text("~/.ssh/id_ed25519"),
                    );
                    if state.has_saved_passphrase && state.passphrase.is_empty() {
                        ui.label(
                            RichText::new(i18n::t("dialog.session.passphrase_saved"))
                                .size(12.0)
                                .color(Color32::from_rgb(80, 120, 80)),
                        );
                    }
                    ui.label(i18n::t("dialog.session.passphrase"));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.passphrase)
                            .password(true)
                            .desired_width(f32::INFINITY),
                    );
                    ui.checkbox(
                        &mut state.save_passphrase,
                        i18n::t("dialog.session.save_passphrase"),
                    );
                }
            }

            ui.add_space(8.0);
            ui.label(i18n::t("dialog.session.color"));
            ui.horizontal(|ui| {
                let none_selected = state.color_tag.is_none();
                if ui.selectable_label(none_selected, i18n::t("dialog.session.color_none")).clicked()
                {
                    state.color_tag = None;
                }
                for (hex, _label) in COLOR_PRESETS {
                    let selected = state.color_tag.as_deref() == Some(*hex);
                    let color = parse_hex(hex).unwrap_or(Color32::GRAY);
                    let (rect, resp) =
                        ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());
                    ui.painter().rect_filled(rect, 3.0, color);
                    if selected {
                        ui.painter().rect_stroke(
                            rect,
                            3.0,
                            egui::Stroke::new(2.0_f32, Color32::from_rgb(40, 40, 50)),
                            egui::StrokeKind::Outside,
                        );
                    }
                    if resp.clicked() {
                        state.color_tag = Some((*hex).to_string());
                    }
                }
            });

            ui.add_space(8.0);
            ui.label(i18n::t("dialog.session.icon"));
            ui.horizontal_wrapped(|ui| {
                let auto = state.icon.is_none();
                if ui
                    .selectable_label(auto, i18n::t("dialog.session.icon_auto"))
                    .on_hover_text(i18n::t("dialog.session.icon_auto_tip"))
                    .clicked()
                {
                    state.icon = None;
                }
                for id in crate::os_icon::PRESETS {
                    let selected = state.icon.as_deref() == Some(*id);
                    let tip = i18n::t(&crate::os_icon::i18n_key(id));
                    if crate::os_icon::selectable(ui, id, selected, 22.0)
                        .on_hover_text(tip)
                        .clicked()
                    {
                        state.icon = Some((*id).to_string());
                    }
                }
            });

            if let Some(err) = &state.error {
                ui.add_space(8.0);
                ui.colored_label(Color32::from_rgb(200, 60, 60), err);
            }

            ui.add_space(12.0);
            crate::dialog_chrome::centered_actions(ui, |ui| {
                if ui.button(i18n::t("dialog.session.save")).clicked() {
                    if let Some(err) = validate(state) {
                        state.error = Some(err);
                    } else {
                        state.error = None;
                        action = Some(EditorAction::Save(state.clone()));
                    }
                }
                if ui.button(i18n::t("dialog.session.cancel")).clicked() {
                    action = Some(EditorAction::Cancel);
                }
            });
        });

    if !open {
        action = Some(EditorAction::Cancel);
    }
    action
}

fn folder_combo(ui: &mut Ui, state: &mut SessionEditorState, tree: &SessionTree) {
    let folders = tree.list_folders();
    let preview = match &state.folder_id {
        None => i18n::t("dialog.session.folder_root"),
        Some(id) => folders
            .iter()
            .find(|(fid, _)| fid == id)
            .map(|(_, n)| n.clone())
            .unwrap_or_else(|| id.clone()),
    };
    egui::ComboBox::from_id_salt("session_folder_combo")
        .width(220.0)
        .selected_text(preview)
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(state.folder_id.is_none(), i18n::t("dialog.session.folder_root"))
                .clicked()
            {
                state.folder_id = None;
            }
            for (fid, fname) in folders {
                if ui
                    .selectable_label(state.folder_id.as_deref() == Some(fid.as_str()), &fname)
                    .clicked()
                {
                    state.folder_id = Some(fid);
                }
            }
        });
}

fn validate(state: &SessionEditorState) -> Option<String> {
    if state.name.trim().is_empty() {
        return Some(i18n::t("dialog.session.err_name"));
    }
    if state.host.trim().is_empty() {
        return Some(i18n::t("dialog.session.err_host"));
    }
    if state.port.trim().parse::<u16>().ok().filter(|p| *p > 0).is_none() {
        return Some(i18n::t("dialog.session.err_port"));
    }
    match state.auth_type {
        AuthType::Password => {
            if state.save_password && state.password.is_empty() && !state.has_saved_password {
                return Some(i18n::t("dialog.session.err_password"));
            }
        }
        AuthType::Publickey => {
            if state.private_key_path.trim().is_empty() {
                return Some(i18n::t("dialog.session.err_key"));
            }
            if state.save_passphrase && state.passphrase.is_empty() && !state.has_saved_passphrase {
                return Some(i18n::t("dialog.session.err_passphrase"));
            }
        }
    }
    None
}

/// Build a [`SessionConfig`] and optional secrets to write into the vault.
pub struct BuiltSession {
    pub config: SessionConfig,
    pub folder_id: Option<String>,
    /// `(entry_id, plaintext)` to store; `None` means leave vault entry untouched.
    pub password_to_save: Option<(String, String)>,
    pub passphrase_to_save: Option<(String, String)>,
    /// Clear vault password ref when switching away from password auth.
    pub clear_password_ref: bool,
}

pub fn build_session(state: &SessionEditorState) -> Result<BuiltSession, String> {
    if let Some(err) = validate(state) {
        return Err(err);
    }
    let port: u16 = state
        .port
        .trim()
        .parse()
        .map_err(|_| i18n::t("dialog.session.err_port"))?;

    let mut password_to_save = None;
    let mut passphrase_to_save = None;
    let clear_password_ref;

    let auth = match state.auth_type {
        AuthType::Password => {
            let entry_id = format!("{}-pwd", state.id);
            let (password_ref, password_to_save_v, clear_pwd) = if state.save_password {
                let to_save = if !state.password.is_empty() {
                    Some((entry_id.clone(), state.password.clone()))
                } else {
                    None
                };
                let keep_ref = to_save.is_some() || state.has_saved_password;
                (
                    if keep_ref {
                        Some(vault::format_vault_ref(&entry_id))
                    } else {
                        None
                    },
                    to_save,
                    false,
                )
            } else {
                (None, None, state.has_saved_password)
            };
            password_to_save = password_to_save_v;
            clear_password_ref = clear_pwd;
            AuthConfig::Password { password_ref }
        }
        AuthType::Publickey => {
            clear_password_ref = state.has_saved_password;
            let entry_id = format!("{}-passphrase", state.id);
            let passphrase_ref = if state.save_passphrase {
                if !state.passphrase.is_empty() {
                    passphrase_to_save = Some((entry_id.clone(), state.passphrase.clone()));
                }
                if passphrase_to_save.is_some() || state.has_saved_passphrase {
                    Some(vault::format_vault_ref(&entry_id))
                } else {
                    None
                }
            } else {
                None
            };
            AuthConfig::Publickey {
                private_key_path: PathBuf::from(state.private_key_path.trim()),
                passphrase_ref,
            }
        }
    };

    Ok(BuiltSession {
        config: SessionConfig {
            id: state.id.clone(),
            name: state.name.trim().to_string(),
            host: state.host.trim().to_string(),
            port,
            username: state.username.trim().to_string(),
            auth,
            color_tag: state.color_tag.clone(),
            icon: state.icon.clone(),
            term_type: "xterm-256color".into(),
            shell_integration: state.shell_integration,
        },
        folder_id: state.folder_id.clone(),
        password_to_save,
        passphrase_to_save,
        clear_password_ref,
    })
}

fn parse_hex(hex: &str) -> Option<Color32> {
    let h = hex.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&h[0..2], 16).ok()?;
    let g = u8::from_str_radix(&h[2..4], 16).ok()?;
    let b = u8::from_str_radix(&h[4..6], 16).ok()?;
    Some(Color32::from_rgb(r, g, b))
}

pub fn slugify_id(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if matches!(c, '-' | '_' | ' ' | '\t') {
            if !out.ends_with('-') {
                out.push('-');
            }
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        format!("s-{}", &uuid::Uuid::new_v4().to_string()[..8])
    } else {
        out
    }
}

pub fn allocate_session_id(tree: &SessionTree, preferred: &str) -> String {
    let base = slugify_id(preferred);
    let mut candidate = base.clone();
    let mut n = 2u32;
    let file = |id: &str| format!("{id}.yaml");
    while tree.contains_session_ref(&file(&candidate)) {
        candidate = format!("{base}-{n}");
        n += 1;
    }
    candidate
}
