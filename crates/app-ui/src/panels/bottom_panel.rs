use crate::commands::CommandBook;
use crate::ctx_menu;
use crate::i18n;
use crate::sys_file_icon;
use crate::ui_icon::{self, Icon};
use connection_mgr::{
    join_remote, normalize_remote, parent_remote, ArcProgress, RemoteDirEntry, RemoteSession,
};
use egui::{Color32, CursorIcon, FontId, RichText, Sense, StrokeKind, Ui};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BottomTab {
    #[default]
    Files,
    Commands,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RemotePaneMode {
    Idle,
    SystemUnsupported,
    Ready,
}

struct PendingList {
    path: String,
    /// When true, update the right-hand browse list.
    apply_browse: bool,
    rx: mpsc::Receiver<Result<Vec<RemoteDirEntry>, String>>,
}

struct ActiveTransfer {
    label: String,
    progress: ArcProgress,
    refresh_remote_on_ok: bool,
    open_after: Option<OpenAfter>,
}

#[derive(Debug, Clone, Copy)]
enum OpenAfter {
    DefaultApp,
    Editor,
}

struct InlineRename {
    old_name: String,
    new_name: String,
    request_focus: bool,
}

#[derive(Debug, Clone)]
enum EntryAction {
    Nav(String),
    Select { name: String, toggle: bool },
    Open { name: String, is_dir: bool },
    Edit { name: String },
    Delete { name: String, is_dir: bool },
    Rename { name: String },
    CommitRename,
    CancelRename,
    Download { name: String },
    ToggleTree(String),
}

pub struct BottomPanelState {
    pub tab: BottomTab,
    pub height: f32,
    pub remote_path: String,
    download_dir: PathBuf,
    selected: HashSet<String>,
    remote_entries: Vec<RemoteDirEntry>,
    remote_error: Option<String>,
    remote_loading: bool,
    bound_key: Option<String>,
    pending_list: Option<PendingList>,
    pending_queue: Vec<(String, bool)>,
    dir_cache: HashMap<String, Vec<RemoteDirEntry>>,
    tree_expanded: HashSet<String>,
    tree_width: f32,
    transfer: Option<ActiveTransfer>,
    /// Local paths waiting to upload after the current transfer finishes.
    upload_queue: VecDeque<PathBuf>,
    status_line: Option<String>,
    inline_rename: Option<InlineRename>,
}

impl Default for BottomPanelState {
    fn default() -> Self {
        let download_dir = dirs::download_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            tab: BottomTab::Files,
            height: 180.0,
            remote_path: "/".into(),
            download_dir,
            selected: HashSet::new(),
            remote_entries: Vec::new(),
            remote_error: None,
            remote_loading: false,
            bound_key: None,
            pending_list: None,
            pending_queue: Vec::new(),
            dir_cache: HashMap::new(),
            tree_expanded: HashSet::from(["/".into()]),
            tree_width: 180.0,
            transfer: None,
            upload_queue: VecDeque::new(),
            status_line: None,
            inline_rename: None,
        }
    }
}

const TAB_ROW: f32 = 28.0;
const TAB_SEP: f32 = 6.0;
const FILES_STATUS_BAR_H: f32 = 26.0;
pub const MIN_TERM_HEIGHT: f32 = 120.0;
pub const MIN_BODY_HEIGHT: f32 = 100.0;
pub const SPLIT_GAP: f32 = 6.0;
const TREE_SEP_W: f32 = 5.0;
const MIN_TREE_W: f32 = 120.0;

pub fn reserved_height(state: &BottomPanelState) -> f32 {
    TAB_ROW + TAB_SEP + state.height
}

pub fn clamp_body_height(body: f32, central_h: f32) -> f32 {
    let max_body =
        (central_h - TAB_ROW - TAB_SEP - SPLIT_GAP - MIN_TERM_HEIGHT).max(MIN_BODY_HEIGHT);
    body.clamp(MIN_BODY_HEIGHT, max_body)
}

pub fn show(
    ui: &mut Ui,
    state: &mut BottomPanelState,
    book: &CommandBook,
    remote: Option<&RemoteSession>,
) -> Option<String> {
    let mut send_cmd = None;
    let panel_rect = ui.max_rect();

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
    let content_top = ui.min_rect().max.y;

    match state.tab {
        BottomTab::Files => {
            tick_files(ui.ctx(), state, remote);

            let panel_bottom = ui.clip_rect().max.y;
            let status_bar_rect = egui::Rect::from_min_max(
                egui::pos2(panel_rect.min.x, panel_bottom - FILES_STATUS_BAR_H),
                egui::pos2(panel_rect.max.x, panel_bottom),
            );
            let files_rect = egui::Rect::from_min_max(
                egui::pos2(panel_rect.min.x, content_top),
                egui::pos2(panel_rect.max.x, status_bar_rect.min.y),
            );

            let mut do_cancel = false;
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(files_rect), |ui| {
                egui::Frame::NONE
                    .inner_margin(egui::Margin::symmetric(4, 0))
                    .show(ui, |ui| {
                        show_files_content(ui, state, remote, ui.max_rect().height());
                    });
            });

            handle_os_file_drop(ui, state, remote, files_rect);

            paint_files_status_bar(ui, state, status_bar_rect, &mut do_cancel);

            if do_cancel {
                if let Some(t) = &state.transfer {
                    t.progress.request_cancel();
                }
            }
        }
        BottomTab::Commands => {
            let cmds_rect = egui::Rect::from_min_max(
                egui::pos2(panel_rect.min.x, content_top),
                panel_rect.max,
            );
            let inner_h = cmds_rect.height().max(0.0);
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(cmds_rect), |ui| {
                egui::Frame::NONE
                    .inner_margin(egui::Margin::symmetric(4, 0))
                    .show(ui, |ui| {
                        ui.label(RichText::new(i18n::t("bottom.commands.hint")).weak().small());
                        if book.commands.is_empty() {
                            ui.label(i18n::t("bottom.commands.empty"));
                        } else {
                            egui::ScrollArea::vertical()
                                .id_salt("commands_scroll")
                                .max_height(inner_h - 28.0)
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
                    });
            });
        }
    }

    ui.advance_cursor_after_rect(panel_rect);
    send_cmd
}

