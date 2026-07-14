use serde::{Deserialize, Serialize};

/// Node in the persisted session tree (`tree.yaml`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TreeNode {
    Folder {
        name: String,
        id: String,
        #[serde(default)]
        children: Vec<TreeNode>,
    },
    Session {
        name: String,
        /// Relative filename under `sessions/`, e.g. `prod-web-01.yaml`
        #[serde(rename = "ref")]
        session_ref: String,
    },
}

impl TreeNode {
    pub fn name(&self) -> &str {
        match self {
            Self::Folder { name, .. } | Self::Session { name, .. } => name,
        }
    }

    pub fn is_folder(&self) -> bool {
        matches!(self, Self::Folder { .. })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionTree {
    #[serde(default)]
    pub root: Vec<TreeNode>,
}

impl SessionTree {
    pub fn new() -> Self {
        Self { root: Vec::new() }
    }

    pub fn walk<'a>(&'a self) -> impl Iterator<Item = &'a TreeNode> {
        TreeWalker {
            stack: self.root.iter().rev().collect(),
        }
    }

    pub fn find_session_ref(&self, name: &str) -> Option<&str> {
        for node in self.walk() {
            if let TreeNode::Session {
                name: n,
                session_ref,
            } = node
            {
                if n == name {
                    return Some(session_ref);
                }
            }
        }
        None
    }

    pub fn contains_session_ref(&self, session_ref: &str) -> bool {
        self.walk().any(|n| {
            matches!(
                n,
                TreeNode::Session {
                    session_ref: r,
                    ..
                } if r == session_ref
            )
        })
    }

    pub fn contains_folder_id(&self, folder_id: &str) -> bool {
        self.walk()
            .any(|n| matches!(n, TreeNode::Folder { id, .. } if id == folder_id))
    }

    /// `(folder_id, folder_name)` for every folder, depth-first.
    pub fn list_folders(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        collect_folders(&self.root, &mut out);
        out
    }

    /// Parent folder id of a session, or `None` if at root / missing.
    pub fn folder_of_session(&self, session_ref: &str) -> Option<String> {
        find_folder_of_session(&self.root, session_ref)
    }

    pub fn insert_session(
        &mut self,
        folder_id: Option<&str>,
        name: String,
        session_ref: String,
    ) -> Result<(), crate::SessionTreeError> {
        if self.contains_session_ref(&session_ref) {
            return Err(crate::SessionTreeError::DuplicateId(session_ref));
        }
        let node = TreeNode::Session { name, session_ref };
        if let Some(fid) = folder_id {
            let children = find_folder_children_mut(&mut self.root, fid).ok_or_else(|| {
                crate::SessionTreeError::NotFound(format!("folder:{fid}"))
            })?;
            children.push(node);
        } else {
            self.root.push(node);
        }
        Ok(())
    }

    /// Update display name and/or move between folders.
    pub fn relocate_session(
        &mut self,
        session_ref: &str,
        new_name: String,
        folder_id: Option<&str>,
    ) -> Result<(), crate::SessionTreeError> {
        self.remove_session_node(session_ref).ok_or_else(|| {
            crate::SessionTreeError::NotFound(session_ref.into())
        })?;
        self.insert_session(folder_id, new_name, session_ref.to_string())
    }

    /// Remove session node from the tree. Returns the display name if found.
    pub fn remove_session_node(&mut self, session_ref: &str) -> Option<String> {
        remove_session_from_list(&mut self.root, session_ref)
    }

    pub fn rename_session(&mut self, session_ref: &str, new_name: String) -> bool {
        rename_session_in_list(&mut self.root, session_ref, new_name)
    }

    pub fn add_folder(&mut self, name: String, id: String) -> Result<(), crate::SessionTreeError> {
        if self.contains_folder_id(&id) {
            return Err(crate::SessionTreeError::DuplicateId(id));
        }
        self.root.push(TreeNode::Folder {
            name,
            id,
            children: Vec::new(),
        });
        Ok(())
    }

    /// Remove an empty folder. Errors if missing or still has children.
    pub fn remove_folder(&mut self, folder_id: &str) -> Result<(), crate::SessionTreeError> {
        match take_folder_if_empty(&mut self.root, folder_id) {
            TakeFolder::Removed => Ok(()),
            TakeFolder::NotEmpty => Err(crate::SessionTreeError::InvalidPath(format!(
                "folder '{folder_id}' is not empty"
            ))),
            TakeFolder::Missing => Err(crate::SessionTreeError::NotFound(format!(
                "folder:{folder_id}"
            ))),
        }
    }

    pub fn rename_folder(&mut self, folder_id: &str, new_name: String) -> bool {
        rename_folder_in_list(&mut self.root, folder_id, new_name)
    }
}

fn collect_folders(nodes: &[TreeNode], out: &mut Vec<(String, String)>) {
    for n in nodes {
        if let TreeNode::Folder { name, id, children } = n {
            out.push((id.clone(), name.clone()));
            collect_folders(children, out);
        }
    }
}

