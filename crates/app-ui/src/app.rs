use crate::commands::CommandBook;
use crate::conn_error::{format_conn_error, ConnErrorDisplay};
use crate::fx::FxLayer;
use crate::i18n::{self, Locale};
use crate::metrics::{HostSnapshot, MetricsService};
use crate::remote_host::RemoteHostService;
use crate::panels::bottom_panel::{self, BottomPanelState};
use crate::panels::host_toolbar::{self, MainTab};
use crate::panels::connect_auth::{self, AuthPromptState};
use crate::panels::session_editor::{self, EditorMode, SessionEditorState};
use crate::panels::session_tree_panel::{self, TreeSelection};
use crate::panels::{connection_list, monitor, routes, status_bar, toolbar};
use crate::terminal_view::TerminalView;
use crate::{fonts, theme};
use connection_mgr::{ConnectFailure, ConnectionManager, ConnError, ConnErrorKey};
use session_tree::BackendKind;
use eframe::egui;
use session_tree::{AppPaths, SessionConfig, SessionStore, SessionTree};
use std::sync::mpsc;
use std::sync::Arc;
use vault::Vault;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum LeftTab {
    #[default]
    Servers,
    Monitor,
}

const MAX_PASSWORD_ATTEMPTS: u32 = 3;

struct PendingConnect {
    config: SessionConfig,
    attempt: u32,
    kind: connect_auth::AuthPromptKind,
    rx: mpsc::Receiver<Result<(), ConnectFailure>>,
    /// When set, success replaces this host tab instead of opening a new one.
    replace_id: Option<connection_mgr::ConnectionId>,
}

enum FolderDialogMode {
    Add { parent_id: Option<String> },
    Rename { id: String },
}

struct FolderDialogState {
    mode: FolderDialogMode,
    name: String,
    error: Option<String>,
    focus: bool,
}

#[derive(Clone)]
enum DeleteTarget {
    Session { session_ref: String, name: String },
    Folder { id: String, name: String },
}

pub struct VsTermApp {
    store: Option<SessionStore>,
    tree: SessionTree,
    connections: Arc<ConnectionManager>,
    metrics: MetricsService,
    remote_host: RemoteHostService,
    selected_nic: Option<String>,
    commands: CommandBook,
    left_tab: LeftTab,
    main_tab: MainTab,
    tree_width: f32,
    list_width: f32,
    bottom: BottomPanelState,
    status: String,
    last_term_size: (u16, u16),
    locale: Locale,
    pending_connect: Option<PendingConnect>,
    error_dialog: Option<ConnErrorDisplay>,
    auth_prompt: Option<AuthPromptState>,
    /// Host tab to replace after auth succeeds (status-light reconnect).
    reconnect_target: Option<connection_mgr::ConnectionId>,
    host_bind_gen: u64,
    tree_selection: Option<TreeSelection>,
    session_editor: Option<SessionEditorState>,
    folder_dialog: Option<FolderDialogState>,
    delete_confirm: Option<DeleteTarget>,
    /// Whether `set_repaint_wake` has been bound to this egui context.
    repaint_wake_bound: bool,
    /// Soft-glow / connect-ripple motion layer.
    fx: FxLayer,
    /// Last painted auth dialog rect (for suck-in / spit-out FX).
    last_auth_dialog_rect: Option<egui::Rect>,
    /// Session waiting for spit-out morph to finish before showing the auth dialog.
    pending_spit_auth: Option<SessionConfig>,
    /// Status-light rect queued until the next frame has a screen size for spit target.
    pending_spit_from: Option<egui::Rect>,
}

