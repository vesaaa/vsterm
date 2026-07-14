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
    }
}
