use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

    /// True when password auth has no saved vault ref.
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

    pub fn has_vault_password(&self) -> bool {
        matches!(
            self,
            Self::Password {
                password_ref: Some(r)
            } if !r.trim().is_empty()
        )
    }

    pub fn private_key_path(&self) -> Option<&std::path::Path> {
        match self {
            Self::Publickey {
                private_key_path, ..
            } => Some(private_key_path.as_path()),
            _ => None,
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
    /// May be empty — UI collects username at connect time.
    #[serde(default)]
    pub username: String,
    pub auth: AuthConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_tag: Option<String>,
    /// OS icon id (`debian`, `ubuntu`, …). `None` = auto-detect after connect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default = "default_term_type")]
    pub term_type: String,
    /// Session-scoped OSC 7/133 shell integration. Default on; disable per host
    /// when a server's audit policy rejects remote bootstrap commands.
    ///
    /// Legacy YAML may still contain a removed `backend` field — serde ignores it.
    #[serde(default = "default_shell_integration")]
    pub shell_integration: bool,
}

fn default_port() -> u16 {
    22
}

fn default_term_type() -> String {
    "xterm-256color".into()
}

fn default_shell_integration() -> bool {
    true
}

impl SessionConfig {
    pub fn display_label(&self) -> String {
        let user = self.username.trim();
        if user.is_empty() {
            format!("{}:{}", self.host, self.port)
        } else {
            format!("{user}@{}:{}", self.host, self.port)
        }
    }

    pub fn needs_password_prompt(&self) -> bool {
        self.auth.needs_password_prompt()
    }

    /// Always collect username/password interactively for password auth.
    pub fn needs_credentials_dialog(&self) -> bool {
        matches!(self.auth, AuthConfig::Password { .. })
    }

    /// Collect / confirm private key path for publickey auth.
    pub fn needs_key_dialog(&self) -> bool {
        matches!(self.auth, AuthConfig::Publickey { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_integration_defaults_on_and_ignores_legacy_backend() {
        let yaml = r#"
id: s-test
name: demo
host: 127.0.0.1
username: root
backend: system
auth:
  type: password
"#;
        let cfg: SessionConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(cfg.shell_integration);
        assert_eq!(cfg.term_type, "xterm-256color");
    }

    #[test]
    fn shell_integration_can_be_disabled() {
        let yaml = r#"
id: s-test
name: demo
host: 127.0.0.1
username: root
shell_integration: false
auth:
  type: password
"#;
        let cfg: SessionConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(!cfg.shell_integration);
    }
}
