use crate::i18n;
use egui::{RichText, Ui};
use session_tree::{SessionTree, TreeNode};

pub enum TreeAction {
    OpenSession { name: String, session_ref: String },
    OpenLocalDemo,
}

pub fn show(ui: &mut Ui, tree: &SessionTree) -> Option<TreeAction> {
    let mut action = None;

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
                if let Some(a) = render_node(ui, node) {
                    action = Some(a);
                }
            }
            if tree.root.is_empty() {
                ui.weak(i18n::t("tree.empty"));
            }
        });

    action
}

fn render_node(ui: &mut Ui, node: &TreeNode) -> Option<TreeAction> {
    match node {
        TreeNode::Folder { name, children, .. } => {
            let response = egui::CollapsingHeader::new(RichText::new(format!("📁 {name}")).strong())
                .default_open(true)
                .show(ui, |ui| {
                    let mut action = None;
                    for child in children {
                        if let Some(a) = render_node(ui, child) {
                            action = Some(a);
                        }
                    }
                    action
                });
            response.body_returned.flatten()
        }
        TreeNode::Session { name, session_ref } => {
            let resp = ui.add(
                egui::Button::new(RichText::new(format!("🖥  {name}")))
                    .frame(false)
                    .min_size([ui.available_width(), 22.0].into()),
            );
            if resp.double_clicked() {
                return Some(TreeAction::OpenSession {
                    name: name.clone(),
                    session_ref: session_ref.clone(),
                });
            }
            if resp.hovered() {
                resp.on_hover_text(i18n::t("tree.open_hint"));
            }
            None
        }
    }
}
