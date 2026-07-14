use thiserror::Error;

#[derive(Debug, Error)]
pub enum SessionTreeError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("yaml error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("session not found: {0}")]
    NotFound(String),

    #[error("invalid path: {0}")]
    InvalidPath(String),

    #[error("duplicate id: {0}")]
    DuplicateId(String),
}
