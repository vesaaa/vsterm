use crate::i18n;
use crate::sys_file_icon::{self, FileIconKind};
use crate::ui_icon::{self, Icon};
use egui::{Color32, RichText, Sense, Ui};
use session_tree::{SessionTree, TreeNode};

#[derive(Debug, Clone)]
pub enum TreeSelection {
    Session { name: String, session_ref: String },
    Folder { id: String, name: String },
}

pub enum TreeAction {
    OpenSession { name: String, session_ref: String },
    OpenLocalDemo,
    AddServer { folder_id: Option<String> },
    EditServer { session_ref: String },
    DeleteServer { session_ref: String, name: String },
    /// `parent_id = None` → top-level; `Some` → under a first-level folder.
    AddFolder { parent_id: Option<String> },
    RenameFolder { id: String, name: String },
    DeleteFolder { id: String, name: String },
    /// Move an existing session into `folder_id` (`None` = root).
    MoveSession {
        session_ref: String,
        name: String,
        folder_id: Option<String>,
    },
}

#[derive(Clone)]
struct SessionDragPayload {
    session_ref: String,
    name: String,
}

pub fn show(
    ui: &mut Ui,
    tree: &SessionTree,
    selection: &mut Option<TreeSelection>,
) -> Option<TreeAction> {
    let mut action = None;

    ui.horizontal(|ui| {
        if ui
            .button(egui::RichText::new(i18n::t("tree.add_server")).size(12.0))
            .on_hover_text(i18n::t("tree.add_server_hint"))
            .clicked()
        {
            let folder_id = match selection {
                Some(TreeSelection::Folder { id, .. }) => Some(id.clone()),
                Some(TreeSelection::Session { session_ref, .. }) => {
                    tree.folder_of_session(session_ref)
                }
                None => None,
            };
            action = Some(TreeAction::AddServer { folder_id });
        }
        if ui
            .button(egui::RichText::new(i18n::t("tree.add_folder")).size(12.0))
            .on_hover_text(i18n::t("tree.add_folder_hint"))
            .clicked()
        {
            let parent_id = match selection {
                Some(TreeSelection::Folder { id, .. }) if tree.can_nest_under(id) => {
                    Some(id.clone())
                }
                _ => None,
            };
            action = Some(TreeAction::AddFolder { parent_id });
        }
    });
    ui.add_space(4.0);

    if ui
        .add(
            egui::Button::new(i18n::t("tree.local_shell"))
                .min_size([ui.available_width(), 28.0].into()),
        )
        .clicked()
    {
        action = Some(TreeAction::OpenLocalDemo);
    }

    ui.add_space(6.0);

    let dragging =
        egui::DragAndDrop::has_payload_of_type::<SessionDragPayload>(ui.ctx());
    if dragging {
        if let Some(a) = root_drop_zone(ui) {
            action = Some(a);
        }
        ui.add_space(4.0);
    }

    egui::ScrollArea::vertical()
        .id_salt("session_tree_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for node in &tree.root {
                if let Some(a) = render_node(ui, tree, node, selection) {
                    action = Some(a);
                }
            }
            if tree.root.is_empty() {
                ui.weak(i18n::t("tree.empty"));
            }
        });

    action
}

fn root_drop_zone(ui: &mut Ui) -> Option<TreeAction> {
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(1.0), 26.0),
        Sense::hover(),
    );
    let hovered = resp.dnd_hover_payload::<SessionDragPayload>().is_some();
    let fill = if hovered {
        Color32::from_rgba_unmultiplied(60, 120, 210, 40)
    } else {
        Color32::from_rgba_unmultiplied(100, 105, 115, 28)
    };
    ui.painter().rect_filled(rect, 3.0, fill);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        i18n::t("tree.drop_root"),
        egui::FontId::proportional(12.0),
        Color32::from_rgb(70, 74, 82),
    );
    resp.dnd_release_payload::<SessionDragPayload>()
        .map(|p| TreeAction::MoveSession {
            session_ref: p.session_ref.clone(),
            name: p.name.clone(),
            folder_id: None,
        })
}

