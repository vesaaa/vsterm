//! Interactive connect dialogs: username+password, or public-key path.

use crate::dialog_chrome::{self, CompactTitleBar};
use crate::i18n;
use egui::{Color32, RichText};
use session_tree::{AuthConfig, AuthType, SessionConfig};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// iPhone-style wrong-entry horizontal shake duration.
const SHAKE_LIFE: Duration = Duration::from_millis(480);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthPromptKind {
    Password,
    PublicKey,
}

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
    /// True while SSH handshake is running.
    pub verifying: bool,
    /// Server software ident from pre-auth TCP probe (`OpenSSH_… Ubuntu-…`), when available.
    pub server_ident: Option<String>,
    /// Started when auth/validation fails — drives horizontal shake offset.
    shake_born: Option<Instant>,
    /// Render off-screen only to measure the true dialog rect (for spit-out FX targeting).
    pub measure_only: bool,
    /// Whether the appear-spark ring has been spawned for this dialog instance.
    pub sparked: bool,
    ident_rx: Option<mpsc::Receiver<Option<String>>>,
}

impl Clone for AuthPromptState {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            kind: self.kind,
            username: self.username.clone(),
            password: self.password.clone(),
            key_path: self.key_path.clone(),
            attempt: self.attempt,
            warn: self.warn.clone(),
            focus_username: self.focus_username,
            focus_secret: self.focus_secret,
            auto_tried: self.auto_tried,
            has_vault_password: self.has_vault_password,
            dialog_shown: self.dialog_shown,
            verifying: self.verifying,
            server_ident: self.server_ident.clone(),
            shake_born: self.shake_born,
            measure_only: self.measure_only,
            sparked: self.sparked,
            ident_rx: None,
        }
    }
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
        let (ident_tx, ident_rx) = mpsc::channel();
        let host = config.host.clone();
        let port = config.port;
        thread::Builder::new()
            .name("vsterm-ssh-ident".into())
            .spawn(move || {
                let result = connection_mgr::probe_ssh_software_ident(&host, port).ok();
                let _ = ident_tx.send(result);
            })
            .ok();
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
            verifying: false,
            server_ident: None,
            shake_born: None,
            measure_only: false,
            sparked: false,
            ident_rx: Some(ident_rx),
        }
    }

    pub fn with_error(mut self, msg: impl Into<String>) -> Self {
        self.warn = Some(msg.into());
        self.focus_secret = true;
        self.verifying = false;
        self.trigger_shake();
        self
    }

    pub fn trigger_shake(&mut self) {
        self.shake_born = Some(Instant::now());
    }

    pub fn is_shaking(&self) -> bool {
        self.shake_born
            .is_some_and(|born| born.elapsed() < SHAKE_LIFE)
    }

    /// Decaying left–right offset (px), clearing when the shake finishes.
    fn shake_dx(&mut self) -> f32 {
        let Some(born) = self.shake_born else {
            return 0.0;
        };
        let t = born.elapsed().as_secs_f32();
        let life = SHAKE_LIFE.as_secs_f32();
        if t >= life {
            self.shake_born = None;
            return 0.0;
        }
        let damp = (1.0 - t / life).powi(2);
        14.0 * damp * (t * 58.0).sin()
    }

    pub fn host_label(&self) -> String {
        format!("{}:{}", self.config.host, self.config.port)
    }

    fn poll_server_ident(&mut self) {
        if self.server_ident.is_some() {
            return;
        }
        let Some(rx) = &self.ident_rx else {
            return;
        };
        match rx.try_recv() {
            Ok(Some(ident)) => {
                self.server_ident = Some(ident);
                self.ident_rx = None;
            }
            Ok(None) => {
                self.ident_rx = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.ident_rx = None;
            }
        }
    }

    /// Snapshot chrome for the shatter FX (title / fields / buttons).
    pub fn shatter_face(&self) -> crate::fx::ShatterFace {
        let title_base = match self.kind {
            AuthPromptKind::Password => i18n::t("dialog.auth.password_title"),
            AuthPromptKind::PublicKey => i18n::t("dialog.auth.key_title"),
        };
        let title = format!("{} ({}/{})", title_base, self.attempt, max_attempts());
        let mut host_line = format!("{}: {}", i18n::t("dialog.auth.host"), self.host_label());
        if let Some(ident) = &self.server_ident {
            host_line.push_str(&format!(" · {ident}"));
        }
        let (secret_label, secret_display, btn_primary) = match self.kind {
            AuthPromptKind::Password => {
                let bullets = if self.password.is_empty() {
                    String::new()
                } else {
                    "•".repeat(self.password.chars().count().clamp(4, 16))
                };
                (
                    i18n::t("dialog.auth.password"),
                    bullets,
                    i18n::t("dialog.auth.connect"),
                )
            }
            AuthPromptKind::PublicKey => {
                let path = if self.key_path.len() > 28 {
                    format!("…{}", &self.key_path[self.key_path.len().saturating_sub(26)..])
                } else {
                    self.key_path.clone()
                };
                (
                    i18n::t("dialog.auth.key_path"),
                    path,
                    i18n::t("dialog.auth.verify"),
                )
            }
        };
        crate::fx::ShatterFace {
            title,
            host_line,
            username_label: i18n::t("dialog.auth.username"),
            username: self.username.clone(),
            secret_label,
            secret_display,
            btn_primary,
            btn_cancel: i18n::t("dialog.auth.cancel"),
        }
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
            && !self.measure_only
            && self.dialog_shown
            && !self.auto_tried
            && !self.verifying
            && self.warn.is_none()
            && !self.username.trim().is_empty()
            && !self.key_path.trim().is_empty()
    }
}

