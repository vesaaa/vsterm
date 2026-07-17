//! Shared SSH auth resolution and connect preflight (builtin russh).

use crate::error::ConnError;
use session_tree::{AuthConfig, SessionConfig};
use std::path::{Path, PathBuf};
use vault::Vault;

/// Resolved secrets for one connect attempt (never logged).
#[derive(Debug, Default, Clone)]
pub struct AuthMaterial {
    pub password: Option<String>,
    pub passphrase: Option<String>,
}

pub fn resolve_auth(
    config: &SessionConfig,
    vault: Option<&Vault>,
    interactive_password: Option<String>,
) -> Result<AuthMaterial, ConnError> {
    let mut auth = AuthMaterial::default();
    match &config.auth {
        AuthConfig::Password { password_ref } => {
            // Interactive password wins when the user typed one in the dialog.
            if let Some(pwd) = interactive_password.filter(|p| !p.is_empty()) {
                auth.password = Some(pwd);
            } else if let Some(r) = password_ref.as_ref().filter(|r| !r.trim().is_empty()) {
                auth.password = Some(load_secret(vault, r)?);
            } else {
                return Err(ConnError::InvalidConfig {
                    field: "password".into(),
                    reason: "password required (enter in dialog or set password_ref)".into(),
                });
            }
        }
        AuthConfig::Publickey {
            passphrase_ref: Some(r),
            ..
        } => {
            auth.passphrase = Some(load_secret(vault, r)?);
        }
        AuthConfig::Publickey { .. } => {}
    }
    Ok(auth)
}

pub fn preflight(
    config: &SessionConfig,
    vault: Option<&Vault>,
    opts: PreflightOpts,
) -> Result<(), ConnError> {
    if config.host.trim().is_empty() {
        return Err(ConnError::InvalidConfig {
            field: "host".into(),
            reason: "host is empty".into(),
        });
    }
    if config.username.trim().is_empty() && !opts.allow_empty_username {
        return Err(ConnError::InvalidConfig {
            field: "username".into(),
            reason: "username is empty".into(),
        });
    }

    match &config.auth {
        AuthConfig::Publickey {
            private_key_path, ..
        } => {
            if !opts.skip_key_file_check {
                let path = expand_tilde(private_key_path);
                if !path.exists() {
                    return Err(ConnError::PrivateKeyMissing {
                        path,
                        configured_auth:
                            "auth.type=publickey — switch to password if you use password login"
                                .into(),
                    });
                }
            }
            if let AuthConfig::Publickey {
                passphrase_ref: Some(r),
                ..
            } = &config.auth
            {
                ensure_vault_secret(vault, r)?;
            }
        }
        AuthConfig::Password { password_ref } => {
            if password_ref
                .as_ref()
                .map(|r| !r.trim().is_empty())
                .unwrap_or(false)
            {
                ensure_vault_secret(vault, password_ref.as_ref().unwrap())?;
            } else if !opts.has_interactive_password {
                // UI will collect password before connect; nothing to check here.
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PreflightOpts {
    pub has_interactive_password: bool,
    pub allow_empty_username: bool,
    pub skip_key_file_check: bool,
}

impl PreflightOpts {
    pub fn connecting(has_interactive_password: bool) -> Self {
        Self {
            has_interactive_password,
            allow_empty_username: false,
            skip_key_file_check: false,
        }
    }

    pub fn before_prompt() -> Self {
        Self {
            has_interactive_password: false,
            allow_empty_username: true,
            skip_key_file_check: true,
        }
    }
}

/// Expand `~` in private key / home-relative paths.
pub fn expand_user_path(path: impl AsRef<std::path::Path>) -> PathBuf {
    expand_tilde(path.as_ref())
}

fn ensure_vault_secret(vault: Option<&Vault>, secret_ref: &str) -> Result<(), ConnError> {
    let Some(vault) = vault else {
        return Err(ConnError::VaultSecretMissing {
            secret_ref: secret_ref.to_string(),
        });
    };
    vault
        .get_ref(secret_ref)
        .map(|_| ())
        .map_err(|_| ConnError::VaultSecretMissing {
            secret_ref: secret_ref.to_string(),
        })
}

fn load_secret(vault: Option<&Vault>, secret_ref: &str) -> Result<String, ConnError> {
    let vault = vault.ok_or_else(|| ConnError::VaultSecretMissing {
        secret_ref: secret_ref.to_string(),
    })?;
    vault
        .get_ref(secret_ref)
        .map_err(|e| ConnError::Vault(format!("{secret_ref}: {e}")))
}

fn expand_tilde(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = dirs_home() {
            return home.join(rest);
        }
    }
    if s == "~" {
        if let Some(home) = dirs_home() {
            return home;
        }
    }
    path.to_path_buf()
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}
