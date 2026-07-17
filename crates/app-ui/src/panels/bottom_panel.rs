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
use std::time::Instant;

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
    is_upload: bool,
    started: Instant,
    last_bytes: u64,
    last_sample: Instant,
    /// Smoothed bytes/sec.
    speed_bps: f64,
}

#[derive(Debug, Clone)]
struct TransferLogItem {
    name: String,
    is_upload: bool,
    ok: bool,
    detail: String,
}

const TRANSFER_LOG_MAX: usize = 40;
const PROGRESS_BAR_W: f32 = 300.0;

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
struct RemoteDragPayload {
    names: Vec<String>,
}

#[derive(Debug, Clone)]
enum QueuedTransfer {
    Upload(PathBuf),
    Download { name: String, dest: PathBuf },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectMode {
    /// Plain click: replace selection.
    Replace,
    /// Ctrl/Cmd click: toggle one item.
    Toggle,
    /// Shift click: range from anchor.
    Range,
}

#[derive(Debug, Clone)]
enum EntryAction {
    Nav(String),
    Select {
        name: String,
        mode: SelectMode,
    },
    ClearSelection,
    Open {
        name: String,
        is_dir: bool,
    },
    Edit {
        name: String,
    },
    Delete {
        name: String,
        is_dir: bool,
    },
    Rename {
        name: String,
    },
    CommitRename,
    CancelRename,
    Download {
        names: Vec<String>,
    },
    NewFolder,
    /// Create a UTF-8 file with the given extension and initial body (may be empty).
    NewFile {
        ext: &'static str,
        initial: &'static str,
    },
    UploadFile,
    UploadFolder,
    ToggleTree(String),
}

/// Native OS file dialogs open only after egui has painted one frame with the
/// context menu already closed. Flow: click → `close_menu` → queue here → next
/// frame paints without the menu → end-of-frame opens `rfd` (see `App::update`).
struct PendingNativeDialog {
    /// False on the click frame; true after one subsequent UI pass.
    menu_closed_painted: bool,
    kind: NativeDialogKind,
}

pub enum NativeDialogKind {
    Download { names: Vec<String> },
    UploadFile,
    UploadFolder,
}

pub struct BottomPanelState {
    pub tab: BottomTab,
    pub height: f32,
    pub remote_path: String,
    download_dir: PathBuf,
    selected: HashSet<String>,
    /// Anchor name for Shift+click range selection in the browse list.
    selection_anchor: Option<String>,
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
    /// Uploads / downloads waiting after the current transfer finishes.
    transfer_queue: VecDeque<QueuedTransfer>,
    /// Recent finished transfers (newest at the front).
    transfer_log: VecDeque<TransferLogItem>,
    transfer_list_open: bool,
    status_line: Option<String>,
    inline_rename: Option<InlineRename>,
    /// Set by a menu/button click; drained at the start of the next `tick_files`
    /// so the context menu can close and repaint before the blocking OS dialog.
    pending_native_dialog: Option<PendingNativeDialog>,
    /// Per-server file-browser views, keyed by `user@host`. Switching server
    /// tabs stashes the outgoing view here and restores the target's, so the
    /// panel keeps each host's directory/listing instead of resetting to `/`
    /// and re-listing every time (which was both jarring and leaked memory).
    saved: HashMap<String, ServerFiles>,
}

/// Snapshot of the per-server file-browser state (everything except the shared
/// UI chrome like panel height and column widths).
struct ServerFiles {
    remote_path: String,
    selected: HashSet<String>,
    selection_anchor: Option<String>,
    remote_entries: Vec<RemoteDirEntry>,
    remote_error: Option<String>,
    remote_loading: bool,
    pending_list: Option<PendingList>,
    pending_queue: Vec<(String, bool)>,
    dir_cache: HashMap<String, Vec<RemoteDirEntry>>,
    tree_expanded: HashSet<String>,
    transfer: Option<ActiveTransfer>,
    transfer_queue: VecDeque<QueuedTransfer>,
    transfer_log: VecDeque<TransferLogItem>,
    transfer_list_open: bool,
    status_line: Option<String>,
    inline_rename: Option<InlineRename>,
}

impl ServerFiles {
    /// A clean view for a server that is being visited for the first time.
    fn fresh() -> Self {
        Self {
            remote_path: "/".into(),
            selected: HashSet::new(),
            selection_anchor: None,
            remote_entries: Vec::new(),
            remote_error: None,
            remote_loading: false,
            pending_list: None,
            pending_queue: Vec::new(),
            dir_cache: HashMap::new(),
            tree_expanded: HashSet::from(["/".into()]),
            transfer: None,
            transfer_queue: VecDeque::new(),
            transfer_log: VecDeque::new(),
            transfer_list_open: false,
            status_line: None,
            inline_rename: None,
        }
    }

    /// Move the live per-server fields out of `state` for stashing.
    fn capture(state: &mut BottomPanelState) -> Self {
        Self {
            remote_path: std::mem::take(&mut state.remote_path),
            selected: std::mem::take(&mut state.selected),
            selection_anchor: state.selection_anchor.take(),
            remote_entries: std::mem::take(&mut state.remote_entries),
            remote_error: state.remote_error.take(),
            remote_loading: state.remote_loading,
            pending_list: state.pending_list.take(),
            pending_queue: std::mem::take(&mut state.pending_queue),
            dir_cache: std::mem::take(&mut state.dir_cache),
            tree_expanded: std::mem::take(&mut state.tree_expanded),
            transfer: state.transfer.take(),
            transfer_queue: std::mem::take(&mut state.transfer_queue),
            transfer_log: std::mem::take(&mut state.transfer_log),
            transfer_list_open: state.transfer_list_open,
            status_line: state.status_line.take(),
            inline_rename: state.inline_rename.take(),
        }
    }