impl VsTermApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        fonts::install(&cc.egui_ctx);
        theme::apply(&cc.egui_ctx);

        let locale = load_locale();
        i18n::set(locale);

        let (store, tree, status) = match bootstrap_store() {
            Ok((store, tree)) => {
                let msg = format!("{}: {}", "数据目录", store.paths().root.display());
                (Some(store), tree, msg)
            }
            Err(err) => (None, SessionTree::new(), format!("load failed: {err}")),
        };

        let commands = store
            .as_ref()
            .and_then(|s| CommandBook::load_or_seed(s.paths()).ok())
            .unwrap_or_else(CommandBook::default_seed);

        let connections = Arc::new(ConnectionManager::new());
        let metrics = MetricsService::start();
        let remote_host = RemoteHostService::start();

        Self {
            store,
            tree,
            connections,
            metrics,
            remote_host,
            selected_nic: None,
            commands,
            left_tab: LeftTab::Servers,
            main_tab: MainTab::Terminal,
            // Fixed-ish widths — never grow from monitor plot/gauge content.
            tree_width: 260.0,
            list_width: 168.0,
            bottom: BottomPanelState::default(),
            status,
            last_term_size: (80, 24),
            locale,
            pending_connect: None,
            error_dialog: None,
            auth_prompt: None,
            reconnect_target: None,
            host_bind_gen: 0,
            tree_selection: None,
            session_editor: None,
            folder_dialog: None,
            delete_confirm: None,
            repaint_wake_bound: false,
            fx: FxLayer::default(),
            last_auth_dialog_rect: None,
            pending_spit_auth: None,
            pending_spit_from: None,
        }
    }

    fn reload_tree(&mut self) {
        if let Some(store) = &self.store {
            match store.load_tree() {
                Ok(tree) => {
                    self.tree = tree;
                    self.status = i18n::t("status.tree_reloaded");
                }
                Err(err) => self.status = format!("{}: {err}", i18n::t("status.open_failed")),
            }
        }
    }

    fn open_local_shell(&mut self, title: impl Into<String>) {
        match self.connections.open_local_shell(title) {
            Ok(_) => {
                self.status = i18n::t("status.opened_shell");
                self.left_tab = LeftTab::Monitor;
                self.main_tab = MainTab::Terminal;
                self.sync_host_binding();
            }
            Err(err) => {
                self.error_dialog = Some(format_conn_error(&err));
                self.status = i18n::t("status.open_failed");
            }
        }
    }

    fn request_open_session(&mut self, config: SessionConfig) {
        self.reconnect_target = None;
        self.begin_auth_for_session(config);
    }

    fn request_reconnect(
        &mut self,
        id: connection_mgr::ConnectionId,
        light: Option<egui::Rect>,
    ) {
        if self.pending_connect.is_some() || self.pending_spit_auth.is_some() {
            self.status = i18n::t("status.connecting");
            return;
        }
        let Some(session_id) = self.connections.session_id_of(id) else {
            self.status = i18n::t("conn.reconnect_unavailable");
            return;
        };
        let state = self.connections.connection_state(id);
        if !matches!(
            state,
            Some(connection_mgr::ConnectionState::Disconnected)
                | Some(connection_mgr::ConnectionState::Failed)
        ) {
            return;
        }

        let Some(store) = &self.store else {
            self.status = i18n::t("status.open_failed");
            return;
        };
        let file = if session_id.ends_with(".yaml") {
            session_id.clone()
        } else {
            format!("{session_id}.yaml")
        };
        match store.load_session(&file) {
            Ok(cfg) => {
                self.connections.set_active(id);
                self.sync_host_binding();
                self.left_tab = LeftTab::Monitor;
                self.main_tab = MainTab::Terminal;
                self.reconnect_target = Some(id);
                if let Some(light) = light.filter(|r| r.is_positive()) {
                    self.begin_auth_with_spit(cfg, light);
                } else {
                    self.begin_auth_for_session(cfg);
                }
            }
            Err(err) => {
                self.error_dialog =
                    Some(format_conn_error(&ConnError::NotFound(err.to_string())));
                self.status = i18n::t("status.open_failed");
            }
        }
    }

    fn begin_auth_for_session(&mut self, config: SessionConfig) {
        if self.pending_connect.is_some() {
            self.status = i18n::t("status.connecting");
            return;
        }

        if let Err(()) = self.preflight_before_auth(&config) {
            return;
        }

        // Password and public-key sessions always go through an interactive prompt.
        self.pending_spit_auth = None;
        self.auth_prompt = Some(AuthPromptState::for_session(config, 1));
    }

    /// Gray status-light reconnect: morph card out of the light, then show the dialog.
    fn begin_auth_with_spit(&mut self, config: SessionConfig, from_light: egui::Rect) {
        if self.pending_connect.is_some() {
            self.status = i18n::t("status.connecting");
            return;
        }
        if let Err(()) = self.preflight_before_auth(&config) {
            return;
        }
        self.auth_prompt = None;
        self.pending_spit_auth = Some(config);
        self.pending_spit_from = Some(from_light);
    }

    /// Returns `Err(())` after surfacing an error dialog / clearing reconnect.
    fn preflight_before_auth(&mut self, config: &SessionConfig) -> Result<(), ()> {
        let resolved = connection_mgr::resolve_backend(config.backend);
        if resolved == BackendKind::System
            && !connection_mgr::SystemSshBackend::is_available()
        {
            self.reconnect_target = None;
            self.pending_spit_auth = None;
            self.error_dialog = Some(format_conn_error(
                &connection_mgr::backend_unavailable_error(BackendKind::System),
            ));
            self.status = i18n::t("status.open_failed");
            return Err(());
        }

        let vault = self
            .store
            .as_ref()
            .and_then(|s| Vault::open(s.paths().vault_path()).ok());
        if let Err(err) =
            connection_mgr::preflight(config, vault.as_ref(), connection_mgr::PreflightOpts::before_prompt())
        {
            self.reconnect_target = None;
            self.pending_spit_auth = None;
            self.error_dialog = Some(format_conn_error(&err));
            self.status = i18n::t("status.open_failed");
            return Err(());
        }
        Ok(())
    }

    fn submit_auth_prompt(&mut self, mut prompt: AuthPromptState) {
        match prompt.build_connect() {
            Ok((config, interactive_password)) => {
                let kind = prompt.kind;
                let attempt = prompt.attempt;
                prompt.verifying = true;
                prompt.warn = None;
                self.auth_prompt = Some(prompt);
                self.start_ssh_session(config, interactive_password, attempt, kind);
                // If handshake never started, drop verifying chrome.
                if self.pending_connect.is_none() {
                    if self.error_dialog.is_some() {
                        self.auth_prompt = None;
                    } else if let Some(p) = &mut self.auth_prompt {
                        p.verifying = false;
                    }
                }
            }
            Err(err) => {
                self.auth_prompt = Some(prompt.with_error(err));
            }
        }
    }

    fn start_ssh_session(
        &mut self,
        config: SessionConfig,
        interactive_password: Option<String>,
        attempt: u32,
        kind: connect_auth::AuthPromptKind,
    ) {
        if self.pending_connect.is_some() {
            self.status = i18n::t("status.connecting");
            return;
        }

        let resolved = connection_mgr::resolve_backend(config.backend);
        if resolved == BackendKind::System
            && !connection_mgr::SystemSshBackend::is_available()
        {
            self.reconnect_target = None;
            self.error_dialog = Some(format_conn_error(
                &connection_mgr::backend_unavailable_error(BackendKind::System),
            ));
            self.status = i18n::t("status.open_failed");
            return;
        }

        let vault = self
            .store
            .as_ref()
            .and_then(|s| Vault::open(s.paths().vault_path()).ok());
        if let Err(err) = connection_mgr::preflight(
            &config,
            vault.as_ref(),
            connection_mgr::PreflightOpts::connecting(interactive_password.is_some()),
        ) {
            // Key missing / vault issues for pubkey → back to key dialog.
            if matches!(
                err,
                ConnError::PrivateKeyMissing { .. } | ConnError::VaultSecretMissing { .. }
            ) && kind == connect_auth::AuthPromptKind::PublicKey
                && attempt <= MAX_PASSWORD_ATTEMPTS
            {
                self.auth_prompt = Some(
                    AuthPromptState::for_session(config, attempt)
                        .with_error(format_conn_error(&err).title),
                );
                self.status = i18n::t("dialog.auth.verify_failed");
                return;
            }
            self.reconnect_target = None;
            self.error_dialog = Some(format_conn_error(&err));
            self.status = i18n::t("status.open_failed");
            return;
        }

        let replace_id = self.reconnect_target.take();
        if let Some(id) = replace_id {
            if !self.connections.mark_connecting(id) {
                self.status = i18n::t("conn.reconnect_unavailable");
                return;
            }
            self.connections.set_active(id);
        }

        // Do not add a host tab / switch panels until authentication succeeds
        // (reconnect keeps the existing tab and only flips its state).
        self.status = format!("{} — {}", i18n::t("status.connecting"), config.display_label());

        let mgr = Arc::clone(&self.connections);
        let vault_path = self.store.as_ref().map(|s| s.paths().vault_path());
        let (tx, rx) = mpsc::channel();

        let config_for_thread = config.clone();
        let config_for_pending = config.clone();
        let replace_for_thread = replace_id;
        std::thread::Builder::new()
            .name("vsterm-ssh-connect".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        let err = ConnError::Backend(format!("tokio runtime: {e}"));
                        let _ = tx.send(Err(err.into_failure()));
                        return;
                    }
                };
                let result = rt.block_on(async {
                    let vault = vault_path.as_ref().and_then(|p| Vault::open(p).ok());
                    ConnectionManager::establish_ssh(
                        &config,
                        vault.as_ref(),
                        interactive_password,
                        80,
                        24,
                    )
                    .await
                });
                match result {
                    Ok(established) => {
                        match replace_for_thread {
                            Some(id) => {
                                if let Err(est) =
                                    mgr.replace_ssh_connected(id, &config_for_thread, established)
                                {
                                    mgr.insert_ssh_connected(&config_for_thread, est);
                                }
                            }
                            None => {
                                mgr.insert_ssh_connected(&config_for_thread, established);
                            }
                        }
                        let _ = tx.send(Ok(()));
                    }
                    Err(err) => {
                        let _ = tx.send(Err(err.into_failure()));
                    }
                }
            })
            .ok();

        self.pending_connect = Some(PendingConnect {
            config: config_for_pending,
            attempt,
            kind,
            rx,
            replace_id,
        });
    }

    fn sync_host_binding(&mut self) {
        let vault_path = self.store.as_ref().map(|s| s.paths().vault_path());
        if let Some(remote) = self.connections.active_remote() {
            self.remote_host.bind(Some(remote), vault_path);
            // Always follow the bound host's nic (cache or freshly preferred).
            self.selected_nic = self.remote_host.selected_nic().or_else(|| {
                self.remote_host
                    .snapshot()
                    .as_ref()
                    .and_then(|s| {
                        HostSnapshot::prefer_primary_nic(&s.nics, s.default_if.as_deref())
                    })
            });
        } else {
            self.remote_host.bind(None, None);
            if self.connections.active_local_metrics() {
                self.selected_nic = self.metrics.selected_nic();
            }
        }
    }

    fn host_snapshot(&self) -> HostSnapshot {
        if self.connections.active_remote().is_some() {
            return self
                .remote_host
                .snapshot()
                .unwrap_or_default();
        }
        if self.connections.active_local_metrics() {
            return self.metrics.snapshot();
        }
        HostSnapshot::default()
    }

    fn poll_pending_connect(&mut self) {
        let Some(pending) = self.pending_connect.take() else {
            return;
        };
        match pending.rx.try_recv() {
            Ok(Ok(())) => {
                self.left_tab = LeftTab::Monitor;
                self.main_tab = MainTab::Terminal;
                self.sync_host_binding();
                self.status = i18n::t("status.connected");
                if let Some(rect) = self.last_auth_dialog_rect.take() {
                    self.fx.begin_auth_suck(rect);
                }
                self.auth_prompt = None;
            }
            Ok(Err(failure)) => {
                self.sync_host_binding();
                let retryable = matches!(
                    failure.key,
                    ConnErrorKey::AuthFailed | ConnErrorKey::PrivateKeyMissing
                ) && pending.attempt < MAX_PASSWORD_ATTEMPTS;
                if retryable {
                    // Keep in-place reconnect target across password retries.
                    self.reconnect_target = pending.replace_id;
                    let msg = match pending.kind {
                        connect_auth::AuthPromptKind::Password => i18n::t("dialog.auth.wrong_password"),
                        connect_auth::AuthPromptKind::PublicKey => {
                            i18n::t("dialog.auth.verify_failed")
                        }
                    };
                    let username = pending.config.username.clone();
                    let key_path = pending
                        .config
                        .auth
                        .private_key_path()
                        .map(|p| p.to_string_lossy().into_owned());
                    let mut prompt =
                        AuthPromptState::for_session(pending.config, pending.attempt + 1)
                            .with_error(msg);
                    prompt.username = username;
                    if let Some(path) = key_path {
                        prompt.key_path = path;
                    }
                    self.auth_prompt = Some(prompt);
                    self.status = i18n::t("dialog.auth.retry");
                } else {
                    if let Some(id) = pending.replace_id {
                        let msg = failure
                            .detail
                            .clone()
                            .unwrap_or_else(|| i18n::t("status.open_failed"));
                        self.connections.mark_failed(id, msg);
                    }
                    self.auth_prompt = None;
                    self.error_dialog =
                        Some(crate::conn_error::format_connect_failure(&failure));
                    if failure.key == ConnErrorKey::AuthFailed {
                        self.error_dialog.as_mut().map(|d| {
                            d.hint = i18n::t("dialog.password.max_attempts");
                        });
                    }
                    self.status = i18n::t("status.open_failed");
                }
            }
            Err(mpsc::TryRecvError::Empty) => {
                self.pending_connect = Some(pending);
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                if let Some(id) = pending.replace_id {
                    self.connections
                        .mark_failed(id, i18n::t("status.open_failed"));
                }
                self.status = i18n::t("status.open_failed");
            }
        }
    }

    fn show_auth_prompt(&mut self, ctx: &egui::Context) {
        let mut action = None;
        if let Some(prompt) = &mut self.auth_prompt {
            let (act, rect) = connect_auth::show(ctx, prompt);
            action = act;
            if let Some(r) = rect {
                self.last_auth_dialog_rect = Some(r);
            }
        }
        // After the dialog has been shown, auto-start the first public-key verification.
        let mut auto_connect = None;
        if action.is_none() {
            if let Some(prompt) = &mut self.auth_prompt {
                if prompt.can_auto_verify() {
                    prompt.auto_tried = true;
                    auto_connect = Some(prompt.clone());
                }
            }
        }
        if let Some(prompt) = auto_connect {
            self.submit_auth_prompt(prompt);
            return;
        }
        match action {
            Some(connect_auth::AuthPromptAction::Connect(prompt)) => {
                self.submit_auth_prompt(prompt);
            }
            Some(connect_auth::AuthPromptAction::Cancel) => {
                self.auth_prompt = None;
                self.last_auth_dialog_rect = None;
                if let Some(id) = self.reconnect_target.take() {
                    if matches!(
                        self.connections.connection_state(id),
                        Some(connection_mgr::ConnectionState::Connecting)
                    ) {
                        self.connections.mark_disconnected(id);
                    }
                }
                self.status = i18n::t("status.open_failed");
            }
            None => {}
        }
    }

    fn show_error_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.error_dialog.clone() else {
            return;
        };
        let mut open = true;
        egui::Window::new(i18n::t("dialog.error.title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new(&dialog.title).strong());
                if let Some(detail) = &dialog.detail {
                    ui.add_space(6.0);
                    ui.label(format!("{}:", i18n::t("dialog.error.detail")));
                    ui.label(
                        egui::RichText::new(detail)
                            .monospace()
                            .color(egui::Color32::from_rgb(100, 100, 110)),
                    );
                }
                ui.add_space(8.0);
                ui.label(format!("{}:", i18n::t("dialog.error.hint")));
                ui.label(&dialog.hint);
                ui.add_space(10.0);
                if ui.button(i18n::t("dialog.error.ok")).clicked() {
                    self.error_dialog = None;
                }
            });
        if !open {
            self.error_dialog = None;
        }
    }

    fn set_locale(&mut self, locale: Locale) {
        self.locale = locale;
        i18n::set(locale);
        save_locale(locale);
        self.status = i18n::t("status.lang_changed");
    }

    fn begin_add_server(&mut self, folder_id: Option<String>) {
        self.left_tab = LeftTab::Servers;
        self.session_editor = Some(SessionEditorState::new_add(folder_id));
    }

    fn begin_edit_server(&mut self, session_ref: &str) {
        let Some(store) = &self.store else {
            return;
        };
        match store.load_session(session_ref) {
            Ok(cfg) => {
                let folder_id = self.tree.folder_of_session(session_ref);
                self.session_editor = Some(SessionEditorState::from_config(&cfg, folder_id));
            }
            Err(err) => {
                self.error_dialog = Some(format_conn_error(&ConnError::NotFound(err.to_string())));
                self.status = i18n::t("status.open_failed");
            }
        }
    }

    fn persist_session_editor(&mut self, mut state: SessionEditorState) {
        let Some(store) = self.store.as_ref() else {
            self.status = i18n::t("status.save_failed");
            return;
        };

        if state.mode == EditorMode::Add {
            state.id = session_editor::allocate_session_id(&self.tree, &state.name);
        }

        let built = match session_editor::build_session(&state) {
            Ok(b) => b,
            Err(err) => {
                if let Some(ed) = &mut self.session_editor {
                    ed.error = Some(err);
                } else {
                    self.session_editor = Some(state);
                    if let Some(ed) = &mut self.session_editor {
                        ed.error = Some(err);
                    }
                }
                return;
            }
        };

        let session_ref = format!("{}.yaml", built.config.id);
        let folder_id = built.folder_id.clone();

        // Vault updates first so a failed vault write doesn't leave orphan config claims.
        if let Ok(mut vault) = Vault::open(store.paths().vault_path()) {
            if let Some((id, secret)) = &built.password_to_save {
                if let Err(err) = vault.set(id, secret) {
                    self.status = format!("{}: {err}", i18n::t("status.save_failed"));
                    self.session_editor = Some(state);
                    return;
                }
            }
            if let Some((id, secret)) = &built.passphrase_to_save {
                if let Err(err) = vault.set(id, secret) {
                    self.status = format!("{}: {err}", i18n::t("status.save_failed"));
                    self.session_editor = Some(state);
                    return;
                }
            }
            if built.clear_password_ref {
                let _ = vault.remove(&format!("{}-pwd", built.config.id));
            }
            if matches!(
                built.config.auth,
                session_tree::AuthConfig::Password { .. }
            ) {
                // switching to password: drop passphrase entry if any leftover
            }
            if matches!(
                built.config.auth,
                session_tree::AuthConfig::Publickey { .. }
            ) {
                let _ = vault.remove(&format!("{}-pwd", built.config.id));
            }
        }

        if let Err(err) = store.save_session(&built.config) {
            self.status = format!("{}: {err}", i18n::t("status.save_failed"));
            self.session_editor = Some(state);
            return;
        }

        let tree_result = match state.mode {
            EditorMode::Add => self.tree.insert_session(
                folder_id.as_deref(),
                built.config.name.clone(),
                session_ref.clone(),
            ),
            EditorMode::Edit => self.tree.relocate_session(
                &session_ref,
                built.config.name.clone(),
                folder_id.as_deref(),
            ),
        };
        if let Err(err) = tree_result {
            self.status = format!("{}: {err}", i18n::t("status.save_failed"));
            self.session_editor = Some(state);
            return;
        }

        if let Err(err) = store.save_tree(&self.tree) {
            self.status = format!("{}: {err}", i18n::t("status.save_failed"));
            self.session_editor = Some(state);
            return;
        }

        self.session_editor = None;
        self.tree_selection = Some(TreeSelection::Session {
            name: built.config.name.clone(),
            session_ref,
        });
        self.status = i18n::t("status.session_saved");
    }

    fn delete_session(&mut self, session_ref: &str) {
        let Some(store) = self.store.as_ref() else {
            return;
        };
        let id = session_ref
            .trim_end_matches(".yaml")
            .trim_end_matches(".yml");
        if let Ok(mut vault) = Vault::open(store.paths().vault_path()) {
            let _ = vault.remove(&format!("{id}-pwd"));
            let _ = vault.remove(&format!("{id}-passphrase"));
        }
        let _ = store.delete_session_file(session_ref);
        self.tree.remove_session_node(session_ref);
        if let Err(err) = store.save_tree(&self.tree) {
            self.status = format!("{}: {err}", i18n::t("status.save_failed"));
            return;
        }
        if matches!(
            &self.tree_selection,
            Some(TreeSelection::Session { session_ref: r, .. }) if r == session_ref
        ) {
            self.tree_selection = None;
        }
        self.status = i18n::t("status.session_deleted");
    }

    fn save_folder_dialog(&mut self) {
        let Some(dialog) = self.folder_dialog.take() else {
            return;
        };
        let name = dialog.name.trim().to_string();
        if name.is_empty() {
            self.folder_dialog = Some(FolderDialogState {
                error: Some(i18n::t("dialog.folder.err_name")),
                focus: true,
                ..dialog
            });
            return;
        }
        let Some(store) = self.store.as_ref() else {
            self.status = i18n::t("status.save_failed");
            return;
        };
        match dialog.mode {
            FolderDialogMode::Add { parent_id } => {
                let id = format!("f-{}", &uuid::Uuid::new_v4().to_string()[..8]);
                if let Err(err) =
                    self.tree
                        .add_folder(name.clone(), id.clone(), parent_id.as_deref())
                {
                    self.status = format!("{}: {err}", i18n::t("status.save_failed"));
                    return;
                }
                self.tree_selection = Some(TreeSelection::Folder { id, name });
            }
            FolderDialogMode::Rename { id } => {
                if !self.tree.rename_folder(&id, name.clone()) {
                    self.status = i18n::t("status.save_failed");
                    return;
                }
                self.tree_selection = Some(TreeSelection::Folder { id, name });
            }
        }
        if let Err(err) = store.save_tree(&self.tree) {
            self.status = format!("{}: {err}", i18n::t("status.save_failed"));
            return;
        }
        self.status = i18n::t("status.folder_saved");
    }

    fn delete_folder(&mut self, id: &str) {
        let Some(store) = self.store.as_ref() else {
            return;
        };
        match self.tree.remove_folder(id) {
            Ok(()) => {
                if let Err(err) = store.save_tree(&self.tree) {
                    self.status = format!("{}: {err}", i18n::t("status.save_failed"));
                    return;
                }
                if matches!(
                    &self.tree_selection,
                    Some(TreeSelection::Folder { id: fid, .. }) if fid == id
                ) {
                    self.tree_selection = None;
                }
                self.status = i18n::t("status.folder_deleted");
            }
            Err(_) => {
                self.status = i18n::t("status.folder_not_empty");
            }
        }
    }

    fn show_session_editor(&mut self, ctx: &egui::Context) {
        let mut taken = None;
        if let Some(state) = &mut self.session_editor {
            if let Some(action) = session_editor::show(ctx, state, &self.tree) {
                taken = Some(action);
            }
        }
        if let Some(action) = taken {
            match action {
                session_editor::EditorAction::Save(state) => self.persist_session_editor(state),
                session_editor::EditorAction::Cancel => self.session_editor = None,
            }
        }
    }

    fn show_folder_dialog(&mut self, ctx: &egui::Context) {
        let mut save = false;
        let mut cancel = false;
        if let Some(dialog) = &mut self.folder_dialog {
            let mut open = true;
            let title = match &dialog.mode {
                FolderDialogMode::Add { parent_id } if parent_id.is_some() => {
                    i18n::t("dialog.folder.add_sub_title")
                }
                FolderDialogMode::Add { .. } => i18n::t("dialog.folder.add_title"),
                FolderDialogMode::Rename { .. } => i18n::t("dialog.folder.rename_title"),
            };
            let parent_label = match &dialog.mode {
                FolderDialogMode::Add {
                    parent_id: Some(pid),
                } => self
                    .tree
                    .list_folders()
                    .into_iter()
                    .find(|(id, _)| id == pid)
                    .map(|(_, label)| label),
                _ => None,
            };
            egui::Window::new(title)
                .id(egui::Id::new("folder_dialog"))
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .default_width(320.0)
                .show(ctx, |ui| {
                    if let Some(parent) = &parent_label {
                        ui.label(
                            egui::RichText::new(format!(
                                "{}: {parent}",
                                i18n::t("dialog.folder.parent")
                            ))
                            .weak(),
                        );
                        ui.add_space(4.0);
                    }
                    ui.label(i18n::t("dialog.folder.name"));
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut dialog.name).desired_width(f32::INFINITY),
                    );
                    if dialog.focus {
                        resp.request_focus();
                        dialog.focus = false;
                    }
                    if let Some(err) = &dialog.error {
                        ui.colored_label(egui::Color32::from_rgb(200, 60, 60), err);
                    }
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button(i18n::t("dialog.folder.save")).clicked() {
                            save = true;
                        }
                        if ui.button(i18n::t("dialog.folder.cancel")).clicked() {
                            cancel = true;
                        }
                    });
                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        save = true;
                    }
                });
            if !open {
                cancel = true;
            }
        }
        if save {
            self.save_folder_dialog();
        } else if cancel {
            self.folder_dialog = None;
        }
    }

    fn show_delete_confirm(&mut self, ctx: &egui::Context) {
        let Some(target) = self.delete_confirm.clone() else {
            return;
        };
        let mut open = true;
        let mut confirm = false;
        let mut cancel = false;
        let body = match &target {
            DeleteTarget::Session { name, .. } => {
                i18n::t("dialog.delete.session").replace("{name}", name)
            }
            DeleteTarget::Folder { name, .. } => {
                i18n::t("dialog.delete.folder").replace("{name}", name)
            }
        };
        egui::Window::new(i18n::t("dialog.delete.title"))
            .id(egui::Id::new("delete_confirm"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(380.0)
            .show(ctx, |ui| {
                ui.label(body);
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button(i18n::t("dialog.delete.confirm")).clicked() {
                        confirm = true;
                    }
                    if ui.button(i18n::t("dialog.delete.cancel")).clicked() {
                        cancel = true;
                    }
                });
            });
        if !open {
            cancel = true;
        }
        if confirm {
            match target {
                DeleteTarget::Session { session_ref, .. } => self.delete_session(&session_ref),
                DeleteTarget::Folder { id, .. } => self.delete_folder(&id),
            }
            self.delete_confirm = None;
        } else if cancel {
            self.delete_confirm = None;
        }
    }

    fn handle_tree_action(&mut self, action: session_tree_panel::TreeAction) {
        match action {
            session_tree_panel::TreeAction::OpenLocalDemo => {
                self.open_local_shell("Local Shell");
            }
            session_tree_panel::TreeAction::OpenSession {
                name: _,
                session_ref,
            } => {
                if let Some(store) = &self.store {
                    match store.load_session(&session_ref) {
                        Ok(cfg) => self.request_open_session(cfg),
                        Err(err) => {
                            self.error_dialog =
                                Some(format_conn_error(&ConnError::NotFound(err.to_string())));
                            self.status = i18n::t("status.open_failed");
                        }
                    }
                }
            }
            session_tree_panel::TreeAction::AddServer { folder_id } => {
                self.begin_add_server(folder_id);
            }
            session_tree_panel::TreeAction::EditServer { session_ref } => {
                self.begin_edit_server(&session_ref);
            }
            session_tree_panel::TreeAction::DeleteServer { session_ref, name } => {
                self.delete_confirm = Some(DeleteTarget::Session { session_ref, name });
            }
            session_tree_panel::TreeAction::AddFolder { parent_id } => {
                self.folder_dialog = Some(FolderDialogState {
                    mode: FolderDialogMode::Add { parent_id },
                    name: String::new(),
                    error: None,
                    focus: true,
                });
            }
            session_tree_panel::TreeAction::RenameFolder { id, name } => {
                self.folder_dialog = Some(FolderDialogState {
                    mode: FolderDialogMode::Rename { id },
                    name,
                    error: None,
                    focus: true,
                });
            }
            session_tree_panel::TreeAction::DeleteFolder { id, name } => {
                self.delete_confirm = Some(DeleteTarget::Folder { id, name });
            }
            session_tree_panel::TreeAction::MoveSession {
                session_ref,
                name,
                folder_id,
            } => {
                self.move_session(&session_ref, &name, folder_id.as_deref());
            }
        }
    }

    fn move_session(&mut self, session_ref: &str, name: &str, folder_id: Option<&str>) {
        let current = self.tree.folder_of_session(session_ref);
        if current.as_deref() == folder_id {
            return;
        }
        let Some(store) = self.store.as_ref() else {
            self.status = i18n::t("status.save_failed");
            return;
        };
        if let Err(err) = self
            .tree
            .relocate_session(session_ref, name.to_string(), folder_id)
        {
            self.status = format!("{}: {err}", i18n::t("status.save_failed"));
            return;
        }
        if let Err(err) = store.save_tree(&self.tree) {
            self.status = format!("{}: {err}", i18n::t("status.save_failed"));
            return;
        }
        self.tree_selection = Some(TreeSelection::Session {
            name: name.to_string(),
            session_ref: session_ref.to_string(),
        });
        self.status = i18n::t("status.session_moved");
    }
}