fn remote_mode(remote: Option<&RemoteSession>) -> RemotePaneMode {
    match remote {
        None => RemotePaneMode::Idle,
        Some(r) if r.sftp_supported() => RemotePaneMode::Ready,
        Some(_) => RemotePaneMode::SystemUnsupported,
    }
}

fn tick_files(ctx: &egui::Context, state: &mut BottomPanelState, remote: Option<&RemoteSession>) {
    let mode = remote_mode(remote);
    let key = remote.map(|r| r.display_key());

    if state.bound_key.as_deref() != key.as_deref() {
        if let Some(t) = state.transfer.take() {
            t.progress.request_cancel();
        }
        state.upload_queue.clear();
        state.bound_key = key;
        state.remote_entries.clear();
        state.selected.clear();
        state.remote_error = None;
        state.pending_list = None;
        state.pending_queue.clear();
        state.dir_cache.clear();
        state.tree_expanded = HashSet::from(["/".into()]);
        state.remote_loading = false;
        state.status_line = None;
        state.inline_rename = None;
        if mode == RemotePaneMode::Ready {
            state.remote_path = "/".into();
            request_dir(state, remote, "/", true);
        }
    }

    if let Some(pending) = state.pending_list.take() {
        match pending.rx.try_recv() {
            Ok(Ok(entries)) => {
                let path = normalize_remote(&pending.path);
                state.dir_cache.insert(path.clone(), entries.clone());
                if pending.apply_browse
                    && normalize_remote(&state.remote_path) == path
                {
                    state.remote_entries = entries;
                    state.remote_error = None;
                    state.selected.clear();
                    state.remote_loading = false;
                    expand_tree_to(state, &path);
                }
                pump_queue(state, remote);
            }
            Ok(Err(err)) => {
                let path = normalize_remote(&pending.path);
                if pending.apply_browse && normalize_remote(&state.remote_path) == path {
                    state.remote_entries.clear();
                    state.remote_error = Some(err);
                    state.selected.clear();
                    state.remote_loading = false;
                }
                pump_queue(state, remote);
            }
            Err(mpsc::TryRecvError::Empty) => {
                state.pending_list = Some(pending);
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                if pending.apply_browse {
                    state.remote_loading = false;
                    state.remote_error = Some(i18n::t("bottom.files.err.list_gone").into());
                }
                pump_queue(state, remote);
            }
        }
    }

    if let Some(xfer) = state.transfer.as_ref() {
        let snap = xfer.progress.snapshot();
        if snap.done {
            let refresh = xfer.refresh_remote_on_ok && snap.error.is_none();
            let open_after = xfer.open_after;
            let label = xfer.label.clone();
            let open_path = PathBuf::from(&label);
            let msg = match &snap.error {
                Some(e) => format_transfer_error(e),
                None => format!(
                    "{} — {}",
                    i18n::t("bottom.files.transfer_ok"),
                    path_leaf(&label)
                ),
            };
            state.transfer = None;
            state.status_line = Some(msg);
            if snap.error.is_none() {
                if let Some(how) = open_after {
                    let err = match how {
                        OpenAfter::DefaultApp => open_path_default(&open_path),
                        OpenAfter::Editor => open_path_editor(&open_path),
                    };
                    if let Err(e) = err {
                        state.status_line =
                            Some(format!("{}: {e}", i18n::t("bottom.files.err.open")));
                    }
                }
            }
            if refresh && mode == RemotePaneMode::Ready {
                let path = state.remote_path.clone();
                state.dir_cache.remove(&normalize_remote(&path));
                request_dir(state, remote, &path, true);
            }
            pump_upload_queue(state, remote);
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
    }

    if state.remote_loading || state.pending_list.is_some() {
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }
}

fn format_transfer_error(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("permission denied") {
        // Prefer a clear path from SFTP messages like:
        // "sftp create /opt/foo.rar: Permission denied: Permission denied"
        let path = raw
            .split_once("sftp create ")
            .or_else(|| raw.split_once("sftp mkdir "))
            .or_else(|| raw.split_once("sftp open "))
            .map(|(_, rest)| rest.split(':').next().unwrap_or(rest).trim())
            .filter(|p| !p.is_empty());
        match path {
            Some(p) => format!(
                "{}: {} — {}",
                i18n::t("bottom.files.transfer_failed"),
                i18n::t("bottom.files.err.permission"),
                p
            ),
            None => format!(
                "{}: {}",
                i18n::t("bottom.files.transfer_failed"),
                i18n::t("bottom.files.err.permission")
            ),
        }
    } else {
        format!("{}: {raw}", i18n::t("bottom.files.transfer_failed"))
    }
}

fn handle_os_file_drop(
    ui: &mut Ui,
    state: &mut BottomPanelState,
    remote: Option<&RemoteSession>,
    drop_rect: egui::Rect,
) {
    if remote_mode(remote) != RemotePaneMode::Ready {
        return;
    }

    let pointer_in_drop = ui
        .ctx()
        .pointer_interact_pos()
        .or_else(|| ui.ctx().pointer_latest_pos())
        .is_some_and(|p| drop_rect.contains(p));

    let hovering = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
    if hovering && pointer_in_drop {
        let painter = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("vsterm_os_file_drop"),
        ));
        painter.rect_filled(drop_rect, 0.0, Color32::from_rgba_unmultiplied(40, 90, 160, 48));
        painter.rect_stroke(
            drop_rect.shrink(1.0),
            0.0,
            egui::Stroke::new(1.5_f32, Color32::from_rgb(70, 130, 210)),
            StrokeKind::Inside,
        );
        painter.text(
            drop_rect.center(),
            egui::Align2::CENTER_CENTER,
            i18n::t("bottom.files.drop_upload"),
            FontId::proportional(16.0),
            Color32::from_rgb(30, 60, 110),
        );
    }

    let dropped: Vec<PathBuf> = ui.ctx().input(|i| {
        i.raw
            .dropped_files
            .iter()
            .filter_map(|f| f.path.clone())
            .collect()
    });
    if dropped.is_empty() {
        return;
    }

    // Drop events sometimes arrive without a live pointer; fall back to the
    // last known position so a release over the files panel still counts.
    let drop_pos = ui
        .ctx()
        .pointer_interact_pos()
        .or_else(|| ui.ctx().pointer_latest_pos());
    let over_target = drop_pos.is_some_and(|p| drop_rect.contains(p));
    if !over_target {
        return;
    }

    enqueue_uploads(state, remote, dropped);
}