    /// Install this view as the live one.
    fn restore(self, state: &mut BottomPanelState) {
        state.remote_path = self.remote_path;
        state.selected = self.selected;
        state.selection_anchor = self.selection_anchor;
        state.remote_entries = self.remote_entries;
        state.remote_error = self.remote_error;
        state.remote_loading = self.remote_loading;
        state.pending_list = self.pending_list;
        state.pending_queue = self.pending_queue;
        state.dir_cache = self.dir_cache;
        state.tree_expanded = self.tree_expanded;
        state.transfer = self.transfer;
        state.transfer_queue = self.transfer_queue;
        state.transfer_log = self.transfer_log;
        state.transfer_list_open = self.transfer_list_open;
        state.status_line = self.status_line;
        state.inline_rename = self.inline_rename;
    }
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
            selection_anchor: None,
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
            transfer_queue: VecDeque::new(),
            transfer_log: VecDeque::new(),
            transfer_list_open: false,
            status_line: None,
            inline_rename: None,
            pending_native_dialog: None,
            saved: HashMap::new(),
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
    active_cwd: Option<(String, u64)>,
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
                        if let Some(cmd) = show_files_content(
                            ui,
                            state,
                            remote,
                            active_cwd.as_ref(),
                            ui.max_rect().height(),
                        ) {
                            send_cmd = Some(cmd);
                        }
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
            let cmds_rect =
                egui::Rect::from_min_max(egui::pos2(panel_rect.min.x, content_top), panel_rect.max);
            let inner_h = cmds_rect.height().max(0.0);
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(cmds_rect), |ui| {
                egui::Frame::NONE
                    .inner_margin(egui::Margin::symmetric(4, 0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(i18n::t("bottom.commands.hint"))
                                .weak()
                                .small(),
                        );
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

    if state.bound_key != key {
        // Stash the outgoing server's view so returning to its tab restores the
        // exact directory + listing. In-flight transfers are NOT cancelled —
        // they belong to that host and keep running in the background.
        if let Some(old) = state.bound_key.take() {
            let view = ServerFiles::capture(state);
            state.saved.insert(old, view);
        }
        state.bound_key = key.clone();
        match key {
            Some(k) => {
                if let Some(view) = state.saved.remove(&k) {
                    // Seen before: restore its view without touching the network.
                    view.restore(state);
                } else {
                    // First visit to this host: start clean and list the root once.
                    ServerFiles::fresh().restore(state);
                    if mode == RemotePaneMode::Ready {
                        request_dir(state, remote, "/", true);
                    }
                }
            }
            None => ServerFiles::fresh().restore(state),
        }
    }

    if let Some(pending) = state.pending_list.take() {
        match pending.rx.try_recv() {
            Ok(Ok(entries)) => {
                let path = normalize_remote(&pending.path);
                state.dir_cache.insert(path.clone(), entries.clone());
                if pending.apply_browse && normalize_remote(&state.remote_path) == path {
                    state.remote_entries = entries;
                    state.remote_error = None;
                    if state.inline_rename.is_none() {
                        state.selected.clear();
                    } else if let Some(r) = &state.inline_rename {
                        state.selected.clear();
                        state.selected.insert(r.old_name.clone());
                        state.selection_anchor = Some(r.old_name.clone());
                    }
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
                ctx.request_repaint_after(crate::render_policy::limit_interval(
                    std::time::Duration::from_millis(50),
                ));
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

    if let Some(xfer) = state.transfer.as_mut() {
        let snap = xfer.progress.snapshot();
        if !snap.done {
            update_transfer_speed(xfer, snap.transferred);
            ctx.request_repaint_after(crate::render_policy::limit_interval(
                std::time::Duration::from_millis(33),
            ));
        }
    }

    if let Some(xfer) = state.transfer.as_ref() {
        let snap = xfer.progress.snapshot();
        if snap.done {
            let refresh = xfer.refresh_remote_on_ok && snap.error.is_none();
            let open_after = xfer.open_after;
            let label = xfer.label.clone();
            let is_upload = xfer.is_upload;
            let open_path = PathBuf::from(&label);
            let leaf = path_leaf(&label).to_string();
            let (msg, log_ok, log_detail) = match &snap.error {
                Some(e) => {
                    let err = format_transfer_error(e);
                    (err.clone(), false, err)
                }
                None => {
                    let detail = format_progress(snap.transferred, snap.total);
                    (
                        format!("{} — {leaf}", i18n::t("bottom.files.transfer_ok")),
                        true,
                        detail,
                    )
                }
            };
            push_transfer_log(
                state,
                TransferLogItem {
                    name: leaf.clone(),
                    is_upload,
                    ok: log_ok,
                    detail: log_detail,
                },
            );
            state.transfer = None;
            state.status_line = Some(msg);
            if std::env::var_os("VSTERM_DIAG").is_some() {
                tracing::warn!(
                    "VSTERM_DIAG: transfer finished ok={log_ok} upload={is_upload} label={leaf}"
                );
            }
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
            pump_transfer_queue(state, remote);
        }
    }

    if state.remote_loading || state.pending_list.is_some() {
        ctx.request_repaint_after(crate::render_policy::limit_interval(
            std::time::Duration::from_millis(50),
        ));
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
    let dropped: Vec<PathBuf> = ui.ctx().input(|i| {
        i.raw
            .dropped_files
            .iter()
            .filter_map(|f| f.path.clone())
            .collect()
    });
    let has_os_drop = !dropped.is_empty();
    let hovering_os = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
    let hovering_remote = egui::DragAndDrop::has_payload_of_type::<RemoteDragPayload>(ui.ctx());

    if remote_mode(remote) != RemotePaneMode::Ready {
        if has_os_drop {
            state.status_line = Some(i18n::t("bottom.files.err.no_sftp").into());
        }
        return;
    }

    if hovering_os || hovering_remote {
        let painter = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("vsterm_os_file_drop"),
        ));
        let fill = if hovering_remote {
            Color32::from_rgba_unmultiplied(40, 140, 80, 48)
        } else {
            Color32::from_rgba_unmultiplied(40, 90, 160, 48)
        };
        let stroke = if hovering_remote {
            Color32::from_rgb(50, 140, 80)
        } else {
            Color32::from_rgb(70, 130, 210)
        };
        let tip = if hovering_remote {
            i18n::t("bottom.files.drop_download")
        } else {
            i18n::t("bottom.files.drop_upload")
        };
        painter.rect_filled(drop_rect, 0.0, fill);
        painter.rect_stroke(
            drop_rect.shrink(1.0),
            0.0,
            egui::Stroke::new(1.5_f32, stroke),
            StrokeKind::Inside,
        );
        painter.text(
            drop_rect.center(),
            egui::Align2::CENTER_CENTER,
            tip,
            FontId::proportional(16.0),
            Color32::from_rgb(30, 60, 110),
        );
    }

    // Accept OS drops whenever the Files tab is active — pointer position at the
    // drop frame is often already invalid on Windows, so do not gate on it.
    if has_os_drop {
        enqueue_uploads(state, remote, dropped);
    }

    // Remote → local: release an in-app drag over the files panel to pick a
    // local folder and download there (Explorer OLE drag-out is not available).
    if ui.input(|i| i.pointer.any_released()) {
        let over = ui
            .ctx()
            .pointer_interact_pos()
            .or_else(|| ui.ctx().pointer_latest_pos())
            .is_some_and(|p| drop_rect.contains(p));
        if over {
            if let Some(payload) = egui::DragAndDrop::take_payload::<RemoteDragPayload>(ui.ctx()) {
                prompt_download_selection(state, remote, payload.names.clone());
            }
        }
    }
}

fn enqueue_uploads(
    state: &mut BottomPanelState,
    remote: Option<&RemoteSession>,
    paths: Vec<PathBuf>,
) {
    let paths: Vec<PathBuf> = paths.into_iter().filter(|p| p.exists()).collect();
    if paths.is_empty() {
        state.status_line = Some(i18n::t("bottom.files.err.local_missing").into());
        return;
    }
    let n = paths.len();
    for path in paths {
        state.transfer_queue.push_back(QueuedTransfer::Upload(path));
    }
    state.status_line = Some(format!("{} ({n})", i18n::t("bottom.files.upload_queued")));
    pump_transfer_queue(state, remote);
}

fn enqueue_downloads(
    state: &mut BottomPanelState,
    remote: Option<&RemoteSession>,
    items: Vec<(String, PathBuf)>,
) {
    if items.is_empty() {
        return;
    }
    let n = items.len();
    for (name, dest) in items {
        state
            .transfer_queue
            .push_back(QueuedTransfer::Download { name, dest });
    }
    state.status_line = Some(format!("{} ({n})", i18n::t("bottom.files.download_queued")));
    pump_transfer_queue(state, remote);
}

fn pump_transfer_queue(state: &mut BottomPanelState, remote: Option<&RemoteSession>) {
    if state.transfer.is_some() {
        return;
    }
    let Some(next) = state.transfer_queue.pop_front() else {
        return;
    };
    match next {
        QueuedTransfer::Upload(path) => {
            state.status_line = Some(format!(
                "{} {}",
                i18n::t("bottom.files.uploading_to"),
                path.display()
            ));
            start_upload(state, remote, path);
        }
        QueuedTransfer::Download { name, dest } => {
            state.status_line = Some(format!(
                "{} {}",
                i18n::t("bottom.files.downloading_to"),
                dest.display()
            ));
            start_download(state, remote, &name, Some(dest), None);
        }
    }
}

fn prompt_download_selection(
    state: &mut BottomPanelState,
    remote: Option<&RemoteSession>,
    names: Vec<String>,
) {
    if names.is_empty() {
        return;
    }
    let Some(folder) = rfd::FileDialog::new()
        .set_title(&i18n::t("bottom.files.save_dir_title"))
        .pick_folder()
    else {
        state.status_line = Some(i18n::t("bottom.files.download_cancelled").into());
        return;
    };
    state.download_dir = folder.clone();
    let mut items = Vec::new();
    for name in names {
        items.push((name.clone(), folder.join(&name)));
    }
    enqueue_downloads(state, remote, items);
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
    active_cwd: Option<&(String, u64)>,
    area_h: f32,
) -> Option<String> {
    const PATH_H: f32 = 26.0;

    let mode = remote_mode(remote);
    let mut actions = Vec::new();
    let mut do_refresh = false;
    let mut commit_path = false;
    let mut send_cmd = None;
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
                    let to_term = ui
                        .add(egui::Button::new(ui_icon::rich(
                            Icon::Upload,
                            13.0,
                            ui_icon::COLOR_MUTED,
                        )))
                        .on_hover_text(i18n::t("bottom.files.sync_to_term"));
                    if to_term.clicked() {
                        let path = normalize_remote(&state.remote_path);
                        send_cmd = Some(format!("cd {}\n", shell_single_quote(&path)));
                    }
                    if to_term.has_focus() {
                        to_term.surrender_focus();
                    }
                    let from_term = ui
                        .add(egui::Button::new(ui_icon::rich(
                            Icon::Download,
                            13.0,
                            ui_icon::COLOR_MUTED,
                        )))
                        .on_hover_text(i18n::t("bottom.files.sync_from_term"));
                    if from_term.clicked() {
                        if let Some((path, _)) = active_cwd {
                            // Force a fresh listing when syncing from the terminal.
                            let path = normalize_remote(path);
                            state.dir_cache.remove(&path);
                            actions.push(EntryAction::Nav(path));
                        } else {
                            // Cwd arrives via OSC 7 from the session-scoped shell
                            // bootstrap (Bash/Zsh/Fish). Never inject probe commands
                            // into the interactive PTY — that would echo into the
                            // user's shell.
                            state.status_line =
                                Some(i18n::t("bottom.files.sync_from_term.failed").into());
                        }
                    }
                    if from_term.has_focus() {
                        from_term.surrender_focus();
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
                    state.tree_width =
                        (state.tree_width + sep_resp.drag_delta().x).clamp(MIN_TREE_W, max_tree);
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
    // Dismiss menus and wake so deferred rfd runs after a clean paint.
    if state.pending_native_dialog.is_some() {
        ui.ctx().memory_mut(|m| m.close_popup());
        ui.ctx().request_repaint();
    }

    if commit_path || do_refresh {
        let path = normalize_remote(&state.remote_path);
        state.selected.clear();
        state.selection_anchor = None;
        if do_refresh {
            state.dir_cache.remove(&path);
        }
        if mode == RemotePaneMode::Ready {
            request_dir(state, remote, &path, true);
        }
    }

    send_cmd
}

fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn paint_files_status_bar(
    ui: &mut Ui,
    state: &mut BottomPanelState,
    status_bar_rect: egui::Rect,
    do_cancel: &mut bool,
) {
    let stroke = ui.style().visuals.widgets.noninteractive.bg_stroke;
    ui.painter().hline(
        status_bar_rect.x_range(),
        status_bar_rect.min.y + 0.5,
        stroke,
    );

    let mut open_clicked = false;
    let mut transfer_anchor = None;

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
                        Some(t) if t > 0 => (snap.transferred as f32 / t as f32).clamp(0.0, 1.0),
                        _ => 0.0,
                    };
                    let bar = paint_transfer_progress_bar(
                        ui,
                        frac,
                        path_leaf(&xfer.label),
                        &format_progress(snap.transferred, snap.total),
                        &format_speed(xfer.speed_bps),
                    );
                    transfer_anchor = Some(bar.rect);
                    if bar.clicked() {
                        open_clicked = true;
                    }
                    bar.on_hover_text(i18n::t("bottom.files.transfer_list.tip"));
                } else if let Some(line) = &state.status_line {
                    let can_open = transfer_list_has_rows(state);
                    let label = ui.add(
                        egui::Label::new(RichText::new(line.clone()).size(12.0).color(text_color))
                            .sense(if can_open {
                                Sense::click()
                            } else {
                                Sense::hover()
                            }),
                    );
                    if can_open {
                        transfer_anchor = Some(label.rect);
                        if label.clicked() {
                            open_clicked = true;
                        }
                        label.on_hover_text(i18n::t("bottom.files.transfer_list.tip"));
                    }
                } else if transfer_list_has_rows(state) {
                    let label = ui
                        .add(
                            egui::Label::new(
                                RichText::new(i18n::t("bottom.files.transfer_list"))
                                    .size(12.0)
                                    .color(text_color),
                            )
                            .sense(Sense::click()),
                        )
                        .on_hover_text(i18n::t("bottom.files.transfer_list.tip"));
                    transfer_anchor = Some(label.rect);
                    if label.clicked() {
                        open_clicked = true;
                    }
                }
            });
        });
    });

    if open_clicked {
        state.transfer_list_open = !state.transfer_list_open;
    }

    if state.transfer_list_open {
        let anchor = transfer_anchor.unwrap_or(status_bar_rect);
        paint_transfer_list_popup(ui, state, anchor, status_bar_rect, open_clicked);
    }
}