impl eframe::App for VsTermApp {
    fn on_exit(&mut self) {
        self.remote_host.bind(None, None);
        self.remote_host.stop();
        self.metrics.stop();
        self.connections.close_all();
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // One-shot: let PTY reader threads wake egui when output arrives.
        if !self.repaint_wake_bound {
            let ctx_wake = ctx.clone();
            self.connections.set_repaint_wake(std::sync::Arc::new(move || {
                ctx_wake.request_repaint();
            }));
            self.repaint_wake_bound = true;
        }

        let connecting = self.pending_connect.is_some()
            || self.auth_prompt.is_some()
            || self
                .connections
                .list_meta()
                .iter()
                .any(|m| m.state == connection_mgr::ConnectionState::Connecting);
        let wants_metrics = self.left_tab == LeftTab::Monitor
            || matches!(
                self.main_tab,
                MainTab::SystemInfo | MainTab::Routes
            );
        let auth_animating = self.auth_prompt.is_some()
            || self.pending_spit_auth.is_some()
            || self.fx.is_active();
        schedule_repaint(ctx, connecting, wants_metrics, auth_animating);
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(i18n::t("app.name")));

        self.poll_pending_connect();
        self.connections.reap_dead();
        let gen = self.connections.generation();
        if self.host_bind_gen != gen {
            self.host_bind_gen = gen;
            self.sync_host_binding();
        }

