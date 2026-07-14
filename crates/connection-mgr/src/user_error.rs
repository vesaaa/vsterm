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
    SystemSshMissing,
    BuiltinUnavailable,
    PrivateKeyMissing,
    VaultSecretMissing,
    InvalidConfig,
    BothBackendsUnavailable,
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
            Self::Backend(msg) if msg.starts_with("BUILTIN_UNAVAILABLE:") => {
                ConnErrorKey::BuiltinUnavailable
            }
            Self::Backend(msg) if msg.starts_with("SYSTEM_SSH_MISSING:") => {
                ConnErrorKey::SystemSshMissing
            }
            Self::Backend(msg) if msg.starts_with("BOTH_BACKENDS_UNAVAILABLE:") => {
                ConnErrorKey::BothBackendsUnavailable
            }
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
            | Self::Vault(s) => Some(s.clone()),
            Self::Backend(s) => {
                let detail = s
                    .splitn(2, ':')
                    .nth(1)
                    .unwrap_or(s.as_str())
                    .trim();
                if detail.is_empty() {
                    None
                } else {
                    Some(detail.to_string())
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