fn transfer_list_has_rows(state: &BottomPanelState) -> bool {
    state.transfer.is_some() || !state.transfer_queue.is_empty() || !state.transfer_log.is_empty()
}

/// Custom progress strip: name left, speed center, size right — and clickable.
fn paint_transfer_progress_bar(
    ui: &mut Ui,
    frac: f32,
    name: &str,
    size_text: &str,
    speed_text: &str,
) -> egui::Response {
    const BAR_H: f32 = 20.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(PROGRESS_BAR_W, BAR_H), Sense::click());

    let bg = ui.visuals().extreme_bg_color;
    let fill = Color32::from_rgb(160, 198, 235);
    let fg = ui.visuals().text_color();
    ui.painter().rect_filled(rect, 0.0, bg);
    let fill_w = (rect.width() * frac.clamp(0.0, 1.0)).round();
    if fill_w > 0.0 {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(rect.min, egui::vec2(fill_w, rect.height())),
            0.0,
            fill,
        );
    }

    let font = FontId::proportional(11.0);
    let pad = 6.0;
    let y = rect.center().y;

    let name_g = ui.fonts(|f| f.layout_no_wrap(name.to_owned(), font.clone(), fg));
    let speed_g = ui.fonts(|f| f.layout_no_wrap(speed_text.to_owned(), font.clone(), fg));
    let size_g = ui.fonts(|f| f.layout_no_wrap(size_text.to_owned(), font, fg));

    // Right: size
    let size_x = rect.right() - pad - size_g.size().x;
    // Center: speed
    let speed_x = rect.center().x - speed_g.size().x * 0.5;
    // Left: name (clip so it does not overlap speed)
    let name_max_right = (speed_x - 8.0).max(rect.left() + pad);
    let name_x = rect.left() + pad;
    let name_clip = egui::Rect::from_min_max(
        egui::pos2(name_x, rect.top()),
        egui::pos2(name_max_right, rect.bottom()),
    );

    ui.painter().with_clip_rect(name_clip).galley(
        egui::pos2(name_x, y - name_g.size().y * 0.5),
        name_g,
        fg,
    );
    ui.painter()
        .galley(egui::pos2(speed_x, y - speed_g.size().y * 0.5), speed_g, fg);
    ui.painter()
        .galley(egui::pos2(size_x, y - size_g.size().y * 0.5), size_g, fg);

    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp
}

