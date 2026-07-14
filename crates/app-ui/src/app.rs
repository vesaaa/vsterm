use crate::commands::CommandBook;
use crate::conn_error::{format_conn_error, ConnErrorDisplay};
use crate::i18n::{self, Locale};
use crate::metrics::{HostSnapshot, MetricsService};
use crate::remote_host::RemoteHostService;
use crate::panels::bottom_panel::{self, BottomPanelState};
use crate::panels::host_toolbar::{self, MainTab};
use crate::panels::{connection_list, monitor, routes, session_tree_panel, status_bar, toolbar};
use crate::terminal_view::TerminalView;
use crate::{fonts, theme};
use connection_mgr::{ConnectFailure, ConnectionManager, ConnError, ConnErrorKey, RemoteSession};
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
    rx: mpsc::Receiver<Result<(), ConnectFailure>>,
}

struct PasswordPromptState {
    config: SessionConfig,
    password: String,
    show_empty_warn: bool,
    attempt: u32,
    focus_password: bool,
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
    password_prompt: Option<PasswordPromptState>,
    host_bind_gen: u64,
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
            password_prompt: None,
            host_bind_gen: 0,
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
        if self.pending_connect.is_some() {
            self.status = i18n::t("status.connecting");
            return;
        }

        let resolved = connection_mgr::resolve_backend(config.backend);
        if resolved != BackendKind::System {
            self.error_dialog = Some(format_conn_error(
                &connection_mgr::backend_unavailable_error(resolved),
            ));
            self.status = i18n::t("status.open_failed");
            return;
        }
        if !connection_mgr::SystemSshBackend::is_available() {
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
        if let Err(err) = connection_mgr::preflight(&config, vault.as_ref(), false) {
            self.error_dialog = Some(format_conn_error(&err));
            self.status = i18n::t("status.open_failed");
            return;
        }

        if config.needs_password_prompt() {
            self.password_prompt = Some(PasswordPromptState {
                config,
                password: String::new(),
                show_empty_warn: false,
                attempt: 1,
                focus_password: true,
            });
            return;
        }

        self.start_ssh_session(config, None, 1);
    }

    fn start_ssh_session(
        &mut self,
        config: SessionConfig,
        interactive_password: Option<String>,
        attempt: u32,
    ) {
        if self.pending_connect.is_some() {
            self.status = i18n::t("status.connecting");
            return;
        }

        let resolved = connection_mgr::resolve_backend(config.backend);
        if resolved != BackendKind::System {
            self.error_dialog = Some(format_conn_error(
                &connection_mgr::backend_unavailable_error(resolved),
            ));
            self.status = i18n::t("status.open_failed");
            return;
        }
        if !connection_mgr::SystemSshBackend::is_available() {
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
        if let Err(err) =
            connection_mgr::preflight(&config, vault.as_ref(), interactive_password.is_some())
        {
            self.error_dialog = Some(format_conn_error(&err));
            self.status = i18n::t("status.open_failed");
            return;
        }

        // Do not add a host tab / switch panels until authentication succeeds.
        self.status = format!("{} — {}", i18n::t("status.connecting"), config.display_label());

        let mgr = Arc::clone(&self.connections);
        let vault_path = self.store.as_ref().map(|s| s.paths().vault_path());
        let (tx, rx) = mpsc::channel();
        let remote = RemoteSession {
            config: config.clone(),
            interactive_password: interactive_password.clone(),
        };

        let config_for_thread = config.clone();
        let config_for_pending = config.clone();
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
                    Ok(io) => {
                        mgr.insert_ssh_connected(&config_for_thread, remote, io);
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
            rx,
        });
    }

    fn sync_host_binding(&mut self) {
        let vault_path = self.store.as_ref().map(|s| s.paths().vault_path());
        if let Some(remote) = self.connections.active_remote() {
            self.remote_host.bind(Some(remote), vault_path);
            if self.selected_nic.is_none() {
                self.selected_nic = self
                    .remote_host
                    .selected_nic()
                    .or_else(|| {
                        self.remote_host
                            .snapshot()
                            .as_ref()
                            .map(|s| HostSnapshot::prefer_primary_nic(&s.nics))
                            .flatten()
                    });
            }
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
            }
            Ok(Err(failure)) => {
                self.sync_host_binding();
                if failure.key == ConnErrorKey::AuthFailed && pending.attempt < MAX_PASSWORD_ATTEMPTS
                {
                    self.password_prompt = Some(PasswordPromptState {
                        config: pending.config,
                        password: String::new(),
                        show_empty_warn: false,
                        attempt: pending.attempt + 1,
                        focus_password: true,
                    });
                    self.status = i18n::t("dialog.password.wrong");
                } else {
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
                self.status = i18n::t("status.open_failed");
            }
        }
    }