        if let Some(from) = self.pending_spit_from.take() {
            let to = self
                .last_auth_dialog_rect
                .filter(|r| r.width() > 40.0 && r.height() > 40.0)
                .unwrap_or_else(|| crate::fx::estimated_auth_dialog_rect(ctx.screen_rect()));
            self.fx.begin_auth_spit(from, to);
        }
        if self.fx.take_spit_finished() {
            if let Some(cfg) = self.pending_spit_auth.take() {
                self.auth_prompt = Some(AuthPromptState::for_session(cfg, 1));
            }
        }

        self.show_auth_prompt(ctx);
        self.show_error_dialog(ctx);
        self.show_session_editor(ctx);
        self.show_folder_dialog(ctx);
        self.show_delete_confirm(ctx);

        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button(i18n::t("menu.file"), |ui| {
                    if ui.button(i18n::t("menu.file.add_server")).clicked() {
                        let folder_id = match &self.tree_selection {
                            Some(TreeSelection::Folder { id, .. }) => Some(id.clone()),
                            Some(TreeSelection::Session { session_ref, .. }) => {
                                self.tree.folder_of_session(session_ref)
                            }
                            None => None,
                        };
                        self.begin_add_server(folder_id);
                        ui.close_menu();
                    }
                    if ui.button(i18n::t("menu.file.add_folder")).clicked() {
                        let parent_id = match &self.tree_selection {
                            Some(TreeSelection::Folder { id, .. })
                                if self.tree.can_nest_under(id) =>
                            {
                                Some(id.clone())
                            }
                            _ => None,
                        };
                        self.folder_dialog = Some(FolderDialogState {
                            mode: FolderDialogMode::Add { parent_id },
                            name: String::new(),
                            error: None,
                            focus: true,
                        });
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(i18n::t("menu.file.refresh_tree")).clicked() {
                        self.reload_tree();
                        ui.close_menu();
                    }
                    if ui.button(i18n::t("menu.file.new_local_shell")).clicked() {
                        self.open_local_shell("Local Shell");
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(i18n::t("menu.file.exit")).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button(i18n::t("menu.connection"), |ui| {
                    if ui.button(i18n::t("menu.connection.close")).clicked() {
                        if let Some(id) = self.connections.active_id() {
                            self.connections.close(id);
                            self.sync_host_binding();
                            self.status = i18n::t("status.closed");
                        }
                        ui.close_menu();
                    }
                });
                ui.menu_button(i18n::t("menu.language"), |ui| {
                    if ui
                        .selectable_label(self.locale == Locale::ZhCn, Locale::ZhCn.label())
                        .clicked()
                    {
                        self.set_locale(Locale::ZhCn);
                        ui.close_menu();
                    }
                    if ui
                        .selectable_label(self.locale == Locale::En, Locale::En.label())
                        .clicked()
                    {
                        self.set_locale(Locale::En);
                        ui.close_menu();
                    }
                });
                ui.label(egui::RichText::new(i18n::t("app.name")).size(13.0));
            });
        });

        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            status_bar::show(ui, &self.status, self.connections.list_meta().len());
        });

        // Col1 + Col2 share ONE SidePanel so the internal drag strip uses the same
        // panel fill (dual SidePanels left a black gap when content expanded rect).
        // Outer edge vs CentralPanel matches the working col2↔col3 resize behavior.
        let side_fill = egui::Color32::from_rgb(248, 249, 250);
        let side_frame = egui::Frame::NONE
            .fill(side_fill)
            .inner_margin(egui::Margin::ZERO)
            .stroke(egui::Stroke::NONE);

        let duo_default = self.tree_width + self.list_width + 4.0;
        let mut active_tab_screen_rect: Option<egui::Rect> = None;
        let duo = egui::SidePanel::left("left_duo")
            .resizable(true)
            .default_width(duo_default)
            .width_range(360.0..=560.0)
            .frame(side_frame)
            .show_separator_line(true)
            .show(ctx, |ui| {
                let total_w = ui.available_width().max(1.0);
                let total_h = ui.available_height().max(1.0);
                let sep_w = 4.0;

                self.list_width = self.list_width.clamp(140.0, 220.0);
                let max_tree = (total_w - sep_w - 140.0).min(320.0);
                self.tree_width = self.tree_width.clamp(220.0, max_tree.max(220.0));
                if self.tree_width + sep_w + self.list_width > total_w {
                    self.list_width = (total_w - sep_w - self.tree_width).max(140.0);
                }
                let tree_w = self.tree_width.min(total_w - sep_w - 140.0).max(180.0);
                let list_w = (total_w - sep_w - tree_w).max(140.0);

                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

                    // —— Column 1 (exact width) ——
                    ui.allocate_ui_with_layout(
                        egui::vec2(tree_w, total_h),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            egui::Frame::NONE
                                .fill(side_fill)
                                .inner_margin(egui::Margin::symmetric(8, 6))
                                .show(ui, |ui| {
                                    ui.set_max_width(tree_w - 16.0);
                                    ui.horizontal(|ui| {
                                        ui.selectable_value(
                                            &mut self.left_tab,
                                            LeftTab::Servers,
                                            i18n::t("tab.servers"),
                                        );
                                        ui.selectable_value(
                                            &mut self.left_tab,
                                            LeftTab::Monitor,
                                            i18n::t("tab.monitor"),
                                        );
                                    });
                                    ui.separator();

                                    match self.left_tab {
                                        LeftTab::Servers => {
                                            let action = session_tree_panel::show(
                                                ui,
                                                &self.tree,
                                                &mut self.tree_selection,
                                            );
                                            if let Some(action) = action {
                                                self.handle_tree_action(action);
                                            }
                                        }
                                        LeftTab::Monitor => {
                                            let has = self
                                                .connections
                                                .with_active(|c| {
                                                    c.state == connection_mgr::ConnectionState::Connected
                                                })
                                                .unwrap_or(false);
                                            let snap = self.host_snapshot();
                                            let err = self.remote_host.last_error();
                                            if let Some(e) = err.as_ref() {
                                                if snap.hostname.is_empty() {
                                                    self.status = format!(
                                                        "{}: {e}",
                                                        i18n::t("monitor.loading")
                                                    );
                                                }
                                            }
                                            monitor::show(
                                                ui,
                                                &snap,
                                                &mut self.selected_nic,
                                                has,
                                                err.as_deref(),
                                            );
                                            if self.connections.active_remote().is_some() {
                                                self.remote_host
                                                    .set_selected_nic(self.selected_nic.clone());
                                            } else if self.connections.active_local_metrics() {
                                                self.metrics
                                                    .set_selected_nic(self.selected_nic.clone());
                                            }
                                        }
                                    }
                                });
                        },
                    );

                    // —— Internal splitter (panel fill, not black) ——
                    let (sep_rect, sep_resp) =
                        ui.allocate_exact_size(egui::vec2(sep_w, total_h), egui::Sense::drag());
                    ui.painter().rect_filled(sep_rect, 0.0, side_fill);
                    let line_x = sep_rect.center().x;
                    let stroke = if sep_resp.dragged() {
                        ui.style().visuals.widgets.active.fg_stroke
                    } else if sep_resp.hovered() {
                        ui.style().visuals.widgets.hovered.fg_stroke
                    } else {
                        ui.style().visuals.widgets.noninteractive.bg_stroke
                    };
                    ui.painter().vline(line_x, sep_rect.y_range(), stroke);
                    if sep_resp.hovered() || sep_resp.dragged() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                    }
                    if sep_resp.dragged() {
                        self.tree_width = (self.tree_width + sep_resp.drag_delta().x)
                            .clamp(220.0, (total_w - sep_w - 140.0).min(320.0).max(220.0));
                    }

                    // —— Column 2 (exact width, flush right into col3) ——
                    ui.allocate_ui_with_layout(
                        egui::vec2(list_w, total_h),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            egui::Frame::NONE
                                .fill(side_fill)
                                .inner_margin(egui::Margin {
                                    left: 0,
                                    right: 0,
                                    top: 4,
                                    bottom: 4,
                                })
                                .show(ui, |ui| {
                                    ui.set_min_width(list_w);
                                    ui.set_max_width(list_w);
                                    let (conn_action, tab_rect, light_rect) =
                                        connection_list::show(ui, &self.connections);
                                    if let Some(rect) = tab_rect {
                                        active_tab_screen_rect = Some(rect);
                                    }
                                    if let Some(rect) = light_rect {
                                        self.fx.settle_auth_target(rect);
                                    }
                                    match conn_action {
                                        Some(connection_list::ConnAction::Select(id)) => {
                                            self.connections.set_active(id);
                                            self.sync_host_binding();
                                            self.left_tab = LeftTab::Monitor;
                                            self.main_tab = MainTab::Terminal;
                                        }
                                        Some(connection_list::ConnAction::Close(id)) => {
                                            self.connections.close(id);
                                            self.sync_host_binding();
                                            self.status = i18n::t("status.closed");
                                        }
                                        Some(connection_list::ConnAction::Reconnect { id, light }) => {
                                            self.request_reconnect(id, Some(light));
                                        }
                                        None => {}
                                    }
                                });
                        },
                    );
                });
            });
        // Outer SidePanel resize: keep col1 width preference; col2 takes the remainder.
        let duo_w = duo.response.rect.width().clamp(360.0, 560.0);
        let sep_w = 4.0;
        self.tree_width = self
            .tree_width
            .clamp(220.0, (duo_w - sep_w - 140.0).min(320.0).max(220.0));
        self.list_width = (duo_w - sep_w - self.tree_width).clamp(140.0, 220.0);
        if self.tree_width + sep_w + self.list_width > duo_w + 0.5 {
            self.tree_width = (duo_w - sep_w - self.list_width).max(180.0);
        }

        // Mask the col2/col3 separator inside the active host tab so tab + main area look joined.
        if let Some(tab_rect) = active_tab_screen_rect {
            let sep_x = duo.response.rect.right();
            let cover = egui::Rect::from_min_max(
                egui::pos2(sep_x - 1.5, tab_rect.min.y),
                egui::pos2(sep_x + 2.0, tab_rect.max.y),
            );
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("active_host_tab_sep_mask"),
            ));
            painter.rect_filled(cover, 0.0, egui::Color32::from_rgb(255, 255, 255));
            // Re-draw top/bottom tab borders over the masked strip.
            let stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(190, 205, 230));
            painter.hline(tab_rect.x_range(), tab_rect.min.y, stroke);
            painter.hline(tab_rect.x_range(), tab_rect.max.y - 1.0, stroke);
        }

        // Third column: only show content for the currently active host.
        let central_frame = egui::Frame::NONE
            .fill(egui::Color32::from_rgb(255, 255, 255))
            .inner_margin(egui::Margin::symmetric(8, 6))
            .stroke(egui::Stroke::NONE);

        egui::CentralPanel::default().frame(central_frame).show(ctx, |ui| {
            let has_conn = self.connections.active_id().is_some();
            if !has_conn {
                // No active host → leave the main column blank.
                return;
            }

            host_toolbar::show(ui, &mut self.main_tab);
            ui.add_space(2.0);
            ui.separator();

            let snap = self.host_snapshot();
            let vault_path = self.store.as_ref().map(|s| s.paths().vault_path());
            let remote = self.connections.active_remote();

            match self.main_tab {
                MainTab::Terminal => {
                    // Strict vertical split: terminal and bottom strip never share hit-test area.
                    let gap = 6.0;
                    let bottom_h = bottom_panel::reserved_height(&self.bottom);
                    let full = ui.available_rect_before_wrap();
                    let term_h = (full.height() - bottom_h - gap).max(80.0);
                    let term_rect = egui::Rect::from_min_size(
                        full.min,
                        egui::vec2(full.width(), term_h),
                    );
                    let bottom_rect = egui::Rect::from_min_max(
                        egui::pos2(full.min.x, term_rect.max.y + gap),
                        full.max,
                    );

                    ui.allocate_ui_at_rect(term_rect, |ui| {
                        ui.set_clip_rect(term_rect);
                        let (cols, rows) = TerminalView::show(ui, &self.connections);
                        if (cols, rows) != self.last_term_size && cols > 0 && rows > 0 {
                            if let Err(err) = self.connections.resize_active(cols, rows) {
                                tracing::debug!("resize: {err}");
                            } else {
                                self.last_term_size = (cols, rows);
                            }
                        }
                    });

                    // Thin separator drawn in the gap (visual only, no interactive widgets).
                    let sep_y = term_rect.max.y + gap * 0.5;
                    ui.painter().hline(
                        term_rect.x_range(),
                        sep_y,
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(210, 214, 220)),
                    );

                    ui.allocate_ui_at_rect(bottom_rect, |ui| {
                        if let Some(cmd) =
                            bottom_panel::show(ui, &mut self.bottom, &self.commands)
                        {
                            if let Err(err) = self.connections.write_to_active(cmd.as_bytes()) {
                                self.status =
                                    format!("{}: {err}", i18n::t("status.open_failed"));
                            }
                        }
                    });

                    // Consume the full region so subsequent UI does not stack under us.
                    ui.advance_cursor_after_rect(full);
                }
                MainTab::SystemInfo => {
                    let err = self.remote_host.last_error();
                    toolbar::show_panel(ui, Some(&snap), err.as_deref());
                }
                MainTab::Routes => {
                    routes::show_panel(ui, remote.as_ref(), vault_path.as_deref());
                }
            }
        });

        self.fx.paint_overlay(ctx);
    }
}