fn enqueue_uploads(
    state: &mut BottomPanelState,
    remote: Option<&RemoteSession>,
    paths: Vec<PathBuf>,
) {
    let mut paths: Vec<PathBuf> = paths
        .into_iter()
        .filter(|p| p.exists())
        .collect();
    if paths.is_empty() {
        state.status_line = Some(i18n::t("bottom.files.err.local_missing").into());
        return;
    }

    if state.transfer.is_none() {
        let first = paths.remove(0);
        state.status_line = Some(format!(
            "{} {}",
            i18n::t("bottom.files.uploading_to"),
            first.display()
        ));
        start_upload(state, remote, first);
    }
    for path in paths {
        state.upload_queue.push_back(path);
    }
    if !state.upload_queue.is_empty() {
        let n = state.upload_queue.len();
        let tip = format!("{} ({n})", i18n::t("bottom.files.upload_queued"));
        if state.status_line.as_deref().unwrap_or("").is_empty() {
            state.status_line = Some(tip);
        }
    }
}

fn pump_upload_queue(state: &mut BottomPanelState, remote: Option<&RemoteSession>) {
    if state.transfer.is_some() {
        return;
    }
    let Some(next) = state.upload_queue.pop_front() else {
        return;
    };
    state.status_line = Some(format!(
        "{} {}",
        i18n::t("bottom.files.uploading_to"),
        next.display()
    ));
    start_upload(state, remote, next);
}

fn path_leaf(s: &str) -> &str {
    Path::new(s)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(s)
}

fn expand_tree_to(state: &mut BottomPanelState, path: &str) {
    let path = normalize_remote(path);
    state.tree_expanded.insert("/".into());
    if path == "/" {
        return;
    }
    let mut acc = String::new();
    for part in path.trim_start_matches('/').split('/') {
        if part.is_empty() {
            continue;
        }
        acc = if acc.is_empty() {
            format!("/{part}")
        } else {
            format!("{acc}/{part}")
        };
        state.tree_expanded.insert(acc.clone());
    }
}

fn pump_queue(state: &mut BottomPanelState, remote: Option<&RemoteSession>) {
    if state.pending_list.is_some() {
        return;
    }
    if let Some((path, apply_browse)) = state.pending_queue.pop() {
        request_dir(state, remote, &path, apply_browse);
    }
}

fn request_dir(
    state: &mut BottomPanelState,
    remote: Option<&RemoteSession>,
    path: &str,
    apply_browse: bool,
) {
    let Some(remote) = remote.filter(|r| r.sftp_supported()) else {
        return;
    };
    let path = normalize_remote(path);
    if apply_browse {
        state.remote_path = path.clone();
        state.remote_loading = true;
        state.remote_error = None;
    }
    if state.pending_list.is_some() {
        // Avoid duplicate queued work.
        if !state
            .pending_queue
            .iter()
            .any(|(p, b)| p == &path && *b == apply_browse)
        {
            state.pending_queue.push((path, apply_browse));
        }
        return;
    }
    let (tx, rx) = mpsc::channel();
    state.pending_list = Some(PendingList {
        path: path.clone(),
        apply_browse,
        rx,
    });
    let session = remote.clone();
    let _ = thread::Builder::new()
        .name("vsterm-sftp-list".into())
        .spawn(move || {
            let res = session.list_dir(&path).map_err(|e| e.to_string());
            let _ = tx.send(res);
        });
}