fn render_node(
    ui: &mut Ui,
    tree: &SessionTree,
    node: &TreeNode,
    selection: &mut Option<TreeSelection>,
) -> Option<TreeAction> {
    match node {
        TreeNode::Folder {
            name,
            id,
            children,
        } => {
            let selected = matches!(
                selection,
                Some(TreeSelection::Folder { id: sid, .. }) if sid == id
            );
            let color = if selected {
                ui_icon::COLOR_ACCENT
            } else {
                Color32::from_rgb(32, 34, 40)
            };
            let header = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                ui.id().with(("folder", id.as_str())),
                true,
            )
            .show_header(ui, |ui| {
                sys_file_icon::add_icon(ui, FileIconKind::Folder, 14.0, Icon::Folder);
                ui.label(RichText::new(name).size(13.0).color(color));
            });
            let (_, header_inner, body_opt) = header.body(|ui| {
                let mut action = None;
                for child in children {
                    if let Some(a) = render_node(ui, tree, child, selection) {
                        action = Some(a);
                    }
                }
                action
            });
            let header = &header_inner.response;
            if header.dnd_hover_payload::<SessionDragPayload>().is_some() {
                ui.painter().rect_filled(
                    header.rect,
                    2.0,
                    Color32::from_rgba_unmultiplied(60, 120, 210, 36),
                );
            }
            let mut action = header
                .dnd_release_payload::<SessionDragPayload>()
                .map(|p| TreeAction::MoveSession {
                    session_ref: p.session_ref.clone(),
                    name: p.name.clone(),
                    folder_id: Some(id.clone()),
                });

            // Click folder header row to select.
            if header.clicked() {
                *selection = Some(TreeSelection::Folder {
                    id: id.clone(),
                    name: name.clone(),
                });
            }
            header.context_menu(|ui| {
                crate::ctx_menu::prepare(ui);
                if crate::ctx_menu::item(
                    ui,
                    Some(Icon::Server),
                    &i18n::t("tree.ctx.add_server"),
                    None,
                    true,
                )
                .clicked()
                {
                    action = Some(TreeAction::AddServer {
                        folder_id: Some(id.clone()),
                    });
                    ui.close_menu();
                }
                if tree.can_nest_under(id)
                    && crate::ctx_menu::item(
                        ui,
                        Some(Icon::FolderPlus),
                        &i18n::t("tree.ctx.add_subfolder"),
                        None,
                        true,
                    )
                    .clicked()
                {
                    action = Some(TreeAction::AddFolder {
                        parent_id: Some(id.clone()),
                    });
                    ui.close_menu();
                }
                crate::ctx_menu::separator(ui);
                if crate::ctx_menu::item(
                    ui,
                    Some(Icon::Pencil),
                    &i18n::t("tree.ctx.rename_folder"),
                    None,
                    true,
                )
                .clicked()
                {
                    action = Some(TreeAction::RenameFolder {
                        id: id.clone(),
                        name: name.clone(),
                    });
                    ui.close_menu();
                }
                if crate::ctx_menu::item(
                    ui,
                    Some(Icon::Trash),
                    &i18n::t("tree.ctx.delete_folder"),
                    None,
                    true,
                )
                .clicked()
                {
                    action = Some(TreeAction::DeleteFolder {
                        id: id.clone(),
                        name: name.clone(),
                    });
                    ui.close_menu();
                }
            });
            action.or(body_opt.map(|b| b.inner).flatten())
        }
        TreeNode::Session { name, session_ref } => {
            let selected = matches!(
                selection,
                Some(TreeSelection::Session { session_ref: r, .. }) if r == session_ref
            );
            let color = if selected {
                ui_icon::COLOR_ACCENT
            } else {
                Color32::from_rgb(32, 34, 40)
            };
            let label = ui_icon::with_label(Icon::Server, name, 14.0, color);
            let resp = ui.add(
                egui::Button::new(label)
                    .frame(false)
                    .sense(Sense::click_and_drag())
                    .min_size([ui.available_width(), 22.0].into()),
            );
            resp.dnd_set_drag_payload(SessionDragPayload {
                session_ref: session_ref.clone(),
                name: name.clone(),
            });
            if resp.clicked() {
                *selection = Some(TreeSelection::Session {
                    name: name.clone(),
                    session_ref: session_ref.clone(),
                });
            }
            if resp.double_clicked() {
                return Some(TreeAction::OpenSession {
                    name: name.clone(),
                    session_ref: session_ref.clone(),
                });
            }
            let mut action = None;
            resp.context_menu(|ui| {
                crate::ctx_menu::prepare(ui);
                if crate::ctx_menu::item(
                    ui,
                    Some(Icon::Terminal),
                    &i18n::t("tree.ctx.connect"),
                    None,
                    true,
                )
                .clicked()
                {
                    action = Some(TreeAction::OpenSession {
                        name: name.clone(),
                        session_ref: session_ref.clone(),
                    });
                    ui.close_menu();
                }
                if crate::ctx_menu::item(
                    ui,
                    Some(Icon::Pencil),
                    &i18n::t("tree.ctx.edit"),
                    None,
                    true,
                )
                .clicked()
                {
                    action = Some(TreeAction::EditServer {
                        session_ref: session_ref.clone(),
                    });
                    ui.close_menu();
                }
                crate::ctx_menu::separator(ui);
                if crate::ctx_menu::item(
                    ui,
                    Some(Icon::Trash),
                    &i18n::t("tree.ctx.delete"),
                    None,
                    true,
                )
                .clicked()
                {
                    action = Some(TreeAction::DeleteServer {
                        session_ref: session_ref.clone(),
                        name: name.clone(),
                    });
                    ui.close_menu();
                }
            });
            resp.on_hover_text(i18n::t("tree.open_hint"));
            action
        }
    }
}