fn paint_transfer_list_popup(
    ui: &mut Ui,
    state: &mut BottomPanelState,
    anchor: egui::Rect,
    status_bar: egui::Rect,
    opened_this_frame: bool,
) {
    const PANEL_W: f32 = 360.0;
    const PANEL_H: f32 = 240.0;
    // Bottom-right: panel bottom edge sits on the status bar top, right-aligned.
    let pos = egui::pos2(status_bar.right() - PANEL_W, status_bar.top() - PANEL_H);

    let mut close = false;
    let resp = egui::Area::new(egui::Id::new("files_transfer_list"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .constrain_to(ui.ctx().screen_rect())
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style())
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.set_min_size(egui::vec2(PANEL_W - 16.0, PANEL_H - 16.0));
                    ui.set_max_size(egui::vec2(PANEL_W - 16.0, PANEL_H - 16.0));

                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(i18n::t("bottom.files.transfer_list"))
                                .size(13.0)
                                .strong(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("×").clicked() {
                                close = true;
                            }
                        });
                    });
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    let list_h = ui.available_height().max(40.0);
                    egui::ScrollArea::vertical()
                        .id_salt("transfer_list_scroll")
                        .max_height(list_h)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_min_width(PANEL_W - 24.0);
                            let mut any = false;

                            if let Some(xfer) = &state.transfer {
                                any = true;
                                let snap = xfer.progress.snapshot();
                                transfer_list_row(
                                    ui,
                                    xfer.is_upload,
                                    path_leaf(&xfer.label),
                                    &format!(
                                        "{}  {}",
                                        format_progress(snap.transferred, snap.total),
                                        format_speed(xfer.speed_bps),
                                    ),
                                    TransferRowKind::Running,
                                );
                            }

                            for q in &state.transfer_queue {
                                any = true;
                                let (name, up) = match q {
                                    QueuedTransfer::Upload(p) => (
                                        p.file_name()
                                            .and_then(|n| n.to_str())
                                            .unwrap_or("?")
                                            .to_string(),
                                        true,
                                    ),
                                    QueuedTransfer::Download { name, .. } => (name.clone(), false),
                                };
                                transfer_list_row(
                                    ui,
                                    up,
                                    &name,
                                    &i18n::t("bottom.files.transfer.queued"),
                                    TransferRowKind::Queued,
                                );
                            }

                            for item in &state.transfer_log {
                                any = true;
                                transfer_list_row(
                                    ui,
                                    item.is_upload,
                                    &item.name,
                                    &item.detail,
                                    if item.ok {
                                        TransferRowKind::Ok
                                    } else {
                                        TransferRowKind::Failed
                                    },
                                );
                            }

                            if !any {
                                ui.label(
                                    RichText::new(i18n::t("bottom.files.transfer_list.empty"))
                                        .weak(),
                                );
                            }
                        });
                });
        });

    if close {
        state.transfer_list_open = false;
    }

    // Skip outside-close on the frame that opened the panel (the opening click
    // would otherwise race). Progress-bar clicks are on the status bar anchor.
    if !opened_this_frame && ui.input(|i| i.pointer.any_click()) {
        if let Some(pos) = ui.ctx().pointer_interact_pos() {
            let on_popup = resp.response.rect.contains(pos);
            let on_anchor = anchor.contains(pos) || status_bar.contains(pos);
            if !on_popup && !on_anchor {
                state.transfer_list_open = false;
            }
        }
    }
}