fn show_files_content(
    ui: &mut Ui,
    state: &mut BottomPanelState,
    remote: Option<&RemoteSession>,
    area_h: f32,
) {
    const PATH_H: f32 = 26.0;

    let mode = remote_mode(remote);
    let mut actions = Vec::new();
    let mut do_refresh = false;
    let mut commit_path = false;
    let mut do_upload_file = false;
    let mut do_upload_folder = false;
    let list_h = (area_h - PATH_H).max(40.0);

    ui.spacing_mut().item_spacing.y = 0.0;
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), PATH_H),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.set_min_height(PATH_H);
            ui.set_max_height(PATH_H);
            ui.horizontal(|ui| {
                let extra = if mode == RemotePaneMode::Ready {
                    130.0
                } else {
                    78.0
                };
                let path_w = (ui.available_width() - extra).max(60.0);
                let path_edit = path_text_edit(ui, &mut state.remote_path, path_w);
                if path_edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    commit_path = true;
                }
                let go = ui
                    .small_button(i18n::t("bottom.files.go"))
                    .on_hover_text(i18n::t("bottom.files.go.tip"));
                if go.clicked() {
                    commit_path = true;
                }
                if go.has_focus() {
                    go.surrender_focus();
                }
                let refresh = ui
                    .add(egui::Button::new(ui_icon::rich(
                        Icon::RefreshCw,
                        13.0,
                        ui_icon::COLOR_MUTED,
                    )))
                    .on_hover_text(i18n::t("bottom.files.refresh"));
                if refresh.clicked() {
                    do_refresh = true;
                }
                if refresh.has_focus() {
                    refresh.surrender_focus();
                }
                if mode == RemotePaneMode::Ready {
                    let upload = ui
                        .add(egui::Button::new(ui_icon::rich(
                            Icon::Upload,
                            13.0,
                            ui_icon::COLOR_MUTED,
                        )))
                        .on_hover_text(i18n::t("bottom.files.ctx.upload"));
                    if upload.clicked() {
                        do_upload_file = true;
                    }
                    if upload.has_focus() {
                        upload.surrender_focus();
                    }
                    let upload_dir = ui
                        .add(egui::Button::new(ui_icon::rich(
                            Icon::FolderPlus,
                            13.0,
                            ui_icon::COLOR_MUTED,
                        )))
                        .on_hover_text(i18n::t("bottom.files.ctx.upload_folder"));
                    if upload_dir.clicked() {
                        do_upload_folder = true;
                    }
                    if upload_dir.has_focus() {
                        upload_dir.surrender_focus();
                    }
                }
            });
        },
    );

    match mode {
        RemotePaneMode::Idle => {
            ui.allocate_ui(egui::vec2(ui.available_width(), list_h), |ui| {
                ui.label(RichText::new(i18n::t("bottom.files.remote.idle")).weak());
            });
        }
        RemotePaneMode::SystemUnsupported => {
            ui.allocate_ui(egui::vec2(ui.available_width(), list_h), |ui| {
                ui.label(
                    RichText::new(i18n::t("bottom.files.remote.system"))
                        .weak()
                        .color(Color32::from_rgb(160, 110, 40)),
                );
            });
        }
        RemotePaneMode::Ready => {
            let avail_w = ui.available_width();
            let max_tree = ((avail_w - TREE_SEP_W) * 0.55).max(MIN_TREE_W);
            state.tree_width = state.tree_width.clamp(MIN_TREE_W, max_tree);
            let tree_w = state.tree_width;
            let list_w = (avail_w - tree_w - TREE_SEP_W).max(80.0);

            ui.horizontal(|ui| {
                ui.set_min_height(list_h);
                ui.set_max_height(list_h);

                ui.allocate_ui_with_layout(
                    egui::vec2(tree_w, list_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_clip_rect(ui.max_rect());
                        let mut tree_acts = show_dir_tree(ui, state, list_h);
                        actions.append(&mut tree_acts);
                    },
                );

                let (sep_rect, sep_resp) =
                    ui.allocate_exact_size(egui::vec2(TREE_SEP_W, list_h), Sense::drag());
                let stroke = if sep_resp.dragged() {
                    ui.style().visuals.widgets.active.fg_stroke
                } else if sep_resp.hovered() {
                    ui.style().visuals.widgets.hovered.fg_stroke
                } else {
                    ui.style().visuals.widgets.noninteractive.bg_stroke
                };
                ui.painter()
                    .vline(sep_rect.center().x, sep_rect.y_range(), stroke);
                if sep_resp.hovered() || sep_resp.dragged() {
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
                }
                if sep_resp.dragged() {
                    state.tree_width = (state.tree_width + sep_resp.drag_delta().x)
                        .clamp(MIN_TREE_W, max_tree);
                }

                ui.allocate_ui_with_layout(
                    egui::vec2(list_w, list_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_clip_rect(ui.max_rect());
                        if state.remote_loading && state.remote_entries.is_empty() {
                            ui.label(RichText::new(i18n::t("bottom.files.loading")).weak());
                        } else if let Some(err) = state.remote_error.clone() {
                            ui.label(
                                RichText::new(err)
                                    .weak()
                                    .color(Color32::from_rgb(180, 70, 70)),
                            );
                        } else {
                            let remote_path = state.remote_path.clone();
                            let remote_entries = state.remote_entries.clone();
                            let mut list_acts = list_remote(
                                ui,
                                state,
                                &remote_path,
                                &remote_entries,
                                list_h,
                                state.transfer.is_none(),
                            );
                            actions.append(&mut list_acts);
                        }
                    },
                );
            });
        }
    }

    for act in actions {
        apply_entry_action(state, remote, act);
    }

    if commit_path || do_refresh {
        let path = normalize_remote(&state.remote_path);
        state.selected.clear();
        if do_refresh {
            state.dir_cache.remove(&path);
        }
        if mode == RemotePaneMode::Ready {
            request_dir(state, remote, &path, true);
        }
    }

    if do_upload_file {
        prompt_upload_file(state, remote);
    }
    if do_upload_folder {
        prompt_upload_folder(state, remote);
    }
}

fn paint_files_status_bar(
    ui: &mut Ui,
    state: &BottomPanelState,
    status_bar_rect: egui::Rect,
    do_cancel: &mut bool,
) {
    let stroke = ui.style().visuals.widgets.noninteractive.bg_stroke;
    ui.painter().hline(
        status_bar_rect.x_range(),
        status_bar_rect.min.y + 0.5,
        stroke,
    );

    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(status_bar_rect), |ui| {
        ui.set_min_height(FILES_STATUS_BAR_H);
        ui.set_max_height(FILES_STATUS_BAR_H);
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            let text_color = ui.visuals().text_color();
            let (sel_n, sel_sz, total_n, total_sz) = selection_stats(state);
            let stats = format!(
                "{}/{}  ·  {}/{}",
                sel_n,
                total_n,
                format_size(sel_sz),
                format_size(total_sz),
            );
            ui.label(RichText::new(stats).size(12.0).color(text_color))
                .on_hover_text(format!(
                    "{} · {}",
                    i18n::t("bottom.files.stat.count_tip"),
                    i18n::t("bottom.files.stat.size_tip"),
                ));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.set_min_width(ui.available_width());
                if let Some(xfer) = &state.transfer {
                    if ui.small_button(i18n::t("bottom.files.cancel")).clicked() {
                        *do_cancel = true;
                    }
                    let snap = xfer.progress.snapshot();
                    let frac = match snap.total {
                        Some(t) if t > 0 => {
                            (snap.transferred as f32 / t as f32).clamp(0.0, 1.0)
                        }
                        _ => 0.0,
                    };
                    ui.add(
                        egui::ProgressBar::new(frac)
                            .desired_width(180.0)
                            .desired_height(20.0)
                            .corner_radius(egui::CornerRadius::ZERO)
                            .text(
                                RichText::new(format!(
                                    "{} {}",
                                    path_leaf(&xfer.label),
                                    format_progress(snap.transferred, snap.total)
                                ))
                                .size(11.0),
                            ),
                    );
                } else if let Some(line) = &state.status_line {
                    ui.label(RichText::new(line).size(12.0).color(text_color));
                }
            });
        });
    });
}

