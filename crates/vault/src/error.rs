use thiserror::Error;

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("keyring error: {0}")]
    Keyring(String),

    #[error("crypto error: {0}")]
    Crypto(String),

    #[error("entry not found: {0}")]
    NotFound(String),

    #[error("invalid vault reference: {0}")]
    InvalidRef(String),

    #[error("vault locked — master passphrase required")]
    Locked,

    #[error("serialization error: {0}")]
    Serde(String),
}