#[derive(Clone, Copy)]
enum TransferRowKind {
    Running,
    Queued,
    Ok,
    Failed,
}

fn transfer_list_row(
    ui: &mut Ui,
    is_upload: bool,
    name: &str,
    detail: &str,
    kind: TransferRowKind,
) {
    let arrow = if is_upload { "↑" } else { "↓" };
    let status = match kind {
        TransferRowKind::Running => i18n::t("bottom.files.transfer.running"),
        TransferRowKind::Queued => i18n::t("bottom.files.transfer.queued"),
        TransferRowKind::Ok => i18n::t("bottom.files.transfer_ok"),
        TransferRowKind::Failed => i18n::t("bottom.files.transfer_failed"),
    };
    let status_color = match kind {
        TransferRowKind::Running => Color32::from_rgb(40, 120, 200),
        TransferRowKind::Queued => Color32::from_rgb(120, 120, 128),
        TransferRowKind::Ok => Color32::from_rgb(40, 140, 80),
        TransferRowKind::Failed => Color32::from_rgb(190, 70, 70),
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new(arrow).size(12.0).color(ui_icon::COLOR_MUTED));
        ui.label(RichText::new(name).size(12.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(status).size(11.0).color(status_color));
            ui.label(RichText::new(detail).size(11.0).weak());
        });
    });
}

fn push_transfer_log(state: &mut BottomPanelState, item: TransferLogItem) {
    state.transfer_log.push_front(item);
    while state.transfer_log.len() > TRANSFER_LOG_MAX {
        state.transfer_log.pop_back();
    }
}

fn update_transfer_speed(xfer: &mut ActiveTransfer, transferred: u64) {
    let now = Instant::now();
    let dt = now.duration_since(xfer.last_sample).as_secs_f64();
    if dt < 0.15 {
        return;
    }
    let delta = transferred.saturating_sub(xfer.last_bytes) as f64;
    let instant = delta / dt;
    xfer.speed_bps = if xfer.speed_bps <= 0.0 {
        instant
    } else {
        xfer.speed_bps * 0.65 + instant * 0.35
    };
    // Fallback to average if samples are noisy at start.
    if xfer.speed_bps < 1.0 {
        let elapsed = now.duration_since(xfer.started).as_secs_f64().max(0.001);
        xfer.speed_bps = transferred as f64 / elapsed;
    }
    xfer.last_bytes = transferred;
    xfer.last_sample = now;
}