fn bootstrap_store() -> anyhow::Result<(SessionStore, SessionTree)> {
    let paths = AppPaths::new(AppPaths::default_root());
    let store = SessionStore::new(paths)?;
    seed_demo_if_empty(&store)?;
    let tree = store.load_tree()?;
    Ok((store, tree))
}

fn load_locale() -> Locale {
    let path = AppPaths::default_root().join("config.yaml");
    if let Ok(text) = std::fs::read_to_string(path) {
        if let Ok(v) = serde_yaml::from_str::<serde_yaml::Value>(&text) {
            if let Some(code) = v.get("locale").and_then(|x| x.as_str()) {
                return Locale::from_code(code);
            }
        }
    }
    Locale::ZhCn
}

fn save_locale(locale: Locale) {
    let root = AppPaths::default_root();
    let _ = std::fs::create_dir_all(&root);
    let path = root.join("config.yaml");
    let mut map = serde_yaml::Mapping::new();
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(serde_yaml::Value::Mapping(m)) = serde_yaml::from_str(&text) {
            map = m;
        }
    }
    map.insert(
        serde_yaml::Value::String("locale".into()),
        serde_yaml::Value::String(locale.code().into()),
    );
    if let Ok(text) = serde_yaml::to_string(&serde_yaml::Value::Mapping(map)) {
        let _ = std::fs::write(path, text);
    }
}