fn selection_stats(state: &BottomPanelState) -> (usize, u64, usize, u64) {
    let total_n = state.remote_entries.len();
    let total_sz: u64 = state
        .remote_entries
        .iter()
        .filter(|e| !e.is_dir)
        .map(|e| e.size.unwrap_or(0))
        .sum();
    let mut sel_n = 0usize;
    let mut sel_sz = 0u64;
    for e in &state.remote_entries {
        if state.selected.contains(&e.name) {
            sel_n += 1;
            if !e.is_dir {
                sel_sz += e.size.unwrap_or(0);
            }
        }
    }
    (sel_n, sel_sz, total_n, total_sz)
}

fn apply_entry_action(
    state: &mut BottomPanelState,
    remote: Option<&RemoteSession>,
    act: EntryAction,
) {
    match act {
        EntryAction::Nav(p) => {
            let p = normalize_remote(&p);
            state.selected.clear();
            expand_tree_to(state, &p);
            if remote_mode(remote) == RemotePaneMode::Ready {
                if let Some(cached) = state.dir_cache.get(&p).cloned() {
                    state.remote_path = p.clone();
                    state.remote_entries = cached;
                    state.remote_error = None;
                    state.remote_loading = false;
                } else {
                    request_dir(state, remote, &p, true);
                }
            }
        }
        EntryAction::ToggleTree(path) => {
            let path = normalize_remote(&path);
            if state.tree_expanded.contains(&path) {
                state.tree_expanded.remove(&path);
            } else {
                state.tree_expanded.insert(path.clone());
                if !state.dir_cache.contains_key(&path)
                    && remote_mode(remote) == RemotePaneMode::Ready
                {
                    request_dir(state, remote, &path, false);
                }
            }
        }
        EntryAction::Select { name, toggle } => {
            if toggle {
                if !state.selected.remove(&name) {
                    state.selected.insert(name);
                }
            } else {
                state.selected.clear();
                state.selected.insert(name);
            }
        }
        EntryAction::Open { name, is_dir } => {
            if is_dir {
                let p = join_remote(&state.remote_path, &name);
                apply_entry_action(state, remote, EntryAction::Nav(p));
            } else {
                let tmp = temp_download_path(&name);
                start_download(state, remote, &name, Some(tmp), Some(OpenAfter::DefaultApp));
            }
        }
        EntryAction::Edit { name } => {
            let tmp = temp_download_path(&name);
            start_download(state, remote, &name, Some(tmp), Some(OpenAfter::Editor));
        }
        EntryAction::Delete { name, is_dir } => {
            let Some(session) = remote.filter(|r| r.sftp_supported()) else {
                state.status_line = Some(i18n::t("bottom.files.err.no_sftp").into());
                return;
            };
            let path = join_remote(&state.remote_path, &name);
            match session.remove(&path, is_dir) {
                Ok(()) => {
                    state.selected.remove(&name);
                    state.status_line = Some(i18n::t("bottom.files.deleted").into());
                    let parent = state.remote_path.clone();
                    state.dir_cache.remove(&normalize_remote(&parent));
                    if is_dir {
                        state.dir_cache.remove(&normalize_remote(&path));
                        state.tree_expanded.remove(&normalize_remote(&path));
                    }
                    request_dir(state, remote, &parent, true);
                }
                Err(e) => {
                    state.status_line =
                        Some(format!("{}: {e}", i18n::t("bottom.files.err.delete")));
                }
            }
        }
        EntryAction::Rename { name } => {
            state.inline_rename = Some(InlineRename {
                old_name: name.clone(),
                new_name: name,
                request_focus: true,
            });
        }
        EntryAction::CommitRename => {
            commit_inline_rename(state, remote);
        }
        EntryAction::CancelRename => {
            state.inline_rename = None;
        }
        EntryAction::Download { name } => {
            prompt_download(state, remote, &name);
        }
    }
}

fn path_text_edit(ui: &mut Ui, path: &mut String, width: f32) -> egui::Response {
    const STROKE: Color32 = Color32::from_rgb(170, 174, 180);
    const STROKE_FOCUS: Color32 = Color32::from_rgb(90, 130, 190);
    let row_h = 22.0;
    let (outer, _) = ui.allocate_exact_size(egui::vec2(width, row_h), Sense::hover());
    ui.painter()
        .rect_filled(outer, 2.0, ui.visuals().extreme_bg_color);
    let inner = outer.shrink(1.0);
    let resp = ui.put(
        inner,
        egui::TextEdit::singleline(path)
            .id_salt("remote_path")
            .frame(false)
            .margin(egui::Margin::symmetric(5, 2))
            .hint_text("/"),
    );
    let stroke = egui::Stroke::new(
        1.0,
        if resp.has_focus() {
            STROKE_FOCUS
        } else {
            STROKE
        },
    );
    ui.painter()
        .rect_stroke(outer, 2.0, stroke, StrokeKind::Outside);
    resp
}