fn format_speed(bps: f64) -> String {
    if bps < 1.0 {
        return "—/s".into();
    }
    format!("{}/s", format_size(bps as u64))
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
            state.selection_anchor = None;
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
        EntryAction::Select { name, mode } => {
            apply_selection(state, &name, mode);
        }
        EntryAction::ClearSelection => {
            state.selected.clear();
            state.selection_anchor = None;
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
        EntryAction::Download { names } => {
            state.pending_native_dialog = Some(PendingNativeDialog {
                menu_closed_painted: false,
                kind: NativeDialogKind::Download { names },
            });
        }
        EntryAction::NewFolder => {
            create_remote_folder(state, remote);
        }
        EntryAction::NewFile { ext, initial } => {
            create_remote_file(state, remote, ext, initial);
        }
        EntryAction::UploadFile => {
            state.pending_native_dialog = Some(PendingNativeDialog {
                menu_closed_painted: false,
                kind: NativeDialogKind::UploadFile,
            });
        }
        EntryAction::UploadFolder => {
            state.pending_native_dialog = Some(PendingNativeDialog {
                menu_closed_painted: false,
                kind: NativeDialogKind::UploadFolder,
            });
        }
    }
}

/// After the UI pass: arm a queued dialog once the menu has painted closed, or
/// return it ready to open (blocking `rfd`). Called from `App::update` end.
pub fn take_native_dialog_after_paint(state: &mut BottomPanelState) -> Option<NativeDialogKind> {
    let Some(pending) = state.pending_native_dialog.take() else {
        return None;
    };
    if !pending.menu_closed_painted {
        state.pending_native_dialog = Some(PendingNativeDialog {
            menu_closed_painted: true,
            kind: pending.kind,
        });
        None
    } else {
        Some(pending.kind)
    }
}

/// Drop a closed host's stashed file-browser state so `dir_cache` does not
/// accumulate for tabs that will never be reopened.
pub fn forget_saved_host(state: &mut BottomPanelState, host_key: &str) {
    state.saved.remove(host_key);
    if state.bound_key.as_deref() == Some(host_key) {
        state.bound_key = None;
    }
}

pub fn has_pending_native_dialog(state: &BottomPanelState) -> bool {
    state.pending_native_dialog.is_some()
}

/// True when the files panel needs a short poll cadence (active/queued transfer
/// or in-flight directory list).
pub fn needs_transfer_poll(state: &BottomPanelState) -> bool {
    state.transfer.is_some()
        || state.pending_list.is_some()
        || !state.transfer_queue.is_empty()
        || state.remote_loading
}

pub fn run_native_dialog(
    state: &mut BottomPanelState,
    remote: Option<&RemoteSession>,
    kind: NativeDialogKind,
) {
    match kind {
        NativeDialogKind::Download { names } => {
            if names.len() == 1 {
                prompt_download(state, remote, &names[0]);
            } else {
                prompt_download_selection(state, remote, names);
            }
        }
        NativeDialogKind::UploadFile => prompt_upload_file(state, remote),
        NativeDialogKind::UploadFolder => prompt_upload_folder(state, remote),
    }
}

fn apply_selection(state: &mut BottomPanelState, name: &str, mode: SelectMode) {
    match mode {
        SelectMode::Replace => {
            state.selected.clear();
            state.selected.insert(name.to_string());
            state.selection_anchor = Some(name.to_string());
        }
        SelectMode::Toggle => {
            if !state.selected.remove(name) {
                state.selected.insert(name.to_string());
            }
            state.selection_anchor = Some(name.to_string());
        }
        SelectMode::Range => {
            let anchor = state
                .selection_anchor
                .clone()
                .unwrap_or_else(|| name.to_string());
            let names: Vec<&str> = state
                .remote_entries
                .iter()
                .map(|e| e.name.as_str())
                .collect();
            let Some(i0) = names.iter().position(|n| *n == anchor.as_str()) else {
                state.selected.clear();
                state.selected.insert(name.to_string());
                state.selection_anchor = Some(name.to_string());
                return;
            };
            let Some(i1) = names.iter().position(|n| *n == name) else {
                return;
            };
            let (lo, hi) = if i0 <= i1 { (i0, i1) } else { (i1, i0) };
            state.selected.clear();
            for n in &names[lo..=hi] {
                state.selected.insert((*n).to_string());
            }
            // Keep original anchor so repeated Shift+clicks extend from the same start.
            if state.selection_anchor.is_none() {
                state.selection_anchor = Some(anchor);
            }
        }
    }
}

fn unique_entry_name(entries: &[RemoteDirEntry], preferred: &str) -> String {
    if !entries.iter().any(|e| e.name == preferred) {
        return preferred.to_string();
    }
    // untitled.txt → untitled (2).txt ; 新建文件夹 → 新建文件夹 (2)
    let (stem, ext) = match preferred.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() && !preferred.starts_with('.') => {
            (stem, Some(ext))
        }
        _ => (preferred, None),
    };
    for i in 2..10_000 {
        let candidate = match ext {
            Some(ext) => format!("{stem} ({i}).{ext}"),
            None => format!("{stem} ({i})"),
        };
        if !entries.iter().any(|e| e.name == candidate) {
            return candidate;
        }
    }
    format!("{preferred}.{}", uuid_fallback())
}

fn uuid_fallback() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| (d.as_nanos() as u32) % 100_000)
        .unwrap_or(0)
}

