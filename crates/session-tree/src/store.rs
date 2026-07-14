use crate::config::SessionConfig;
use crate::error::SessionTreeError;
use crate::tree::SessionTree;
use dirs::home_dir;
use std::fs;
use std::path::{Path, PathBuf};

/// Well-known paths under `~/.vsterm/` (or a custom root).
#[derive(Debug, Clone)]
pub struct AppPaths {
    pub root: PathBuf,
}

impl AppPaths {
    pub fn default_root() -> PathBuf {
        home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".vsterm")
    }

    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn sessions_dir(&self) -> PathBuf {
        self.root.join("sessions")
    }

    pub fn tree_path(&self) -> PathBuf {
        self.sessions_dir().join("tree.yaml")
    }

    pub fn credentials_dir(&self) -> PathBuf {
        self.root.join("credentials")
    }

    pub fn vault_path(&self) -> PathBuf {
        self.credentials_dir().join("vault.enc")
    }

    pub fn layouts_dir(&self) -> PathBuf {
        self.root.join("layouts")
    }

    pub fn themes_dir(&self) -> PathBuf {
        self.root.join("themes")
    }

    pub fn config_path(&self) -> PathBuf {
        self.root.join("config.yaml")
    }

    pub fn ensure_dirs(&self) -> Result<(), SessionTreeError> {
        for dir in [
            self.sessions_dir(),
            self.credentials_dir(),
            self.layouts_dir(),
            self.themes_dir(),
        ] {
            fs::create_dir_all(&dir)?;
        }
        Ok(())
    }
}

/// Load / save session tree and individual session configs.
pub struct SessionStore {
    paths: AppPaths,
}

impl SessionStore {
    pub fn new(paths: AppPaths) -> Result<Self, SessionTreeError> {
        paths.ensure_dirs()?;
        Ok(Self { paths })
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    pub fn load_tree(&self) -> Result<SessionTree, SessionTreeError> {
        let path = self.paths.tree_path();
        if !path.exists() {
            return Ok(SessionTree::new());
        }
        let text = fs::read_to_string(&path)?;
        Ok(serde_yaml::from_str(&text)?)
    }

    pub fn save_tree(&self, tree: &SessionTree) -> Result<(), SessionTreeError> {
        let path = self.paths.tree_path();
        let text = serde_yaml::to_string(tree)?;
        fs::write(path, text)?;
        Ok(())
    }

    pub fn load_session(&self, file_name: &str) -> Result<SessionConfig, SessionTreeError> {
        let path = self.resolve_session_path(file_name)?;
        let text = fs::read_to_string(&path)?;
        Ok(serde_yaml::from_str(&text)?)
    }

    pub fn save_session(&self, config: &SessionConfig) -> Result<(), SessionTreeError> {
        let file_name = format!("{}.yaml", config.id);
        let path = self.paths.sessions_dir().join(file_name);
        let text = serde_yaml::to_string(config)?;
        fs::write(path, text)?;
        Ok(())
    }

    pub fn delete_session_file(&self, file_name: &str) -> Result<(), SessionTreeError> {
        let path = self.resolve_session_path(file_name)?;
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    fn resolve_session_path(&self, file_name: &str) -> Result<PathBuf, SessionTreeError> {
        let name = Path::new(file_name)
            .file_name()
            .ok_or_else(|| SessionTreeError::InvalidPath(file_name.into()))?;
        Ok(self.paths.sessions_dir().join(name))
    }
}