    fn show_password_dialog(&mut self, ctx: &egui::Context) {
        let mut connect = false;
        let mut cancel = false;

        if let Some(prompt) = &mut self.password_prompt {
            let mut open = true;
            let attempt = prompt.attempt;
            egui::Window::new(format!(
                "{} ({}/{})",
                i18n::t("dialog.password.title"),
                attempt,
                MAX_PASSWORD_ATTEMPTS
            ))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(360.0)
            .show(ctx, |ui| {
                ui.label(format!(
                    "{}: {}",
                    i18n::t("dialog.password.host"),
                    prompt.config.display_label()
                ));
                if attempt > 1 {
                    ui.colored_label(
                        egui::Color32::from_rgb(220, 80, 80),
                        i18n::t("dialog.password.wrong"),
                    );
                    ui.add_space(4.0);
                }
                ui.add_space(6.0);
                ui.label(i18n::t("dialog.password.hint"));
                ui.add_space(8.0);
                ui.label(i18n::t("dialog.password.field"));
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut prompt.password)
                        .password(true)
                        .desired_width(f32::INFINITY),
                );
                if prompt.focus_password {
                    resp.request_focus();
                    prompt.focus_password = false;
                }
                if prompt.show_empty_warn {
                    ui.colored_label(
                        egui::Color32::from_rgb(220, 80, 80),
                        i18n::t("dialog.password.empty"),
                    );
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button(i18n::t("dialog.password.connect")).clicked() {
                        if prompt.password.is_empty() {
                            prompt.show_empty_warn = true;
                        } else {
                            connect = true;
                        }
                    }
                    if ui.button(i18n::t("dialog.password.cancel")).clicked() {
                        cancel = true;
                    }
                });
                // egui: Enter typically causes lost_focus; also accept Enter while focused.
                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                if enter && (resp.has_focus() || resp.lost_focus()) {
                    if prompt.password.is_empty() {
                        prompt.show_empty_warn = true;
                    } else {
                        connect = true;
                    }
                }
            });
            if !open {
                cancel = true;
            }
        }

        if connect {
            let prompt = self.password_prompt.take().unwrap();
            self.start_ssh_session(prompt.config, Some(prompt.password), prompt.attempt);
        } else if cancel {
            self.password_prompt = None;
            self.status = i18n::t("status.open_failed");
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
}

impl eframe::App for VsTermApp {
    fn on_exit(&mut self) {
        self.remote_host.bind(None, None);
        self.remote_host.stop();
        self.metrics.stop();
        self.connections.close_all();
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(i18n::t("app.name")));

        self.poll_pending_connect();
        self.connections.reap_dead();
        let gen = self.connections.generation();
        if self.host_bind_gen != gen {
            self.host_bind_gen = gen;
            self.sync_host_binding();
        }
        self.show_password_dialog(ctx);
        self.show_error_dialog(ctx);

        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button(i18n::t("menu.file"), |ui| {
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
                                            let action =
                                                session_tree_panel::show(ui, &self.tree);
                                            if let Some(action) = action {
                                                match action {
                                                    session_tree_panel::TreeAction::OpenLocalDemo => {
                                                        self.open_local_shell("Local Shell");
                                                    }
                                                    session_tree_panel::TreeAction::OpenSession {
                                                        name: _,
                                                        session_ref,
                                                    } => {
                                                        if let Some(store) = &self.store {
                                                            match store.load_session(&session_ref)
                                                            {
                                                                Ok(cfg) => {
                                                                    self.request_open_session(cfg);
                                                                }
                                                                Err(err) => {
                                                                    self.error_dialog = Some(
                                                                        format_conn_error(
                                                                            &ConnError::NotFound(
                                                                                err.to_string(),
                                                                            ),
                                                                        ),
                                                                    );
                                                                    self.status = i18n::t(
                                                                        "status.open_failed",
                                                                    );
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
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
                                    let (conn_action, tab_rect) =
                                        connection_list::show(ui, &self.connections);
                                    if let Some(rect) = tab_rect {
                                        active_tab_screen_rect = Some(rect);
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
                    let bottom_h = self.bottom.height + 36.0;
                    let total = ui.available_height();
                    let term_h = (total - bottom_h - 4.0).max(80.0);

                    egui::Frame::NONE.show(ui, |ui| {
                        ui.set_min_height(term_h);
                        ui.set_max_height(term_h);
                        let (cols, rows) = TerminalView::show(ui, &self.connections);
                        if (cols, rows) != self.last_term_size && cols > 0 && rows > 0 {
                            if let Err(err) = self.connections.resize_active(cols, rows) {
                                tracing::debug!("resize: {err}");
                            } else {
                                self.last_term_size = (cols, rows);
                            }
                        }
                    });

                    ui.add_space(4.0);
                    ui.separator();
                    if let Some(cmd) = bottom_panel::show(ui, &mut self.bottom, &self.commands) {
                        if let Err(err) = self.connections.write_to_active(cmd.as_bytes()) {
                            self.status = format!("{}: {err}", i18n::t("status.open_failed"));
                        }
                    }
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
