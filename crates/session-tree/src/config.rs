use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Which SSH backend to use for a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BackendKind {
    /// Prefer system `ssh` if available, else fall back to builtin russh.
    #[default]
    Auto,
    /// Pure Rust russh engine.
    Builtin,
    /// System OpenSSH via portable-pty.
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthType {
    Password,
    Publickey,
}

/// Unified auth intent. Secrets are referenced via `vault://` refs, never plaintext in YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AuthConfig {
    Password {
        /// e.g. `vault://prod-web-01-pwd`. Omit or leave empty to prompt at connect time.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        password_ref: Option<String>,
    },
    Publickey {
        private_key_path: PathBuf,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        passphrase_ref: Option<String>,
    },
}

impl AuthConfig {
    pub fn auth_type(&self) -> AuthType {
        match self {
            Self::Password { .. } => AuthType::Password,
            Self::Publickey { .. } => AuthType::Publickey,
        }
    }

    /// True when password auth has no saved vault ref — UI should prompt before connect.
    pub fn needs_password_prompt(&self) -> bool {
        if let Self::Password { password_ref } = self {
            password_ref
                .as_ref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true)
        } else {
            false
        }
    }
}

/// Single session connection profile (one YAML file under `sessions/`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub id: String,
    pub name: String,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub username: String,
    #[serde(default)]
    pub backend: BackendKind,
    pub auth: AuthConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_tag: Option<String>,
    #[serde(default = "default_term_type")]
    pub term_type: String,
}

fn default_port() -> u16 {
    22
}

fn default_term_type() -> String {
    "xterm-256color".into()
}

impl SessionConfig {
    pub fn display_label(&self) -> String {
        format!("{}@{}:{}", self.username, self.host, self.port)
    }

    pub fn needs_password_prompt(&self) -> bool {
        self.auth.needs_password_prompt()
    }
}
