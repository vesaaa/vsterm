use thiserror::Error;

#[derive(Debug, Error)]
pub enum TermError {
    #[error("pty error: {0}")]
    Pty(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("terminal not running")]
    NotRunning,
}