fn create_remote_folder(state: &mut BottomPanelState, remote: Option<&RemoteSession>) {
    let Some(session) = remote.filter(|r| r.sftp_supported()) else {
        state.status_line = Some(i18n::t("bottom.files.err.no_sftp").into());
        return;
    };
    let name = unique_entry_name(
        &state.remote_entries,
        &i18n::t("bottom.files.new_folder_name"),
    );
    let path = join_remote(&state.remote_path, &name);
    match session.mkdir(&path) {
        Ok(()) => {
            state.status_line = Some(i18n::t("bottom.files.created").into());
            state.selected.clear();
            state.selected.insert(name.clone());
            state.selection_anchor = Some(name.clone());
            state.inline_rename = Some(InlineRename {
                old_name: name.clone(),
                new_name: name,
                request_focus: true,
            });
            let parent = state.remote_path.clone();
            state.dir_cache.remove(&normalize_remote(&parent));
            request_dir(state, remote, &parent, true);
        }
        Err(e) => {
            state.status_line = Some(format!("{}: {e}", i18n::t("bottom.files.err.create")));
        }
    }
}

fn create_remote_file(
    state: &mut BottomPanelState,
    remote: Option<&RemoteSession>,
    ext: &str,
    initial: &str,
) {
    let Some(session) = remote.filter(|r| r.sftp_supported()) else {
        state.status_line = Some(i18n::t("bottom.files.err.no_sftp").into());
        return;
    };
    let ext = ext.trim_start_matches('.');
    let preferred = format!("{}.{}", i18n::t("bottom.files.new_file_stem"), ext);
    let name = unique_entry_name(&state.remote_entries, &preferred);
    let path = join_remote(&state.remote_path, &name);
    // Body is UTF-8 (empty or shebang + newline).
    match session.write_file(&path, initial.as_bytes()) {
        Ok(()) => {
            state.status_line = Some(i18n::t("bottom.files.created").into());
            state.selected.clear();
            state.selected.insert(name.clone());
            state.selection_anchor = Some(name.clone());
            state.inline_rename = Some(InlineRename {
                old_name: name.clone(),
                new_name: name,
                request_focus: true,
            });
            let parent = state.remote_path.clone();
            state.dir_cache.remove(&normalize_remote(&parent));
            request_dir(state, remote, &parent, true);
        }
        Err(e) => {
            state.status_line = Some(format!("{}: {e}", i18n::t("bottom.files.err.create")));
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
    enqueue_downloads(state, remote, vec![(name.to_string(), path)]);
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
    let now = Instant::now();
    state.transfer = Some(ActiveTransfer {
        label,
        progress: progress.clone(),
        refresh_remote_on_ok: false,
        open_after,
        is_upload: false,
        started: now,
        last_bytes: 0,
        last_sample: now,
        speed_bps: 0.0,
    });
    state.status_line = None;
    let session = remote.clone();
    let _ = thread::Builder::new()
        .name("vsterm-sftp-get".into())
        .spawn(move || {
            let _ = session.get_path(&remote_path, &local, Some(&progress));
        });
}

fn start_upload(state: &mut BottomPanelState, remote: Option<&RemoteSession>, local_path: PathBuf) {
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
    let now = Instant::now();
    state.transfer = Some(ActiveTransfer {
        label,
        progress: progress.clone(),
        refresh_remote_on_ok: true,
        open_after: None,
        is_upload: true,
        started: now,
        last_bytes: 0,
        last_sample: now,
        speed_bps: 0.0,
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
                    egui::pos2(icon_x + 16.0, rect.center().y - name_g.size().y * 0.5),
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
                let mut dirs: Vec<&RemoteDirEntry> = entries.iter().filter(|e| e.is_dir).collect();
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
    let mods = ui.input(|i| i.modifiers);
    let select_mode = if mods.shift {
        SelectMode::Range
    } else if mods.command || mods.ctrl {
        SelectMode::Toggle
    } else {
        SelectMode::Replace
    };
    let selected = state.selected.clone();
    let editing = state.inline_rename.as_ref().map(|r| r.old_name.clone());

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
                paint_parent_row(ui, rect);
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                // Single click is enough — this row is only a navigation affordance.
                if resp.clicked() || resp.double_clicked() {
                    actions.push(EntryAction::Nav(parent_remote(remote_path)));
                }
            }

            for entry in entries {
                let is_sel = selected.contains(&entry.name);
                let id = ui.id().with(("rf", remote_path, entry.name.as_str()));
                let (_, rect) = ui.allocate_space(egui::vec2(full_w, row_h));
                let resp = ui.interact(rect, id, Sense::click_and_drag());

                if editing.as_deref() == Some(entry.name.as_str()) {
                    let just_opened = state
                        .inline_rename
                        .as_ref()
                        .is_some_and(|r| r.request_focus);
                    let rename = state.inline_rename.as_mut().unwrap();
                    let rename_id = id.with("rename");
                    if rename.request_focus {
                        ui.memory_mut(|m| m.request_focus(rename_id));
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
                    sys_file_icon::paint_entry(ui, &entry.name, entry.is_dir, icon_rect, 16.0);
                    let edit_w = rename_field_width(
                        ui,
                        &rename.new_name,
                        rect.left() + 20.0,
                        rect.right() - 4.0,
                    );
                    let edit_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.left() + 20.0, rect.top() + 1.0),
                        egui::vec2(edit_w, (rect.height() - 2.0).max(16.0)),
                    );
                    let mut edit_output = None;
                    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(edit_rect), |ui| {
                        ui.set_min_size(edit_rect.size());
                        let out = egui::TextEdit::singleline(&mut rename.new_name)
                            .id(rename_id)
                            .desired_width(edit_w)
                            .frame(true)
                            .margin(egui::Margin::symmetric(2, 0))
                            .show(ui);
                        edit_output = Some(out);
                    });
                    let edit = edit_output.expect("rename TextEdit output");
                    if just_opened {
                        let end = rename_stem_char_len(&rename.new_name, entry.is_dir);
                        let mut state_te = edit.state;
                        state_te
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::two(
                                egui::text::CCursor::new(0),
                                egui::text::CCursor::new(end),
                            )));
                        state_te.store(ui.ctx(), edit.response.id);
                        ui.ctx().request_repaint();
                    }
                    if !just_opened && edit.response.lost_focus() {
                        actions.push(EntryAction::CommitRename);
                    }
                    if edit.response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        actions.push(EntryAction::CommitRename);
                    }
                    if edit.response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
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
                if resp.drag_started() && can_xfer {
                    let names = if selected.contains(&entry.name) && !selected.is_empty() {
                        selected.iter().cloned().collect::<Vec<_>>()
                    } else {
                        vec![entry.name.clone()]
                    };
                    egui::DragAndDrop::set_payload(ui.ctx(), RemoteDragPayload { names });
                    ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
                }
                if resp.clicked() {
                    actions.push(EntryAction::Select {
                        name: entry.name.clone(),
                        mode: select_mode,
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
                        let names = if selected.contains(&entry.name) && !selected.is_empty() {
                            entries
                                .iter()
                                .filter(|e| selected.contains(&e.name))
                                .map(|e| e.name.clone())
                                .collect::<Vec<_>>()
                        } else {
                            vec![entry.name.clone()]
                        };
                        ctx_act = Some(EntryAction::Download { names });
                        ui.close_menu();
                    }
                });
                if let Some(a) = ctx_act {
                    // Keep multi-selection when downloading several items.
                    let keep_selection =
                        matches!(&a, EntryAction::Download { names } if names.len() > 1);
                    if !keep_selection {
                        actions.push(EntryAction::Select {
                            name: entry.name.clone(),
                            mode: SelectMode::Replace,
                        });
                    }
                    actions.push(a);
                }
            }

            // Empty area below entries: clear selection on click; create via context menu.
            let empty_h = ui.available_height().max(48.0);
            let (_, empty_resp) =
                ui.allocate_exact_size(egui::vec2(full_w, empty_h), Sense::click());
            if empty_resp.clicked() {
                actions.push(EntryAction::ClearSelection);
            }
            let mut empty_act = None;
            empty_resp.context_menu(|ui| {
                ctx_menu::prepare(ui);
                if ctx_menu::item(
                    ui,
                    Some(Icon::FolderPlus),
                    &i18n::t("bottom.files.ctx.new_folder"),
                    None,
                    can_xfer,
                )
                .clicked()
                {
                    empty_act = Some(EntryAction::NewFolder);
                    ui.close_menu();
                }
                ctx_menu::submenu(
                    ui,
                    Some(Icon::File),
                    &i18n::t("bottom.files.ctx.new_file"),
                    |ui| {
                        for (label, ext, initial) in [
                            (".sh(sh)", "sh", "#!/bin/sh\n"),
                            (".sh(bash)", "sh", "#!/bin/bash\n"),
                            (".json", "json", ""),
                            (".config", "config", ""),
                            (".yaml", "yaml", ""),
                            (".txt", "txt", ""),
                        ] {
                            if ctx_menu::item(ui, None, label, None, can_xfer).clicked() {
                                empty_act = Some(EntryAction::NewFile { ext, initial });
                                ui.close_menu();
                            }
                        }
                    },
                );
                ctx_menu::separator(ui);
                if ctx_menu::item(
                    ui,
                    Some(Icon::Upload),
                    &i18n::t("bottom.files.ctx.upload"),
                    None,
                    can_xfer,
                )
                .clicked()
                {
                    empty_act = Some(EntryAction::UploadFile);
                    ui.close_menu();
                }
                if ctx_menu::item(
                    ui,
                    Some(Icon::FolderPlus),
                    &i18n::t("bottom.files.ctx.upload_folder"),
                    None,
                    can_xfer,
                )
                .clicked()
                {
                    empty_act = Some(EntryAction::UploadFolder);
                    ui.close_menu();
                }
            });
            if let Some(a) = empty_act {
                actions.push(a);
            }
        });
    actions
}