fn seed_demo_if_empty(store: &SessionStore) -> anyhow::Result<()> {
    let tree = store.load_tree()?;
    if !tree.root.is_empty() {
        return Ok(());
    }

    use session_tree::{AuthConfig, BackendKind, SessionConfig, TreeNode};

    let web = SessionConfig {
        id: "demo-web-01".into(),
        name: "web-01".into(),
        host: "10.0.1.10".into(),
        port: 22,
        username: "deploy".into(),
        backend: BackendKind::Auto,
        auth: AuthConfig::Password {
            password_ref: None,
        },
        color_tag: Some("#ff5555".into()),
        term_type: "xterm-256color".into(),
    };
    let db = SessionConfig {
        id: "demo-db-01".into(),
        name: "db-01".into(),
        host: "10.0.1.20".into(),
        port: 22,
        username: "deploy".into(),
        backend: BackendKind::Auto,
        auth: AuthConfig::Password {
            password_ref: Some("vault://demo-db-01-pwd".into()),
        },
        color_tag: Some("#50fa7b".into()),
        term_type: "xterm-256color".into(),
    };

    store.save_session(&web)?;
    store.save_session(&db)?;

    if let Ok(mut vault) = vault::Vault::open(store.paths().vault_path()) {
        let _ = vault.set("demo-db-01-pwd", "demo-password");
    }

    let tree = SessionTree {
        root: vec![
            TreeNode::Folder {
                name: "生产环境".into(),
                id: "f001".into(),
                children: vec![
                    TreeNode::Session {
                        name: "web-01".into(),
                        session_ref: "demo-web-01.yaml".into(),
                    },
                    TreeNode::Session {
                        name: "db-01".into(),
                        session_ref: "demo-db-01.yaml".into(),
                    },
                ],
            },
            TreeNode::Folder {
                name: "测试环境".into(),
                id: "f003".into(),
                children: vec![],
            },
        ],
    };
    store.save_tree(&tree)?;
    Ok(())
}

/// Event-driven painting: wake on input / terminal output; slow timer only for
/// cursor blink, connecting polls, and metrics panels. Avoids a fixed ~30 FPS loop.
fn schedule_repaint(ctx: &egui::Context, connecting: bool, wants_metrics: bool, fx_active: bool) {
    use std::time::Duration;

    let interactive = ctx.input(|i| {
        i.pointer.any_down()
            || i.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::Text(_)
                        | egui::Event::Paste(_)
                        | egui::Event::Key {
                            pressed: true,
                            ..
                        }
                        | egui::Event::PointerButton { pressed: true, .. }
                        | egui::Event::MouseWheel { .. }
                )
            })
    });

    if interactive || fx_active {
        ctx.request_repaint();
    }

    let ms = if fx_active {
        16
    } else if connecting {
        200
    } else if wants_metrics {
        400
    } else {
        // Terminal cursor blink cadence when idle.
        530
    };
    ctx.request_repaint_after(Duration::from_millis(ms));
}

