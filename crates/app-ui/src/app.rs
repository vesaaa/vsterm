use crate::commands::CommandBook;
use crate::conn_error::{format_conn_error, ConnErrorDisplay};
use crate::fx::{ConnectFxMode, FxLayer};
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
use connection_mgr::{ConnectFailure, ConnectionManager, ConnError, ConnErrorKey, ZmodemStatus};
use eframe::egui;
use session_tree::{AppPaths, SessionConfig, SessionStore, SessionTree, TreeNode};
use std::collections::HashMap;
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
    /// Connect / reconnect motion preference (trail / shatter / off).
    connect_fx: ConnectFxMode,
    pending_connect: Option<PendingConnect>,
    error_dialog: Option<ConnErrorDisplay>,
    auth_prompt: Option<AuthPromptState>,
    /// Host tab to replace after auth succeeds (status-light reconnect).
    reconnect_target: Option<connection_mgr::ConnectionId>,
    host_bind_gen: u64,
    tree_selection: Option<TreeSelection>,
    /// `session_ref` → configured/detected OS icon id (`None` = auto / unknown).
    session_icons: HashMap<String, Option<String>>,
    session_editor: Option<SessionEditorState>,
    folder_dialog: Option<FolderDialogState>,
    delete_confirm: Option<DeleteTarget>,
    /// Whether `set_repaint_wake` has been bound to this egui context.
    repaint_wake_bound: bool,
    /// Soft-glow / connect-ripple motion layer.
    fx: FxLayer,
    /// Last painted auth dialog rect (for suck-in / spit-out FX).
    last_auth_dialog_rect: Option<egui::Rect>,
    /// Prompt held while the spit-out morph plays; shown when the morph lands.
    pending_spit_auth: Option<AuthPromptState>,
    /// Status-light rect (spit origin), pending until we've measured the dialog size.
    pending_spit_from: Option<egui::Rect>,
    /// True while an off-screen auth dialog is being measured for the spit target.
    spit_measuring: bool,
    /// Central (terminal) region from the last frame — shatter shards disperse here.
    last_central_rect: Option<egui::Rect>,
    /// Deferred OS file picker for remote `rz` / `sz` (ZMODEM).
    pending_zmodem_pick: Option<PendingZmodemPick>,
}

/// Two-phase arming so the picker opens after a clean paint (same idea as SFTP).
enum ZmodemPickPhase {
    Arm,
    Open,
}

#[derive(Clone)]
enum ZmodemPickKind {
    Upload { prompt_id: u64 },
    SaveAs {
        suggested_name: String,
        prompt_id: u64,
    },
}

struct PendingZmodemPick {
    phase: ZmodemPickPhase,
    kind: ZmodemPickKind,
}