/// Width of the inline rename field: hug the filename, not the whole row.
fn rename_field_width(ui: &Ui, name: &str, left: f32, right: f32) -> f32 {
    let text_w = ui
        .fonts(|f| f.layout_no_wrap(name.to_owned(), FontId::proportional(13.0), Color32::WHITE))
        .size()
        .x;
    let max_w = (right - left).max(40.0);
    (text_w + 14.0).clamp(40.0, max_w)
}

/// Char length to select on rename focus: stem only for files with an extension.
fn rename_stem_char_len(name: &str, is_dir: bool) -> usize {
    if is_dir {
        return name.chars().count();
    }
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() && !name.starts_with('.') => {
            stem.chars().count()
        }
        _ => name.chars().count(),
    }
}

fn paint_parent_row(ui: &mut Ui, rect: egui::Rect) {
    let icon_w = 16.0;
    let icon_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 2.0, rect.top()),
        egui::pos2(rect.left() + 2.0 + icon_w, rect.bottom()),
    );
    sys_file_icon::paint_ui_icon(ui, Icon::FolderUp, icon_rect, icon_w);

    let label = i18n::t("bottom.files.parent");
    let x = rect.left() + 4.0 + icon_w + 4.0;
    let y = rect.center().y;
    let name_galley =
        ui.fonts(|f| f.layout_no_wrap(label, FontId::proportional(13.0), ui_icon::COLOR_MUTED));
    let name_pos = egui::pos2(x, y - name_galley.size().y * 0.5);
    ui.painter()
        .galley(name_pos, name_galley, ui_icon::COLOR_MUTED);
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