fn commit_inline_rename(state: &mut BottomPanelState, remote: Option<&RemoteSession>) {
    let Some(rename) = state.inline_rename.take() else {
        return;
    };
    let new_name = rename.new_name.trim();
    if new_name.is_empty() || new_name.contains('/') {
        state.status_line = Some(i18n::t("bottom.files.err.rename_invalid").into());
        return;
    }
    if new_name == rename.old_name {
        return;
    }
    let Some(session) = remote.filter(|r| r.sftp_supported()) else {
        state.status_line = Some(i18n::t("bottom.files.err.no_sftp").into());
        return;
    };
    let parent = state.remote_path.clone();
    let from = join_remote(&parent, &rename.old_name);
    let to = join_remote(&parent, new_name);
    match session.rename(&from, &to) {
        Ok(()) => {
            state.selected.remove(&rename.old_name);
            state.selected.insert(new_name.to_string());
            state.status_line = Some(i18n::t("bottom.files.renamed").into());
            state.dir_cache.remove(&normalize_remote(&parent));
            request_dir(state, remote, &parent, true);
        }
        Err(e) => {
            state.status_line = Some(format!("{}: {e}", i18n::t("bottom.files.err.rename")));
        }
    }
}

fn prompt_download(state: &mut BottomPanelState, remote: Option<&RemoteSession>, name: &str) {
    let is_dir = remote_entry(state, name).is_some_and(|e| e.is_dir);
    let Some(path) = pick_download_dest(name, is_dir) else {
        state.status_line = Some(i18n::t("bottom.files.download_cancelled").into());
        return;
    };
    if let Some(parent) = path.parent() {
        state.download_dir = parent.to_path_buf();
    }
    state.status_line = Some(format!(
        "{} {}",
        i18n::t("bottom.files.downloading_to"),
        path.display()
    ));
    start_download(state, remote, name, Some(path), None);
}

fn pick_download_dest(name: &str, is_dir: bool) -> Option<PathBuf> {
    if is_dir {
        rfd::FileDialog::new()
            .set_title(&i18n::t("bottom.files.save_dir_title"))
            .pick_folder()
            .map(|parent| parent.join(name))
    } else {
        pick_save_path(name)
    }
}

fn prompt_upload_file(state: &mut BottomPanelState, remote: Option<&RemoteSession>) {
    let Some(path) = rfd::FileDialog::new()
        .set_title(&i18n::t("bottom.files.ctx.upload"))
        .pick_file()
    else {
        state.status_line = Some(i18n::t("bottom.files.upload_cancelled").into());
        return;
    };
    enqueue_uploads(state, remote, vec![path]);
}

fn prompt_upload_folder(state: &mut BottomPanelState, remote: Option<&RemoteSession>) {
    let Some(path) = rfd::FileDialog::new()
        .set_title(&i18n::t("bottom.files.ctx.upload_folder"))
        .pick_folder()
    else {
        state.status_line = Some(i18n::t("bottom.files.upload_cancelled").into());
        return;
    };
    enqueue_uploads(state, remote, vec![path]);
}

fn pick_save_path(default_name: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(&i18n::t("bottom.files.save_title"))
        .set_file_name(default_name)
        .save_file()
        .map(PathBuf::from)
}

fn temp_download_path(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("vsterm-sftp");
    let _ = std::fs::create_dir_all(&dir);
    let safe: String = name
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    dir.join(safe)
}

fn open_path_default(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &path.to_string_lossy()])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn open_path_editor(path: &Path) -> Result<(), String> {
    if let Ok(editor) = std::env::var("EDITOR") {
        if !editor.is_empty() {
            std::process::Command::new(&editor)
                .arg(path)
                .spawn()
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("notepad")
            .arg(path)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-t"])
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        for bin in ["sensible-editor", "nano", "vim", "vi"] {
            if std::process::Command::new(bin).arg(path).spawn().is_ok() {
                return Ok(());
            }
        }
        open_path_default(path)
    }
}

fn start_download(
    state: &mut BottomPanelState,
    remote: Option<&RemoteSession>,
    name: &str,
    dest: Option<PathBuf>,
    open_after: Option<OpenAfter>,
) {
    let Some(remote) = remote.filter(|r| r.sftp_supported()) else {
        state.status_line = Some(i18n::t("bottom.files.err.no_sftp").into());
        return;
    };
    if state.transfer.is_some() {
        return;
    }
    let remote_path = join_remote(&state.remote_path, name);
    let local = dest.unwrap_or_else(|| state.download_dir.join(name));
    let progress = ArcProgress::new();
    let label = local.to_string_lossy().into_owned();
    state.transfer = Some(ActiveTransfer {
        label,
        progress: progress.clone(),
        refresh_remote_on_ok: false,
        open_after,
    });
    state.status_line = None;
    let session = remote.clone();
    let _ = thread::Builder::new()
        .name("vsterm-sftp-get".into())
        .spawn(move || {
            let _ = session.get_path(&remote_path, &local, Some(&progress));
        });
}

fn start_upload(
    state: &mut BottomPanelState,
    remote: Option<&RemoteSession>,
    local_path: PathBuf,
) {
    let Some(remote) = remote.filter(|r| r.sftp_supported()) else {
        state.status_line = Some(i18n::t("bottom.files.err.no_sftp").into());
        return;
    };
    if state.transfer.is_some() {
        return;
    }
    let Some(name) = local_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(str::to_string)
    else {
        state.status_line = Some(i18n::t("bottom.files.err.local_missing").into());
        return;
    };
    if !local_path.exists() {
        state.status_line = Some(i18n::t("bottom.files.err.local_missing").into());
        return;
    }
    let remote_path = join_remote(&state.remote_path, &name);
    let progress = ArcProgress::new();
    let label = name.clone();
    state.transfer = Some(ActiveTransfer {
        label,
        progress: progress.clone(),
        refresh_remote_on_ok: true,
        open_after: None,
    });
    state.status_line = None;
    let session = remote.clone();
    let _ = thread::Builder::new()
        .name("vsterm-sftp-put".into())
        .spawn(move || {
            let _ = session.put_path(&local_path, &remote_path, Some(&progress));
        });
}

fn remote_entry<'a>(state: &'a BottomPanelState, name: &str) -> Option<&'a RemoteDirEntry> {
    state.remote_entries.iter().find(|e| e.name == name)
}