impl VsTermApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        fonts::install(&cc.egui_ctx);
        theme::apply(&cc.egui_ctx);
        crate::sys_file_icon::warm_up();

        let locale = load_locale();
        i18n::set(locale);
        let connect_fx = load_connect_fx();

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

        let mut session_icons = HashMap::new();
        if let Some(store) = &store {
            rebuild_session_icons(store, &tree, &mut session_icons);
        }

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
            // Fixed column widths — not user-resizable.
            tree_width: 260.0,
            list_width: 168.0,
            bottom: BottomPanelState::default(),
            status,
            last_term_size: (80, 24),
            locale,
            connect_fx,
            pending_connect: None,
            error_dialog: None,
            auth_prompt: None,
            reconnect_target: None,
            host_bind_gen: 0,
            tree_selection: None,
            session_icons,
            session_editor: None,
            folder_dialog: None,
            delete_confirm: None,
            repaint_wake_bound: false,
            fx: FxLayer::default(),
            last_auth_dialog_rect: None,
            pending_spit_auth: None,
            pending_spit_from: None,
            spit_measuring: false,
            last_central_rect: None,
            pending_zmodem_pick: None,
        }
    }

    fn reload_tree(&mut self) {
        if let Some(store) = &self.store {
            match store.load_tree() {
                Ok(tree) => {
                    self.tree = tree;
                    rebuild_session_icons(store, &self.tree, &mut self.session_icons);
                    self.status = i18n::t("status.tree_reloaded");
                }
                Err(err) => self.status = format!("{}: {err}", i18n::t("status.open_failed")),
            }
        }
    }

    /// When icon is auto (`None`) and remote metrics report an OS id, persist it.
    fn maybe_writeback_session_icon(&mut self) {
        let Some(os_id) = self
            .remote_host
            .snapshot()
            .and_then(|s| s.os_id.clone())
        else {
            return;
        };
        let Some(session_id) = self
            .connections
            .with_active(|c| c.session_id.clone())
            .flatten()
        else {
            return;
        };
        let session_ref = if session_id.ends_with(".yaml") {
            session_id.clone()
        } else {
            format!("{session_id}.yaml")
        };
        // Already have a fixed icon in cache?
        if self
            .session_icons
            .get(&session_ref)
            .and_then(|o| o.as_ref())
            .is_some()
        {
            return;
        }
        let Some(store) = self.store.as_ref() else {
            return;
        };
        let Ok(mut cfg) = store.load_session(&session_ref) else {
            return;
        };
        if cfg.icon.is_some() {
            self.session_icons
                .insert(session_ref, cfg.icon.clone());
            return;
        }
        cfg.icon = Some(os_id.clone());
        if store.save_session(&cfg).is_ok() {
            self.session_icons
                .insert(session_ref, Some(os_id));
        }
    }

    fn open_local_shell(&mut self, title: impl Into<String>) {
        match self.connections.open_local_shell(title) {
            Ok(_) => {
                self.status = i18n::t("status.opened_shell");
                // Local shell ≡ a host tab: open Monitor so the rail tracks this host.
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
        if !self.connect_fx.connect_animated() {
            // No entrance animation — open the prompt immediately.
            self.auth_prompt = Some(AuthPromptState::for_session(config, 1));
            self.pending_spit_auth = None;
            self.pending_spit_from = None;
            self.spit_measuring = false;
            return;
        }
        // Render the real dialog off-screen for one frame to capture its exact
        // size, so the entrance animation lands precisely where the dialog appears.
        let mut prompt = AuthPromptState::for_session(config, 1);
        prompt.measure_only = true;
        self.auth_prompt = Some(prompt);
        self.pending_spit_auth = None;
        self.pending_spit_from = Some(from_light);
        self.spit_measuring = true;
    }

    /// Returns `Err(())` after surfacing an error dialog / clearing reconnect.
    fn preflight_before_auth(&mut self, config: &SessionConfig) -> Result<(), ()> {
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
                // Connected host tab ↔ Monitor rail (active host only samples).
                self.left_tab = LeftTab::Monitor;
                self.main_tab = MainTab::Terminal;
                self.sync_host_binding();
                self.status = i18n::t("status.connected");
                if let Some(rect) = self.last_auth_dialog_rect.take() {
                    let accent = crate::fx::accent_from_tag(pending.config.color_tag.as_deref());
                    match self.connect_fx {
                        ConnectFxMode::Trail => self.fx.begin_auth_suck(rect, accent),
                        ConnectFxMode::Shatter => {
                            let zone = self.last_central_rect.unwrap_or(rect);
                            if let Some(p) = self.auth_prompt.as_ref() {
                                self.fx.begin_auth_shatter_out(
                                    rect,
                                    accent,
                                    zone,
                                    p.shatter_face(),
                                );
                            }
                        }
                        ConnectFxMode::Off => {}
                    }
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
                    let msg = failure
                        .detail
                        .clone()
                        .unwrap_or_else(|| i18n::t("status.open_failed"));
                    let auth_exhausted = matches!(
                        failure.key,
                        ConnErrorKey::AuthFailed | ConnErrorKey::PrivateKeyMissing
                    );
                    if auth_exhausted {
                        // No modal — leave / create a Failed host tab; status light reopens auth.
                        if let Some(id) = pending.replace_id {
                            self.connections.mark_failed(id, msg);
                        } else {
                            let id = self
                                .connections
                                .insert_failed_host(&pending.config, msg);
                            self.connections.set_active(id);
                            self.left_tab = LeftTab::Monitor;
                            self.main_tab = MainTab::Terminal;
                        }
                        self.sync_host_binding();
                        self.auth_prompt = None;
                        self.status = i18n::t("dialog.password.max_attempts");
                    } else {
                        if let Some(id) = pending.replace_id {
                            self.connections.mark_failed(id, msg);
                        }
                        self.auth_prompt = None;
                        self.error_dialog =
                            Some(crate::conn_error::format_connect_failure(&failure));
                        self.status = i18n::t("status.open_failed");
                    }
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
        let mut spawn_sparks = None;
        let motion = self.connect_fx.motion_enabled();
        if let Some(prompt) = &mut self.auth_prompt {
            let (act, rect) = connect_auth::show(ctx, prompt);
            action = act;
            if let Some(r) = rect {
                self.last_auth_dialog_rect = Some(r);
                // Spark ring once a fresh (non-error, visible) dialog first renders.
                if !prompt.measure_only
                    && !prompt.sparked
                    && prompt.warn.is_none()
                    && !prompt.verifying
                {
                    prompt.sparked = true;
                    if motion {
                        spawn_sparks = Some((
                            r,
                            crate::fx::accent_from_tag(prompt.config.color_tag.as_deref()),
                        ));
                    }
                }
            }
        }
        if let Some((rect, accent)) = spawn_sparks {
            self.fx.begin_dialog_sparks(rect, accent);
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
        let _title_bar = crate::dialog_chrome::CompactTitleBar::push(ctx);
        egui::Window::new(crate::dialog_chrome::title(i18n::t("dialog.error.title")))
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
                crate::dialog_chrome::centered_actions(ui, |ui| {
                    if ui.button(i18n::t("dialog.error.ok")).clicked() {
                        self.error_dialog = None;
                    }
                });
            });
        if !open {
            self.error_dialog = None;
        }
    }

    fn set_locale(&mut self, locale: Locale) {
        self.locale = locale;
        i18n::set(locale);
        save_app_config(locale, self.connect_fx);
        self.status = i18n::t("status.lang_changed");
    }

    fn set_connect_fx(&mut self, mode: ConnectFxMode) {
        self.connect_fx = mode;
        if !mode.connect_animated() {
            // Drop any in-flight morph when turning effects off.
            self.fx = FxLayer::default();
            if self.spit_measuring {
                if let Some(mut p) = self.auth_prompt.take() {
                    p.measure_only = false;
                    self.auth_prompt = Some(p);
                } else if let Some(mut p) = self.pending_spit_auth.take() {
                    p.measure_only = false;
                    self.auth_prompt = Some(p);
                }
                self.pending_spit_from = None;
                self.spit_measuring = false;
            }
        }
        save_app_config(self.locale, mode);
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

        self.session_icons
            .insert(session_ref.clone(), built.config.icon.clone());
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
        self.session_icons.remove(session_ref);
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
            let _title_bar = crate::dialog_chrome::CompactTitleBar::push(ctx);
            egui::Window::new(crate::dialog_chrome::title(title))
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
                    crate::dialog_chrome::centered_actions(ui, |ui| {
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
        let _title_bar = crate::dialog_chrome::CompactTitleBar::push(ctx);
        egui::Window::new(crate::dialog_chrome::title(i18n::t("dialog.delete.title")))
            .id(egui::Id::new("delete_confirm"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(380.0)
            .show(ctx, |ui| {
                ui.label(body);
                ui.add_space(12.0);
                crate::dialog_chrome::centered_actions(ui, |ui| {
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
        // Opt-in perf probe (set VSTERM_DIAG=1): prints real repaint rate and
        // per-frame UI cost every 2 s. Lets us tell a continuous-repaint bug
        // (fps pinned high) from expensive frames / background CPU (fps low).
        let _diag = FrameDiag::start();
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
        // Sampling rules (product):
        // - Connect / activate a host tab → left rail opens Monitor (see connect
        //   / Select paths). Collect only for the *active* host while Monitor
        //   is open, or while System Info is open (needs the same snapshot).
        // - Non-active host tabs: `sync_host_binding` rebinds/clears the target
        //   so their collectors stop; only one RemoteHostService target exists.
        // - Routes: fetch only inside the Routes panel when that main tab is
        //   selected — do not keep metrics sampling alive for Routes alone.
        let monitor_rail = self.left_tab == LeftTab::Monitor;
        let system_info = self.main_tab == MainTab::SystemInfo;
        let wants_live_host_data = monitor_rail || system_info;
        let has_local_host = self.connections.active_local_metrics();
        let has_remote_host = self.connections.active_remote().is_some();
        self.metrics
            .set_active(wants_live_host_data && has_local_host);
        // Sampling is set below once we know whether a transfer is active.
        // Do not keep a 400 ms chart timer alive when the Monitor/System Info
        // surface has no host from which it could obtain data.
        let wants_metrics = wants_live_host_data && (has_local_host || has_remote_host);
        let auth_animating = self.auth_prompt.is_some()
            || self.pending_spit_auth.is_some()
            || self.fx.is_active();
        let transfer_active = bottom_panel::needs_transfer_poll(&self.bottom)
            || matches!(
                self.connections.active_zmodem_status(),
                Some(
                    ZmodemStatus::Receiving { .. }
                        | ZmodemStatus::Sending { .. }
                        | ZmodemStatus::AwaitingUpload { .. }
                        | ZmodemStatus::AwaitingSaveAs { .. }
                )
            );
        self.poll_zmodem();
        // Remote metrics share the same SSH session/runtime as the interactive
        // shell and SFTP. Keep collecting only when the UI needs it *and* no
        // transfer is saturating the session — otherwise typing echo waits
        // behind pipelined reads / exec channels.
        self.remote_host.set_sampling(
            wants_live_host_data && has_remote_host && !transfer_active,
        );
        let menu_open = ctx.is_context_menu_open()
            || ctx.memory(|m| m.any_popup_open());
        schedule_repaint(
            ctx,
            connecting,
            wants_metrics,
            auth_animating,
            transfer_active,
            menu_open,
        );

        self.poll_pending_connect();
        self.connections.reap_dead();
        let gen = self.connections.generation();
        if self.host_bind_gen != gen {
            self.host_bind_gen = gen;
            self.sync_host_binding();
        }
        self.maybe_writeback_session_icon();

        if self.fx.take_spit_finished() {
            // Morph landed on the dialog rect — reveal the real (input-ready) dialog.
            self.auth_prompt = self.pending_spit_auth.take();
        }

        self.show_auth_prompt(ctx);

        // Off-screen measurement finished this frame → launch the entrance
        // animation toward the dialog's true, centered rect, then hide it until
        // the animation lands (Trail spit morph or Shatter re-assembly).
        if self.spit_measuring {
            let measured = self
                .last_auth_dialog_rect
                .filter(|r| r.width() > 40.0 && r.height() > 40.0);
            if let (Some(from), Some(size_rect)) = (self.pending_spit_from, measured) {
                let target = egui::Rect::from_center_size(
                    ctx.screen_rect().center(),
                    size_rect.size(),
                );
                let mut prompt = self.auth_prompt.take();
                let accent = prompt
                    .as_ref()
                    .map(|p| crate::fx::accent_from_tag(p.config.color_tag.as_deref()))
                    .unwrap_or(crate::fx::DEFAULT_ACCENT);
                if let Some(p) = prompt.as_mut() {
                    p.measure_only = false;
                }
                match self.connect_fx {
                    ConnectFxMode::Shatter => {
                        let zone = self.last_central_rect.unwrap_or(target);
                        if let Some(p) = prompt.as_ref() {
                            self.fx
                                .begin_auth_shatter_in(target, accent, zone, p.shatter_face());
                        } else {
                            self.fx.begin_auth_spit(from, target, accent);
                        }
                    }
                    _ => self.fx.begin_auth_spit(from, target, accent),
                }
                self.last_auth_dialog_rect = Some(target);
                self.pending_spit_auth = prompt;
                self.pending_spit_from = None;
                self.spit_measuring = false;
            }
        }
        self.show_error_dialog(ctx);
        self.show_session_editor(ctx);
        self.show_folder_dialog(ctx);
        self.show_delete_confirm(ctx);

        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                use crate::ctx_menu;
                use crate::ui_icon::Icon;

                ui.menu_button(i18n::t("menu.file"), |ui| {
                    ctx_menu::prepare(ui);
                    if ctx_menu::item(
                        ui,
                        Some(Icon::Server),
                        &i18n::t("menu.file.add_server"),
                        None,
                        true,
                    )
                    .clicked()
                    {
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
                    if ctx_menu::item(
                        ui,
                        Some(Icon::FolderPlus),
                        &i18n::t("menu.file.add_folder"),
                        None,
                        true,
                    )
                    .clicked()
                    {
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
                    ctx_menu::separator(ui);
                    if ctx_menu::item(
                        ui,
                        Some(Icon::RefreshCw),
                        &i18n::t("menu.file.refresh_tree"),
                        None,
                        true,
                    )
                    .clicked()
                    {
                        self.reload_tree();
                        ui.close_menu();
                    }
                    if ctx_menu::item(
                        ui,
                        Some(Icon::Terminal),
                        &i18n::t("menu.file.new_local_shell"),
                        None,
                        true,
                    )
                    .clicked()
                    {
                        self.open_local_shell("Local Shell");
                        ui.close_menu();
                    }
                    ctx_menu::separator(ui);
                    if ctx_menu::item(
                        ui,
                        Some(Icon::LogOut),
                        &i18n::t("menu.file.exit"),
                        None,
                        true,
                    )
                    .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button(i18n::t("menu.connection"), |ui| {
                    ctx_menu::prepare(ui);
                    let can_close = self.connections.active_id().is_some();
                    if ctx_menu::item(
                        ui,
                        Some(Icon::Unplug),
                        &i18n::t("menu.connection.close"),
                        None,
                        can_close,
                    )
                    .clicked()
                    {
                        if let Some(id) = self.connections.active_id() {
                            self.close_host_tab(id);
                        }
                        ui.close_menu();
                    }
                });
                ui.menu_button(i18n::t("menu.options"), |ui| {
                    ctx_menu::prepare(ui);
                    ctx_menu::submenu(ui, Some(Icon::Languages), &i18n::t("menu.language"), |ui| {
                        if ctx_menu::check_item(
                            ui,
                            None,
                            Locale::ZhCn.label(),
                            self.locale == Locale::ZhCn,
                            true,
                        )
                        .clicked()
                        {
                            self.set_locale(Locale::ZhCn);
                            ui.close_menu();
                        }
                        if ctx_menu::check_item(
                            ui,
                            None,
                            Locale::En.label(),
                            self.locale == Locale::En,
                            true,
                        )
                        .clicked()
                        {
                            self.set_locale(Locale::En);
                            ui.close_menu();
                        }
                    });
                    ctx_menu::submenu(
                        ui,
                        Some(Icon::Sparkles),
                        &i18n::t("menu.options.effects"),
                        |ui| {
                            ctx_menu::submenu(
                                ui,
                                Some(Icon::Plug),
                                &i18n::t("menu.options.effects.connect"),
                                |ui| {
                                    for (mode, key) in [
                                        (
                                            ConnectFxMode::Trail,
                                            "menu.options.effects.connect.trail",
                                        ),
                                        (
                                            ConnectFxMode::Shatter,
                                            "menu.options.effects.connect.shatter",
                                        ),
                                        (
                                            ConnectFxMode::Off,
                                            "menu.options.effects.connect.off",
                                        ),
                                    ] {
                                        if ctx_menu::check_item(
                                            ui,
                                            None,
                                            &i18n::t(key),
                                            self.connect_fx == mode,
                                            true,
                                        )
                                        .clicked()
                                        {
                                            self.set_connect_fx(mode);
                                            ui.close_menu();
                                        }
                                    }
                                },
                            );
                        },
                    );
                });
                ui.label(egui::RichText::new(i18n::t("app.name")).size(13.0));
            });
        });

        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            let zmodem = self.connections.active_zmodem_status();
            let zmodem_busy = matches!(
                zmodem,
                Some(
                    ZmodemStatus::Receiving { .. }
                        | ZmodemStatus::Sending { .. }
                        | ZmodemStatus::AwaitingUpload { .. }
                        | ZmodemStatus::AwaitingSaveAs { .. }
                )
            );
            let zmodem_progress = zmodem.as_ref().and_then(|s| s.progress_fraction());
            status_bar::show(
                ui,
                &self.status,
                self.connections.list_meta().len(),
                crate::render_policy::is_software_renderer(),
                zmodem_busy,
                zmodem_progress,
                || {
                    let _ = self.connections.cancel_zmodem();
                },
            );
        });

        // Col1 + Col2 share ONE SidePanel (fixed widths, no resize).
        let side_fill = egui::Color32::from_rgb(248, 249, 250);
        let side_frame = egui::Frame::NONE
            .fill(side_fill)
            .inner_margin(egui::Margin::ZERO)
            .stroke(egui::Stroke::NONE);

        const SEP_W: f32 = 4.0;
        let tree_w = self.tree_width;
        let list_w = self.list_width;
        let duo_w = tree_w + SEP_W + list_w;
        let mut active_tab_screen_rect: Option<egui::Rect> = None;
        let duo = egui::SidePanel::left("left_duo")
            .resizable(false)
            .default_width(duo_w)
            .width_range(duo_w..=duo_w)
            .frame(side_frame)
            .show_separator_line(true)
            .show(ctx, |ui| {
                let total_h = ui.available_height().max(1.0);

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
                                                &self.session_icons,
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

                    // —— Column divider (visual only) ——
                    let (sep_rect, _) =
                        ui.allocate_exact_size(egui::vec2(SEP_W, total_h), egui::Sense::hover());
                    ui.painter().rect_filled(sep_rect, 0.0, side_fill);
                    ui.painter().vline(
                        sep_rect.center().x,
                        sep_rect.y_range(),
                        ui.style().visuals.widgets.noninteractive.bg_stroke,
                    );

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
                                            self.close_host_tab(id);
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
            // Re-stroke only the wiped segment on the col2 side — stop at the
            // divider so borders don't spill into the main column.
            let stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(190, 205, 230));
            let border_x = egui::Rangef::new(cover.min.x, sep_x);
            painter.hline(border_x, tab_rect.min.y, stroke);
            painter.hline(border_x, tab_rect.max.y - 1.0, stroke);
        }

        // Third column: only show content for the currently active host.
        let central_frame = egui::Frame::NONE
            .fill(egui::Color32::from_rgb(255, 255, 255))
            .inner_margin(egui::Margin {
                left: 8,
                right: 8,
                top: 6,
                bottom: 0,
            })
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

            let vault_path = self.store.as_ref().map(|s| s.paths().vault_path());
            let remote = self.connections.active_remote();
            let active_cwd = self.connections.active_cwd();

            match self.main_tab {
                MainTab::Terminal => {
                    // Vertical split: terminal | draggable gap | bottom (files/commands).
                    let gap = bottom_panel::SPLIT_GAP;
                    let full = ui.available_rect_before_wrap();
                    self.bottom.height =
                        bottom_panel::clamp_body_height(self.bottom.height, full.height());
                    let bottom_h = bottom_panel::reserved_height(&self.bottom);
                    let term_h = (full.height() - bottom_h - gap).max(bottom_panel::MIN_TERM_HEIGHT);
                    let term_rect = egui::Rect::from_min_size(
                        full.min,
                        egui::vec2(full.width(), term_h),
                    );
                    self.last_central_rect = Some(term_rect);
                    self.fx.set_shatter_scatter(term_rect);

                    let sep_rect = egui::Rect::from_min_max(
                        egui::pos2(full.min.x, term_rect.max.y),
                        egui::pos2(full.max.x, term_rect.max.y + gap),
                    );
                    let bottom_rect = egui::Rect::from_min_max(
                        egui::pos2(full.min.x, sep_rect.max.y),
                        egui::pos2(full.max.x, ui.max_rect().max.y),
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

                    // Draggable separator (drag up → taller bottom panel).
                    let sep_id = ui.id().with("term_bottom_split");
                    let sep_resp = ui.interact(sep_rect, sep_id, egui::Sense::drag());
                    let stroke = if sep_resp.dragged() {
                        ui.style().visuals.widgets.active.fg_stroke
                    } else if sep_resp.hovered() {
                        ui.style().visuals.widgets.hovered.fg_stroke
                    } else {
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(210, 214, 220))
                    };
                    ui.painter().hline(sep_rect.x_range(), sep_rect.center().y, stroke);
                    if sep_resp.hovered() || sep_resp.dragged() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                    }
                    if sep_resp.dragged() {
                        // Pointer up (negative Δy) grows the bottom body.
                        self.bottom.height = bottom_panel::clamp_body_height(
                            self.bottom.height - sep_resp.drag_delta().y,
                            full.height(),
                        );
                    }

                    ui.allocate_ui_at_rect(bottom_rect, |ui| {
                        if let Some(cmd) = bottom_panel::show(
                            ui,
                            &mut self.bottom,
                            &self.commands,
                            remote.as_ref(),
                            active_cwd.clone(),
                        ) {
                            if let Err(err) = self.connections.write_to_active(cmd.as_bytes()) {
                                self.status =
                                    format!("{}: {err}", i18n::t("status.open_failed"));
                            }
                        }
                    });

                    ui.advance_cursor_after_rect(full);
                }
                MainTab::SystemInfo => {
                    let snap = self.host_snapshot();
                    let connected = self
                        .connections
                        .with_active(|c| {
                            c.state == connection_mgr::ConnectionState::Connected
                        })
                        .unwrap_or(false);
                    if connected {
                        let err = self.remote_host.last_error();
                        toolbar::show_panel(ui, Some(&snap), err.as_deref());
                    } else {
                        // Do not show empty-snap "loading" or any stale host view.
                        toolbar::show_panel(ui, None, None);
                    }
                }
                MainTab::Routes => {
                    let connected = self
                        .connections
                        .with_active(|c| {
                            c.state == connection_mgr::ConnectionState::Connected
                        })
                        .unwrap_or(false);
                    let source = if !connected {
                        routes::RoutesSource::Disconnected
                    } else if self.connections.active_local_metrics() {
                        routes::RoutesSource::Local
                    } else if let Some(r) = remote.as_ref() {
                        routes::RoutesSource::Remote(r)
                    } else {
                        routes::RoutesSource::Disconnected
                    };
                    routes::show_panel(ui, source, vault_path.as_deref());
                }
            }
        });

        self.fx.paint_overlay(ctx);

        // Open OS file dialogs only after egui has painted a frame with the
        // context menu closed (click → close_menu → this arm → next paint → open).
        match bottom_panel::take_native_dialog_after_paint(&mut self.bottom) {
            Some(kind) => {
                // rfd blocks the UI thread for the whole picker; attribute it
                // explicitly so it is not mistaken for download-end jank.
                let t0 = std::time::Instant::now();
                let remote = self.connections.active_remote();
                bottom_panel::run_native_dialog(&mut self.bottom, remote.as_ref(), kind);
                if std::env::var_os("VSTERM_DIAG").is_some() {
                    tracing::warn!(
                        "VSTERM_DIAG: native file dialog blocked UI for {:.0} ms",
                        t0.elapsed().as_secs_f64() * 1000.0
                    );
                }
                // `schedule_repaint` already ran *before* this blocking dialog.
                // Any `request_repaint_after` from that call can expire while
                // rfd holds the UI thread, and a transfer started here would
                // never schedule its 33 ms poll until some unrelated wake —
                // matching "2nd save → seconds with no progress bar → finishes
                // instantly". Always wake immediately after the dialog returns.
                ctx.request_repaint();
            }
            None if bottom_panel::has_pending_native_dialog(&self.bottom) => {
                ctx.request_repaint();
            }
            None => {}
        }

        match self.pending_zmodem_pick.take() {
            Some(PendingZmodemPick {
                phase: ZmodemPickPhase::Arm,
                kind,
            }) => {
                // Drop stale arms if the transfer already finished.
                let still_waiting = match &kind {
                    ZmodemPickKind::Upload { prompt_id } => matches!(
                        self.connections.active_zmodem_status(),
                        Some(ZmodemStatus::AwaitingUpload { prompt_id: id }) if id == *prompt_id
                    ),
                    ZmodemPickKind::SaveAs { prompt_id, .. } => matches!(
                        self.connections.active_zmodem_status(),
                        Some(ZmodemStatus::AwaitingSaveAs { prompt_id: id, .. }) if id == *prompt_id
                    ),
                };
                if still_waiting {
                    self.pending_zmodem_pick = Some(PendingZmodemPick {
                        phase: ZmodemPickPhase::Open,
                        kind,
                    });
                    ctx.request_repaint();
                }
            }
            Some(PendingZmodemPick {
                phase: ZmodemPickPhase::Open,
                kind: ZmodemPickKind::Upload { prompt_id },
            }) => {
                let still = matches!(
                    self.connections.active_zmodem_status(),
                    Some(ZmodemStatus::AwaitingUpload { prompt_id: id }) if id == prompt_id
                );
                if still {
                    let paths = rfd::FileDialog::new()
                        .set_title(i18n::t("zmodem.pick_title"))
                        .pick_files()
                        .unwrap_or_default();
                    match self.connections.provide_zmodem_upload(paths) {
                        Ok(()) => {}
                        Err(err) => {
                            self.status = format!("ZMODEM: {err}");
                        }
                    }
                }
                ctx.request_repaint();
            }
            Some(PendingZmodemPick {
                phase: ZmodemPickPhase::Open,
                kind: ZmodemPickKind::SaveAs {
                    suggested_name,
                    prompt_id,
                },
            }) => {
                let still = matches!(
                    self.connections.active_zmodem_status(),
                    Some(ZmodemStatus::AwaitingSaveAs { prompt_id: id, .. }) if id == prompt_id
                );
                if still {
                    let mut dlg = rfd::FileDialog::new()
                        .set_title(i18n::t("zmodem.save_title"))
                        .set_file_name(&suggested_name);
                    if let Some(dir) = self.connections.active_zmodem_download_dir() {
                        dlg = dlg.set_directory(dir);
                    }
                    let path = dlg.save_file();
                    match self.connections.provide_zmodem_download(path) {
                        Ok(()) => {}
                        Err(err) => {
                            self.status = format!("ZMODEM: {err}");
                        }
                    }
                }
                ctx.request_repaint();
            }
            None => {}
        }
    }
}

impl VsTermApp {
    /// Close a host tab: cancel SFTP/ZMODEM, unbind metrics, drop SSH/PTY.
    fn close_host_tab(&mut self, id: connection_mgr::ConnectionId) {
        let host_key = self.connections.remote_display_key(id);
        // Always clear metrics bind if it points at this host (keeps RemoteSession
        // Arcs from pinning the SSH session during teardown).
        if let Some(key) = host_key.as_deref() {
            self.remote_host.forget_host(key);
            bottom_panel::shutdown_host(&mut self.bottom, key);
        } else if self.connections.active_id() == Some(id) {
            self.remote_host.bind(None, None);
        }
        self.connections.close(id);
        self.sync_host_binding();
        self.status = i18n::t("status.closed");
    }

    fn poll_zmodem(&mut self) {
        match self.connections.active_zmodem_status() {
            Some(ZmodemStatus::Receiving {
                file_name,
                bytes,
                total,
            }) => {
                self.status =
                    format_zmodem_progress(i18n::t("zmodem.receiving"), &file_name, bytes, total);
                self.pending_zmodem_pick = None;
            }
            Some(ZmodemStatus::Sending {
                file_name,
                bytes,
                total,
            }) => {
                self.status =
                    format_zmodem_progress(i18n::t("zmodem.sending"), &file_name, bytes, total);
                self.pending_zmodem_pick = None;
            }
            Some(ZmodemStatus::AwaitingUpload { prompt_id }) => {
                self.status = i18n::t("zmodem.await_upload");
                let already = matches!(
                    &self.pending_zmodem_pick,
                    Some(PendingZmodemPick {
                        kind: ZmodemPickKind::Upload { prompt_id: id },
                        ..
                    }) if *id == prompt_id
                );
                if !already {
                    self.pending_zmodem_pick = Some(PendingZmodemPick {
                        phase: ZmodemPickPhase::Arm,
                        kind: ZmodemPickKind::Upload { prompt_id },
                    });
                }
            }
            Some(ZmodemStatus::AwaitingSaveAs {
                suggested_name,
                total,
                prompt_id,
            }) => {
                self.status = match total {
                    Some(t) if t > 0 => format!(
                        "{}: {} ({})",
                        i18n::t("zmodem.await_save"),
                        suggested_name,
                        fmt_bytes(t)
                    ),
                    _ => format!("{}: {suggested_name}", i18n::t("zmodem.await_save")),
                };
                let already = matches!(
                    &self.pending_zmodem_pick,
                    Some(PendingZmodemPick {
                        kind: ZmodemPickKind::SaveAs { prompt_id: id, .. },
                        ..
                    }) if *id == prompt_id
                );
                if !already {
                    self.pending_zmodem_pick = Some(PendingZmodemPick {
                        phase: ZmodemPickPhase::Arm,
                        kind: ZmodemPickKind::SaveAs {
                            suggested_name,
                            prompt_id,
                        },
                    });
                }
            }
            Some(ZmodemStatus::Done { summary }) => {
                self.status = summary;
                self.connections.clear_zmodem_finished();
                self.pending_zmodem_pick = None;
            }
            Some(ZmodemStatus::Failed { message }) => {
                self.status = message;
                self.connections.clear_zmodem_finished();
                self.pending_zmodem_pick = None;
            }
            Some(ZmodemStatus::Idle) | None => {
                // Drop any deferred picker — transfer is over, keyboard is free.
                self.pending_zmodem_pick = None;
            }
        }
    }
}

fn format_zmodem_progress(kind: String, name: &str, bytes: u64, total: Option<u64>) -> String {
    let name = if name.is_empty() { "…" } else { name };
    match total {
        Some(t) if t > 0 => {
            let pct = ((bytes as f64 / t as f64) * 100.0).clamp(0.0, 100.0);
            format!("{kind}: {name}  {}/{}  ({pct:.0}%)", fmt_bytes(bytes), fmt_bytes(t))
        }
        _ => format!("{kind}: {name}  {}", fmt_bytes(bytes)),
    }
}

fn fmt_bytes(n: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let n = n as f64;
    if n >= GB {
        format!("{:.2} GB", n / GB)
    } else if n >= MB {
        format!("{:.1} MB", n / MB)
    } else if n >= KB {
        format!("{:.0} KB", n / KB)
    } else {
        format!("{n:.0} B")
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
    load_app_config().0
}

fn load_connect_fx() -> ConnectFxMode {
    load_app_config().1
}

fn load_app_config() -> (Locale, ConnectFxMode) {
    let path = AppPaths::default_root().join("config.yaml");
    let mut locale = Locale::ZhCn;
    let mut connect_fx = ConnectFxMode::default();
    if let Ok(text) = std::fs::read_to_string(path) {
        if let Ok(v) = serde_yaml::from_str::<serde_yaml::Value>(&text) {
            if let Some(code) = v.get("locale").and_then(|x| x.as_str()) {
                locale = Locale::from_code(code);
            }
            if let Some(code) = v.get("connect_fx").and_then(|x| x.as_str()) {
                connect_fx = ConnectFxMode::from_code(code);
            }
        }
    }
    (locale, connect_fx)
}

fn rebuild_session_icons(
    store: &SessionStore,
    tree: &SessionTree,
    out: &mut HashMap<String, Option<String>>,
) {
    out.clear();
    for node in tree.walk() {
        if let TreeNode::Session { session_ref, .. } = node {
            let icon = store
                .load_session(session_ref)
                .map(|c| c.icon)
                .unwrap_or(None);
            out.insert(session_ref.clone(), icon);
        }
    }
}

fn save_app_config(locale: Locale, connect_fx: ConnectFxMode) {
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
    map.insert(
        serde_yaml::Value::String("connect_fx".into()),
        serde_yaml::Value::String(connect_fx.code().into()),
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

    use session_tree::{AuthConfig, SessionConfig, TreeNode};

    let web = SessionConfig {
        id: "demo-web-01".into(),
        name: "web-01".into(),
        host: "10.0.1.10".into(),
        port: 22,
        username: "deploy".into(),
        auth: AuthConfig::Password {
            password_ref: None,
        },
        color_tag: Some("#ff5555".into()),
        icon: None,
        term_type: "xterm-256color".into(),
        shell_integration: true,
    };
    let db = SessionConfig {
        id: "demo-db-01".into(),
        name: "db-01".into(),
        host: "10.0.1.20".into(),
        port: 22,
        username: "deploy".into(),
        auth: AuthConfig::Password {
            password_ref: Some("vault://demo-db-01-pwd".into()),
        },
        color_tag: Some("#50fa7b".into()),
        icon: None,
        term_type: "xterm-256color".into(),
        shell_integration: true,
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

/// Scope guard that times one `update()` call and periodically reports the real
/// repaint rate + per-frame UI-thread cost. Enabled only when `VSTERM_DIAG` is set.
struct FrameDiag {
    started: std::time::Instant,
}

impl FrameDiag {
    fn start() -> Self {
        static ENABLED: once_cell::sync::Lazy<bool> =
            once_cell::sync::Lazy::new(|| std::env::var_os("VSTERM_DIAG").is_some());
        let started = std::time::Instant::now();
        if *ENABLED {
            use std::cell::RefCell;
            thread_local! {
                static PREV_START: RefCell<Option<std::time::Instant>> =
                    const { RefCell::new(None) };
            }
            PREV_START.with(|p| {
                if let Some(prev) = *p.borrow() {
                    let gap_ms = prev.elapsed().as_secs_f64() * 1000.0;
                    // While interacting, gaps ≫ update cost mean latency lives
                    // outside App::update (GPU/scheduling/soft faults).
                    if gap_ms >= 80.0 {
                        tracing::warn!("VSTERM_DIAG: FRAME GAP {:.0} ms", gap_ms);
                    }
                    GAP_ACC.with(|g| {
                        let mut max_gap = g.borrow_mut();
                        if gap_ms > *max_gap {
                            *max_gap = gap_ms;
                        }
                    });
                }
                *p.borrow_mut() = Some(started);
            });
        }
        FrameDiag { started }
    }
}

thread_local! {
    static GAP_ACC: std::cell::RefCell<f64> = const { std::cell::RefCell::new(0.0) };
}

impl Drop for FrameDiag {
    fn drop(&mut self) {
        static ENABLED: once_cell::sync::Lazy<bool> =
            once_cell::sync::Lazy::new(|| std::env::var_os("VSTERM_DIAG").is_some());
        if !*ENABLED {
            return;
        }
        use std::cell::RefCell;
        use std::time::Instant;
        thread_local! {
            static ACC: RefCell<(u32, f64, f64, Option<Instant>)> =
                const { RefCell::new((0, 0.0, 0.0, None)) };
        }
        let dt_ms = self.started.elapsed().as_secs_f64() * 1000.0;
        // Immediate line for stalls: the 2s summary alone cannot tell dialog
        // blocks apart from download-end freezes.
        if dt_ms >= 100.0 {
            tracing::warn!("VSTERM_DIAG: SLOW FRAME {:.0} ms", dt_ms);
        }
        ACC.with(|c| {
            let mut a = c.borrow_mut();
            a.0 += 1;
            a.1 += dt_ms;
            if dt_ms > a.2 {
                a.2 = dt_ms;
            }
            let start = a.3.get_or_insert_with(Instant::now);
            let win = start.elapsed().as_secs_f64();
            if win >= 2.0 {
                let fps = a.0 as f64 / win;
                let avg = a.1 / a.0 as f64;
                let gap_max = GAP_ACC.with(|g| {
                    let mut v = g.borrow_mut();
                    let out = *v;
                    *v = 0.0;
                    out
                });
                tracing::warn!(
                    "VSTERM_DIAG: {:.1} fps | frame avg {:.2} ms max {:.2} ms | gap max {:.0} ms ({} frames / {:.1}s)",
                    fps,
                    avg,
                    a.2,
                    gap_max,
                    a.0,
                    win
                );
                *a = (0, 0.0, 0.0, Some(Instant::now()));
            }
        });
    }
}

/// Event-driven painting: wake on input / terminal output; slow timer only for
/// connecting polls and metrics panels. Fully idle windows do not schedule timers.
fn schedule_repaint(
    ctx: &egui::Context,
    connecting: bool,
    wants_metrics: bool,
    fx_active: bool,
    transfer_active: bool,
    menu_open: bool,
) {
    use std::time::Duration;

    // Input events already caused the current frame. Only a held pointer needs
    // follow-up frames; a click, key press, or PointerMoved event must not start
    // a blanket 750 ms redraw loop.
    let pointer_held = ctx.input(|i| i.pointer.any_down());

    // Only schedule a timer when something actually needs periodic updates.
    if fx_active || pointer_held || menu_open {
        ctx.request_repaint_after(crate::render_policy::animation_interval());
    } else if transfer_active {
        // Progress / list polls must keep the event loop alive even when the
        // transfer was started from a blocking native dialog (schedule_repaint
        // already ran before rfd returned).
        ctx.request_repaint_after(crate::render_policy::limit_interval(
            Duration::from_millis(33),
        ));
    } else if connecting {
        ctx.request_repaint_after(Duration::from_millis(200));
    } else if wants_metrics {
        // Charts only — not used as the interaction wake source anymore.
        ctx.request_repaint_after(Duration::from_millis(400));
    }
}

