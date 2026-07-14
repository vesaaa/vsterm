//! Interactive connect dialogs: username+password, or public-key path.

use crate::i18n;
use egui::{Color32, RichText};
use session_tree::{AuthConfig, AuthType, SessionConfig};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthPromptKind {
    Password,
    PublicKey,
}

#[derive(Debug, Clone)]
pub struct AuthPromptState {
    pub config: SessionConfig,
    pub kind: AuthPromptKind,
    pub username: String,
    pub password: String,
    pub key_path: String,
    pub attempt: u32,
    pub warn: Option<String>,
    pub focus_username: bool,
    pub focus_secret: bool,
    /// First pubkey attempt is started automatically when path + username are ready.
    pub auto_tried: bool,
    pub has_vault_password: bool,
    /// Set after the dialog has been painted once (so auto-verify still shows the window briefly).
    pub dialog_shown: bool,
}

impl AuthPromptState {
    pub fn for_session(config: SessionConfig, attempt: u32) -> Self {
        let kind = match config.auth.auth_type() {
            AuthType::Password => AuthPromptKind::Password,
            AuthType::Publickey => AuthPromptKind::PublicKey,
        };
        let key_path = config
            .auth
            .private_key_path()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let has_vault_password = config.auth.has_vault_password();
        let username = config.username.clone();
        let focus_username = username.trim().is_empty();
        Self {
            config,
            kind,
            username,
            password: String::new(),
            key_path,
            attempt,
            warn: None,
            focus_username,
            focus_secret: !focus_username,
            auto_tried: false,
            has_vault_password,
            dialog_shown: false,
        }
    }

    pub fn with_error(mut self, msg: impl Into<String>) -> Self {
        self.warn = Some(msg.into());
        self.focus_secret = true;
        self
    }

    pub fn host_label(&self) -> String {
        format!("{}:{}", self.config.host, self.config.port)
    }

    /// Apply dialog fields onto a connect-ready config + interactive password.
    pub fn build_connect(&self) -> Result<(SessionConfig, Option<String>), String> {
        let username = self.username.trim().to_string();
        if username.is_empty() {
            return Err(i18n::t("dialog.auth.err_username"));
        }
        let mut config = self.config.clone();
        config.username = username;

        match self.kind {
            AuthPromptKind::Password => {
                if self.password.is_empty() && !self.has_vault_password {
                    return Err(i18n::t("dialog.auth.err_password"));
                }
                let interactive = if self.password.is_empty() {
                    None
                } else {
                    Some(self.password.clone())
                };
                Ok((config, interactive))
            }
            AuthPromptKind::PublicKey => {
                let path = self.key_path.trim();
                if path.is_empty() {
                    return Err(i18n::t("dialog.auth.err_key"));
                }
                let expanded = connection_mgr::expand_user_path(path);
                if !expanded.exists() {
                    return Err(i18n::t("dialog.auth.err_key_missing"));
                }
                let passphrase_ref = match &config.auth {
                    AuthConfig::Publickey {
                        passphrase_ref, ..
                    } => passphrase_ref.clone(),
                    _ => None,
                };
                config.auth = AuthConfig::Publickey {
                    private_key_path: PathBuf::from(path),
                    passphrase_ref,
                };
                Ok((config, None))
            }
        }
    }

    pub fn can_auto_verify(&self) -> bool {
        self.kind == AuthPromptKind::PublicKey
            && self.dialog_shown
            && !self.auto_tried
            && self.warn.is_none()
            && !self.username.trim().is_empty()
            && !self.key_path.trim().is_empty()
            && connection_mgr::expand_user_path(self.key_path.trim()).exists()
    }
}

pub enum AuthPromptAction {
    Connect(AuthPromptState),
    Cancel,
}

pub fn show(ctx: &egui::Context, state: &mut AuthPromptState) -> Option<AuthPromptAction> {
    let mut action = None;
    let mut open = true;
    let title = match state.kind {
        AuthPromptKind::Password => i18n::t("dialog.auth.password_title"),
        AuthPromptKind::PublicKey => i18n::t("dialog.auth.key_title"),
    };

    egui::Window::new(format!("{} ({}/{})", title, state.attempt, max_attempts()))
        .id(egui::Id::new("connect_auth_prompt"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(400.0)
        .show(ctx, |ui| {
            state.dialog_shown = true;
            if state.kind == AuthPromptKind::PublicKey && !state.auto_tried && state.warn.is_none()
            {
                ui.label(
                    RichText::new(i18n::t("dialog.auth.auto_verifying"))
                        .size(12.0)
                        .color(Color32::from_rgb(60, 110, 160)),
                );
                ui.add_space(6.0);
            }
        ui.label(format!(
            "{}: {}",
            i18n::t("dialog.auth.host"),
            state.host_label()
        ));
        if let Some(warn) = &state.warn {
            ui.add_space(4.0);
            ui.colored_label(Color32::from_rgb(220, 80, 80), warn);
        }
        ui.add_space(8.0);

        ui.label(i18n::t("dialog.auth.username"));
        let user_resp = ui.add(
            egui::TextEdit::singleline(&mut state.username)
                .desired_width(f32::INFINITY)
                .hint_text(i18n::t("dialog.auth.username_hint")),
        );
        if state.focus_username {
            user_resp.request_focus();
            state.focus_username = false;
        }

        ui.add_space(6.0);
        let secret_resp = match state.kind {
            AuthPromptKind::Password => {
                ui.label(i18n::t("dialog.auth.password"));
                if state.has_vault_password {
                    ui.label(
                        RichText::new(i18n::t("dialog.auth.password_vault_hint"))
                            .size(11.0)
                            .color(Color32::from_rgb(90, 110, 90)),
                    );
                } else {
                    ui.label(
                        RichText::new(i18n::t("dialog.auth.password_hint"))
                            .size(11.0)
                            .weak(),
                    );
                }
                ui.add(
                    egui::TextEdit::singleline(&mut state.password)
                        .password(true)
                        .desired_width(f32::INFINITY),
                )
            }
            AuthPromptKind::PublicKey => {
                ui.label(i18n::t("dialog.auth.key_path"));
                ui.label(
                    RichText::new(i18n::t("dialog.auth.key_hint"))
                        .size(11.0)
                        .weak(),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut state.key_path)
                        .desired_width(f32::INFINITY)
                        .hint_text("~/.ssh/id_ed25519"),
                )
            }
        };
        if state.focus_secret {
            secret_resp.request_focus();
            state.focus_secret = false;
        }

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let confirm_label = match state.kind {
                AuthPromptKind::Password => i18n::t("dialog.auth.connect"),
                AuthPromptKind::PublicKey => i18n::t("dialog.auth.verify"),
            };
            if ui.button(confirm_label).clicked() {
                match state.build_connect() {
                    Ok(_) => action = Some(AuthPromptAction::Connect(state.clone())),
                    Err(err) => state.warn = Some(err),
                }
            }
            if ui.button(i18n::t("dialog.auth.cancel")).clicked() {
                action = Some(AuthPromptAction::Cancel);
            }
        });

        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        if enter {
            match state.build_connect() {
                Ok(_) => action = Some(AuthPromptAction::Connect(state.clone())),
                Err(err) => state.warn = Some(err),
            }
        }
    });

    if !open {
        action = Some(AuthPromptAction::Cancel);
    }
    action
}

/// Shared with app reconnect loop.
pub fn max_attempts() -> u32 {
    3
}
