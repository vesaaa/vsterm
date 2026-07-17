//! User-facing error hints (mapped to i18n keys in app-ui).

use crate::error::ConnError;

/// Stable key for i18n lookup in the UI layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnErrorKey {
    ConnectFailed,
    AuthFailed,
    HostKeyMismatch,
    HostKeyUnknown,
    Io,
    Term,
    Backend,
    NotFound,
    NotConnected,
    Vault,
    PrivateKeyMissing,
    VaultSecretMissing,
    InvalidConfig,
}

/// Payload for async UI when a connect attempt fails (Send + Clone).
#[derive(Debug, Clone)]
pub struct ConnectFailure {
    pub key: ConnErrorKey,
    pub detail: Option<String>,
}

impl ConnError {
    pub fn into_failure(self) -> ConnectFailure {
        ConnectFailure {
            key: self.i18n_key(),
            detail: self.detail(),
        }
    }

    pub fn i18n_key(&self) -> ConnErrorKey {
        match self {
            Self::Connect(_) => ConnErrorKey::ConnectFailed,
            Self::AuthFailed(_) => ConnErrorKey::AuthFailed,
            Self::HostKeyMismatch(_) => ConnErrorKey::HostKeyMismatch,
            Self::HostKeyUnknown { .. } => ConnErrorKey::HostKeyUnknown,
            Self::Io(_) => ConnErrorKey::Io,
            Self::Term(_) => ConnErrorKey::Term,
            Self::Backend(_) => ConnErrorKey::Backend,
            Self::NotFound(_) => ConnErrorKey::NotFound,
            Self::NotConnected => ConnErrorKey::NotConnected,
            Self::Vault(_) => ConnErrorKey::Vault,
            Self::PrivateKeyMissing { .. } => ConnErrorKey::PrivateKeyMissing,
            Self::VaultSecretMissing { .. } => ConnErrorKey::VaultSecretMissing,
            Self::InvalidConfig { .. } => ConnErrorKey::InvalidConfig,
        }
    }

    /// Short technical detail shown under the localized title (may be empty).
    pub fn detail(&self) -> Option<String> {
        match self {
            Self::Connect(s)
            | Self::AuthFailed(s)
            | Self::HostKeyMismatch(s)
            | Self::Term(s)
            | Self::NotFound(s)
            | Self::Vault(s)
            | Self::Backend(s) => {
                if s.is_empty() {
                    None
                } else {
                    Some(s.clone())
                }
            }
            Self::HostKeyUnknown { fingerprint } => Some(fingerprint.clone()),
            Self::PrivateKeyMissing { path, .. } => Some(path.display().to_string()),
            Self::VaultSecretMissing { secret_ref } => Some(secret_ref.clone()),
            Self::InvalidConfig { field, reason } => Some(format!("{field}: {reason}")),
            Self::Io(e) => Some(e.to_string()),
            Self::NotConnected => None,
        }
    }
}