fn show_dir_tree(ui: &mut Ui, state: &BottomPanelState, max_h: f32) -> Vec<EntryAction> {
    let mut actions = Vec::new();
    let current = normalize_remote(&state.remote_path);
    egui::ScrollArea::vertical()
        .id_salt("sftp_dir_tree")
        .max_height(max_h)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.style_mut().interaction.selectable_labels = false;

            fn walk(
                ui: &mut Ui,
                state: &BottomPanelState,
                path: &str,
                label: &str,
                depth: usize,
                current: &str,
                actions: &mut Vec<EntryAction>,
            ) {
                let path_n = normalize_remote(path);
                let expanded = state.tree_expanded.contains(&path_n);
                let selected = current == path_n;
                let row_h = 20.0;
                let full_w = ui.available_width();
                let id = ui.id().with(("sftp_tree", path_n.as_str()));
                let (_, rect) = ui.allocate_space(egui::vec2(full_w, row_h));
                let resp = ui.interact(rect, id, Sense::click());

                let indent = 4.0 + depth as f32 * 14.0;
                if selected {
                    ui.painter()
                        .rect_filled(rect, 2.0, Color32::from_rgb(220, 228, 240));
                }

                // Chevron hit target
                let chev_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.left() + indent, rect.top()),
                    egui::vec2(14.0, row_h),
                );
                let chev = if expanded {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                };
                let chev_g = ui.fonts(|f| {
                    f.layout_no_wrap(
                        ui_icon::glyph_or_dot(chev),
                        ui_icon::font_id(chev, 11.0),
                        ui_icon::COLOR_MUTED,
                    )
                });
                ui.painter().galley(
                    egui::pos2(
                        chev_rect.center().x - chev_g.size().x * 0.5,
                        rect.center().y - chev_g.size().y * 0.5,
                    ),
                    chev_g,
                    ui_icon::COLOR_MUTED,
                );

                let icon_x = chev_rect.right() + 2.0;
                let icon_rect = egui::Rect::from_min_max(
                    egui::pos2(icon_x, rect.top()),
                    egui::pos2(icon_x + 16.0, rect.bottom()),
                );
                sys_file_icon::paint_entry(ui, label, true, icon_rect, 16.0);
                let name_g = ui.fonts(|f| {
                    f.layout_no_wrap(
                        label.to_owned(),
                        FontId::proportional(13.0),
                        ui_icon::COLOR_MUTED,
                    )
                });
                ui.painter().galley(
                    egui::pos2(
                        icon_x + 16.0,
                        rect.center().y - name_g.size().y * 0.5,
                    ),
                    name_g,
                    ui_icon::COLOR_MUTED,
                );

                if resp.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    let pos = resp.interact_pointer_pos();
                    if pos.is_some_and(|p| chev_rect.contains(p)) {
                        actions.push(EntryAction::ToggleTree(path_n.clone()));
                    } else {
                        actions.push(EntryAction::Nav(path_n.clone()));
                    }
                }
                if resp.double_clicked() {
                    actions.push(EntryAction::Nav(path_n.clone()));
                    if !expanded {
                        actions.push(EntryAction::ToggleTree(path_n.clone()));
                    }
                }

                if !expanded {
                    return;
                }
                let Some(entries) = state.dir_cache.get(&path_n) else {
                    // Still loading — show a faint placeholder once.
                    ui.label(RichText::new("…").small().weak());
                    return;
                };
                let mut dirs: Vec<&RemoteDirEntry> =
                    entries.iter().filter(|e| e.is_dir).collect();
                dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                for d in dirs {
                    let child = join_remote(&path_n, &d.name);
                    walk(ui, state, &child, &d.name, depth + 1, current, actions);
                }
            }

            walk(ui, state, "/", "/", 0, &current, &mut actions);
        });
    actions
}