pub enum AuthPromptAction {
    Connect(AuthPromptState),
    Cancel,
}

/// Returns `(action, dialog_screen_rect)` — rect is used for suck FX on success.
pub fn show(ctx: &egui::Context, state: &mut AuthPromptState) -> (Option<AuthPromptAction>, Option<egui::Rect>) {
    state.poll_server_ident();
    let shake_x = state.shake_dx();

    let mut action = None;
    let mut open = true;
    let mut dialog_rect = None;
    let title = match state.kind {
        AuthPromptKind::Password => i18n::t("dialog.auth.password_title"),
        AuthPromptKind::PublicKey => i18n::t("dialog.auth.key_title"),
    };

    // Compact title bar — shared metrics with other modal dialogs.
    let _title_bar = CompactTitleBar::push(ctx);

    let measuring = state.measure_only;
    let title_text = dialog_chrome::title(format!("{} ({}/{})", title, state.attempt, max_attempts()));
    let mut window = egui::Window::new(title_text)
        .id(egui::Id::new("connect_auth_prompt"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(400.0);
    window = if measuring {
        // Render off-screen (invisible) purely to capture the true rect size.
        window
            .constrain(false)
            .anchor(egui::Align2::LEFT_TOP, [-10000.0, -10000.0])
    } else {
        window.anchor(egui::Align2::CENTER_CENTER, [shake_x, 0.0])
    };
    let response = window
        .show(ctx, |ui| {
            state.dialog_shown = true;
            if state.verifying {
                ui.label(
                    RichText::new(i18n::t("dialog.auth.verifying"))
                        .size(13.0)
                        .color(Color32::from_rgb(40, 120, 160)),
                );
                ui.add_space(8.0);
            }
            let enabled = !state.verifying;
            ui.add_enabled_ui(enabled, |ui| {
                if state.kind == AuthPromptKind::PublicKey
                    && !state.auto_tried
                    && state.warn.is_none()
                {
                    ui.label(
                        RichText::new(i18n::t("dialog.auth.auto_verifying"))
                            .size(12.0)
                            .color(Color32::from_rgb(60, 110, 160)),
                    );
                    ui.add_space(6.0);
                }

                ui.horizontal_wrapped(|ui| {
                    ui.label(format!(
                        "{}: {}",
                        i18n::t("dialog.auth.host"),
                        state.host_label()
                    ));
                    if let Some(ident) = &state.server_ident {
                        ui.label(
                            RichText::new(format!("· {ident}"))
                                .size(12.0)
                                .color(Color32::from_rgb(90, 105, 125)),
                        );
                    }
                });
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
                if state.focus_username && enabled && !measuring {
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
                if state.focus_secret && enabled && !measuring {
                    secret_resp.request_focus();
                    state.focus_secret = false;
                }

                ui.add_space(12.0);
                crate::dialog_chrome::centered_actions(ui, |ui| {
                    let confirm_label = match state.kind {
                        AuthPromptKind::Password => i18n::t("dialog.auth.connect"),
                        AuthPromptKind::PublicKey => i18n::t("dialog.auth.verify"),
                    };
                    if ui
                        .add_enabled(enabled, egui::Button::new(confirm_label))
                        .clicked()
                    {
                        match state.build_connect() {
                            Ok(_) => action = Some(AuthPromptAction::Connect(state.clone())),
                            Err(err) => state.warn = Some(err),
                        }
                    }
                    if ui
                        .add_enabled(enabled, egui::Button::new(i18n::t("dialog.auth.cancel")))
                        .clicked()
                    {
                        action = Some(AuthPromptAction::Cancel);
                    }
                });

                if enabled {
                    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if enter {
                        match state.build_connect() {
                            Ok(_) => action = Some(AuthPromptAction::Connect(state.clone())),
                            Err(err) => state.warn = Some(err),
                        }
                    }
                }
            });
        });

    if let Some(inner) = response {
        dialog_rect = Some(inner.response.rect);
    }

    if measuring {
        // Measurement pass never yields an action.
        return (None, dialog_rect);
    }

    if !open && !state.verifying {
        action = Some(AuthPromptAction::Cancel);
    }
    (action, dialog_rect)
}

/// Shared with app reconnect loop.
pub fn max_attempts() -> u32 {
    3
}