fn find_folder_of_session(nodes: &[TreeNode], session_ref: &str) -> Option<String> {
    for n in nodes {
        match n {
            TreeNode::Folder { id, children, .. } => {
                for c in children {
                    if let TreeNode::Session {
                        session_ref: r, ..
                    } = c
                    {
                        if r == session_ref {
                            return Some(id.clone());
                        }
                    }
                }
                if let Some(found) = find_folder_of_session(children, session_ref) {
                    return Some(found);
                }
            }
            TreeNode::Session { .. } => {}
        }
    }
    None
}

fn find_folder_children_mut<'a>(
    nodes: &'a mut [TreeNode],
    folder_id: &str,
) -> Option<&'a mut Vec<TreeNode>> {
    for n in nodes {
        if let TreeNode::Folder { id, children, .. } = n {
            if id == folder_id {
                return Some(children);
            }
            if let Some(found) = find_folder_children_mut(children, folder_id) {
                return Some(found);
            }
        }
    }
    None
}

fn remove_session_from_list(nodes: &mut Vec<TreeNode>, session_ref: &str) -> Option<String> {
    let mut idx = None;
    let mut name = None;
    for (i, n) in nodes.iter().enumerate() {
        if let TreeNode::Session {
            name: nme,
            session_ref: r,
        } = n
        {
            if r == session_ref {
                idx = Some(i);
                name = Some(nme.clone());
                break;
            }
        }
    }
    if let Some(i) = idx {
        nodes.remove(i);
        return name;
    }
    for n in nodes.iter_mut() {
        if let TreeNode::Folder { children, .. } = n {
            if let Some(name) = remove_session_from_list(children, session_ref) {
                return Some(name);
            }
        }
    }
    None
}

fn rename_session_in_list(nodes: &mut [TreeNode], session_ref: &str, new_name: String) -> bool {
    for n in nodes.iter_mut() {
        match n {
            TreeNode::Session {
                name,
                session_ref: r,
            } if r == session_ref => {
                *name = new_name;
                return true;
            }
            TreeNode::Folder { children, .. } => {
                if rename_session_in_list(children, session_ref, new_name.clone()) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn rename_folder_in_list(nodes: &mut [TreeNode], folder_id: &str, new_name: String) -> bool {
    for n in nodes.iter_mut() {
        if let TreeNode::Folder { id, name, children } = n {
            if id == folder_id {
                *name = new_name;
                return true;
            }
            if rename_folder_in_list(children, folder_id, new_name.clone()) {
                return true;
            }
        }
    }
    false
}

enum TakeFolder {
    Removed,
    NotEmpty,
    Missing,
}

fn take_folder_if_empty(nodes: &mut Vec<TreeNode>, folder_id: &str) -> TakeFolder {
    let mut idx = None;
    let mut empty = false;
    for (i, n) in nodes.iter().enumerate() {
        if let TreeNode::Folder { id, children, .. } = n {
            if id == folder_id {
                idx = Some(i);
                empty = children.is_empty();
                break;
            }
        }
    }
    if let Some(i) = idx {
        if !empty {
            return TakeFolder::NotEmpty;
        }
        nodes.remove(i);
        return TakeFolder::Removed;
    }
    for n in nodes.iter_mut() {
        if let TreeNode::Folder { children, .. } = n {
            match take_folder_if_empty(children, folder_id) {
                TakeFolder::Missing => {}
                other => return other,
            }
        }
    }
    TakeFolder::Missing
}

struct TreeWalker<'a> {
    stack: Vec<&'a TreeNode>,
}

impl<'a> Iterator for TreeWalker<'a> {
    type Item = &'a TreeNode;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.stack.pop()?;
        if let TreeNode::Folder { children, .. } = node {
            for child in children.iter().rev() {
                self.stack.push(child);
            }
        }
        Some(node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sample_tree() {
        let yaml = r#"
root:
  - type: folder
    name: 生产环境
    id: f001
    children:
      - type: session
        name: web-01
        ref: prod-web-01.yaml
"#;
        let tree: SessionTree = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(tree.root.len(), 1);
        assert_eq!(tree.find_session_ref("web-01"), Some("prod-web-01.yaml"));
        assert_eq!(
            tree.folder_of_session("prod-web-01.yaml").as_deref(),
            Some("f001")
        );
    }

    #[test]
    fn insert_relocate_remove() {
        let mut tree = SessionTree::new();
        tree.add_folder("Prod".into(), "f1".into()).unwrap();
        tree.insert_session(Some("f1"), "web".into(), "web.yaml".into())
            .unwrap();
        assert!(tree.contains_session_ref("web.yaml"));
        tree.relocate_session("web.yaml", "web-renamed".into(), None)
            .unwrap();
        assert!(tree.folder_of_session("web.yaml").is_none());
        assert_eq!(tree.find_session_ref("web-renamed"), Some("web.yaml"));
        assert!(tree.remove_session_node("web.yaml").is_some());
        assert!(!tree.contains_session_ref("web.yaml"));
        tree.remove_folder("f1").unwrap();
    }
}
