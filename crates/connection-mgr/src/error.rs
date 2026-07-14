use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConnError {
    #[error("connection failed: {0}")]
    Connect(String),

    #[error("authentication failed: {0}")]
    AuthFailed(String),

    #[error("host key mismatch: {0}")]
    HostKeyMismatch(String),

    #[error("host key unknown: fingerprint={fingerprint}")]
    HostKeyUnknown { fingerprint: String },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("pty/term error: {0}")]
    Term(String),

    #[error("backend error: {0}")]
    Backend(String),

    #[error("session not found: {0}")]
    NotFound(String),

    #[error("not connected")]
    NotConnected,

    #[error("vault error: {0}")]
    Vault(String),
}