fn list_remote(
    ui: &mut Ui,
    state: &mut BottomPanelState,
    remote_path: &str,
    entries: &[RemoteDirEntry],
    max_h: f32,
    can_xfer: bool,
) -> Vec<EntryAction> {
    let mut actions = Vec::new();
    let row_h = 20.0;
    let toggle = ui.input(|i| i.modifiers.command || i.modifiers.ctrl);
    let selected = state.selected.clone();
    let editing = state
        .inline_rename
        .as_ref()
        .map(|r| r.old_name.clone());

    egui::ScrollArea::vertical()
        .id_salt(("remote_files", remote_path.to_owned()))
        .max_height(max_h)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.style_mut().interaction.selectable_labels = false;
            let full_w = ui.available_width();

            if normalize_remote(remote_path) != "/" {
                let id = ui.id().with(("rf", remote_path, ".."));
                let (_, rect) = ui.allocate_space(egui::vec2(full_w, row_h));
                let resp = ui.interact(rect, id, Sense::click());
                paint_row(ui, rect, "..", true, None, false);
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                if resp.double_clicked() {
                    actions.push(EntryAction::Nav(parent_remote(remote_path)));
                }
            }

            for entry in entries {
                let is_sel = selected.contains(&entry.name);
                let id = ui.id().with(("rf", remote_path, entry.name.as_str()));
                let (_, rect) = ui.allocate_space(egui::vec2(full_w, row_h));
                let resp = ui.interact(rect, id, Sense::click());

                if editing.as_deref() == Some(entry.name.as_str()) {
                    let just_opened = state
                        .inline_rename
                        .as_ref()
                        .is_some_and(|r| r.request_focus);
                    let rename = state.inline_rename.as_mut().unwrap();
                    if rename.request_focus {
                        ui.memory_mut(|m| m.request_focus(id.with("rename")));
                        rename.request_focus = false;
                    }
                    if is_sel {
                        ui.painter()
                            .rect_filled(rect, 2.0, Color32::from_rgb(220, 228, 240));
                    }
                    let icon_rect = egui::Rect::from_min_max(
                        egui::pos2(rect.left() + 2.0, rect.top()),
                        egui::pos2(rect.left() + 18.0, rect.bottom()),
                    );
                    sys_file_icon::paint_entry(
                        ui,
                        &entry.name,
                        entry.is_dir,
                        icon_rect,
                        16.0,
                    );
                    let edit_rect = egui::Rect::from_min_max(
                        egui::pos2(rect.left() + 20.0, rect.top() + 1.0),
                        egui::pos2(rect.right() - 4.0, rect.bottom() - 1.0),
                    );
                    let edit = ui.put(
                        edit_rect,
                        egui::TextEdit::singleline(&mut rename.new_name)
                            .id_salt("rename")
                            .frame(true)
                            .margin(egui::Margin::symmetric(2, 0)),
                    );
                    if !just_opened && edit.lost_focus() {
                        actions.push(EntryAction::CommitRename);
                    }
                    if edit.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        actions.push(EntryAction::CommitRename);
                    }
                    if edit.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        actions.push(EntryAction::CancelRename);
                    }
                    continue;
                }

                paint_row(
                    ui,
                    rect,
                    &entry.name,
                    entry.is_dir,
                    if entry.is_dir { None } else { entry.size },
                    is_sel,
                );
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    actions.push(EntryAction::Select {
                        name: entry.name.clone(),
                        toggle,
                    });
                }
                if resp.double_clicked() && entry.is_dir {
                    actions.push(EntryAction::Nav(join_remote(remote_path, &entry.name)));
                }

                let mut ctx_act = None;
                resp.context_menu(|ui| {
                    ctx_menu::prepare(ui);
                    if ctx_menu::item(
                        ui,
                        Some(if entry.is_dir {
                            Icon::Folder
                        } else {
                            Icon::File
                        }),
                        &i18n::t("bottom.files.ctx.open"),
                        None,
                        true,
                    )
                    .clicked()
                    {
                        ctx_act = Some(EntryAction::Open {
                            name: entry.name.clone(),
                            is_dir: entry.is_dir,
                        });
                        ui.close_menu();
                    }
                    if ctx_menu::item(
                        ui,
                        Some(Icon::Pencil),
                        &i18n::t("bottom.files.ctx.edit"),
                        None,
                        !entry.is_dir,
                    )
                    .clicked()
                    {
                        ctx_act = Some(EntryAction::Edit {
                            name: entry.name.clone(),
                        });
                        ui.close_menu();
                    }
                    ctx_menu::separator(ui);
                    if ctx_menu::item(
                        ui,
                        Some(Icon::Trash),
                        &i18n::t("bottom.files.ctx.delete"),
                        None,
                        true,
                    )
                    .clicked()
                    {
                        ctx_act = Some(EntryAction::Delete {
                            name: entry.name.clone(),
                            is_dir: entry.is_dir,
                        });
                        ui.close_menu();
                    }
                    if ctx_menu::item(
                        ui,
                        Some(Icon::Pencil),
                        &i18n::t("bottom.files.ctx.rename"),
                        None,
                        true,
                    )
                    .clicked()
                    {
                        ctx_act = Some(EntryAction::Rename {
                            name: entry.name.clone(),
                        });
                        ui.close_menu();
                    }
                    ctx_menu::separator(ui);
                    if ctx_menu::item(
                        ui,
                        None,
                        &i18n::t("bottom.files.ctx.download"),
                        None,
                        can_xfer,
                    )
                    .clicked()
                    {
                        ctx_act = Some(EntryAction::Download {
                            name: entry.name.clone(),
                        });
                        ui.close_menu();
                    }
                });
                if let Some(a) = ctx_act {
                    actions.push(EntryAction::Select {
                        name: entry.name.clone(),
                        toggle: false,
                    });
                    actions.push(a);
                }
            }
        });
    actions
}

fn paint_row(
    ui: &mut Ui,
    rect: egui::Rect,
    name: &str,
    is_dir: bool,
    size: Option<u64>,
    selected: bool,
) {
    if selected {
        ui.painter()
            .rect_filled(rect, 2.0, Color32::from_rgb(220, 228, 240));
    }
    let icon_w = 16.0;
    let icon_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 2.0, rect.top()),
        egui::pos2(rect.left() + 2.0 + icon_w, rect.bottom()),
    );
    sys_file_icon::paint_entry(ui, name, is_dir, icon_rect, icon_w);

    let x = rect.left() + 4.0 + icon_w + 4.0;
    let y = rect.center().y;

    let name_galley = ui.fonts(|f| {
        f.layout_no_wrap(
            name.to_owned(),
            FontId::proportional(13.0),
            ui_icon::COLOR_MUTED,
        )
    });
    let name_pos = egui::pos2(x, y - name_galley.size().y * 0.5);
    ui.painter()
        .galley(name_pos, name_galley, ui_icon::COLOR_MUTED);

    if let Some(sz) = size {
        let text = format_size(sz);
        let g = ui.fonts(|f| {
            f.layout_no_wrap(
                text,
                FontId::monospace(11.0),
                Color32::from_rgb(140, 145, 155),
            )
        });
        let pos = egui::pos2(rect.right() - g.size().x - 6.0, y - g.size().y * 0.5);
        ui.painter()
            .galley(pos, g, Color32::from_rgb(140, 145, 155));
    }
}

fn format_size(n: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let f = n as f64;
    if f >= GB {
        format!("{:.1}G", f / GB)
    } else if f >= MB {
        format!("{:.1}M", f / MB)
    } else if f >= KB {
        format!("{:.0}K", f / KB)
    } else {
        format!("{n}B")
    }
}

fn format_progress(done: u64, total: Option<u64>) -> String {
    match total {
        Some(t) => format!("{}/{}", format_size(done), format_size(t)),
        None => format_size(done),
    }
}
