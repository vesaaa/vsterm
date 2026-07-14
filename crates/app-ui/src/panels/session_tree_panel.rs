use crate::i18n;
use egui::{Color32, RichText, Ui};
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
    AddFolder,
    RenameFolder { id: String, name: String },
    DeleteFolder { id: String, name: String },
}

pub fn show(
    ui: &mut Ui,
    tree: &SessionTree,
    selection: &mut Option<TreeSelection>,
) -> Option<TreeAction> {
    let mut action = None;

    ui.horizontal(|ui| {
        if ui
            .button(RichText::new(i18n::t("tree.add_server")).size(12.0))
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
            .button(RichText::new(i18n::t("tree.add_folder")).size(12.0))
            .clicked()
        {
            action = Some(TreeAction::AddFolder);
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
    egui::ScrollArea::vertical()
        .id_salt("session_tree_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for node in &tree.root {
                if let Some(a) = render_node(ui, node, selection) {
                    action = Some(a);
                }
            }
            if tree.root.is_empty() {
                ui.weak(i18n::t("tree.empty"));
            }
        });

    action
}

fn render_node(
    ui: &mut Ui,
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
            let header = if selected {
                RichText::new(format!("📁 {name}"))
                    .size(13.0)
                    .color(Color32::from_rgb(30, 80, 160))
            } else {
                RichText::new(format!("📁 {name}")).size(13.0)
            };
            let response = egui::CollapsingHeader::new(header)
                .id_salt(("folder", id.as_str()))
                .default_open(true)
                .show(ui, |ui| {
                    let mut action = None;
                    for child in children {
                        if let Some(a) = render_node(ui, child, selection) {
                            action = Some(a);
                        }
                    }
                    action
                });

            // Click folder header row to select.
            if response.header_response.clicked() {
                *selection = Some(TreeSelection::Folder {
                    id: id.clone(),
                    name: name.clone(),
                });
            }
            let mut action = None;
            response.header_response.context_menu(|ui| {
                if ui.button(i18n::t("tree.ctx.add_server")).clicked() {
                    action = Some(TreeAction::AddServer {
                        folder_id: Some(id.clone()),
                    });
                    ui.close_menu();
                }
                if ui.button(i18n::t("tree.ctx.rename_folder")).clicked() {
                    action = Some(TreeAction::RenameFolder {
                        id: id.clone(),
                        name: name.clone(),
                    });
                    ui.close_menu();
                }
                if ui.button(i18n::t("tree.ctx.delete_folder")).clicked() {
                    action = Some(TreeAction::DeleteFolder {
                        id: id.clone(),
                        name: name.clone(),
                    });
                    ui.close_menu();
                }
            });
            action.or(response.body_returned.flatten())
        }
        TreeNode::Session { name, session_ref } => {
            let selected = matches!(
                selection,
                Some(TreeSelection::Session { session_ref: r, .. }) if r == session_ref
            );
            let label = if selected {
                RichText::new(format!("🖥  {name}"))
                    .color(Color32::from_rgb(30, 80, 160))
            } else {
                RichText::new(format!("🖥  {name}"))
            };
            let resp = ui.add(
                egui::Button::new(label)
                    .frame(false)
                    .min_size([ui.available_width(), 22.0].into()),
            );
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
            resp.clone().context_menu(|ui| {
                if ui.button(i18n::t("tree.ctx.connect")).clicked() {
                    action = Some(TreeAction::OpenSession {
                        name: name.clone(),
                        session_ref: session_ref.clone(),
                    });
                    ui.close_menu();
                }
                if ui.button(i18n::t("tree.ctx.edit")).clicked() {
                    action = Some(TreeAction::EditServer {
                        session_ref: session_ref.clone(),
                    });
                    ui.close_menu();
                }
                if ui.button(i18n::t("tree.ctx.delete")).clicked() {
                    action = Some(TreeAction::DeleteServer {
                        session_ref: session_ref.clone(),
                        name: name.clone(),
                    });
                    ui.close_menu();
                }
            });
            if resp.hovered() {
                resp.on_hover_text(i18n::t("tree.open_hint"));
            }
            action
        }
    }
}
