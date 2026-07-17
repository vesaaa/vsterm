//! Map connection errors to localized user messages.

use connection_mgr::{ConnError, ConnErrorKey, ConnectFailure};
use crate::i18n;

#[derive(Clone)]
pub struct ConnErrorDisplay {
    pub title: String,
    pub detail: Option<String>,
    pub hint: String,
}

fn suffix(key: ConnErrorKey) -> &'static str {
    match key {
        ConnErrorKey::ConnectFailed => "connect_failed",
        ConnErrorKey::AuthFailed => "auth_failed",
        ConnErrorKey::HostKeyMismatch => "host_key_mismatch",
        ConnErrorKey::HostKeyUnknown => "host_key_unknown",
        ConnErrorKey::Io => "io",
        ConnErrorKey::Term => "term",
        ConnErrorKey::Backend => "backend",
        ConnErrorKey::NotFound => "not_found",
        ConnErrorKey::NotConnected => "not_connected",
        ConnErrorKey::Vault => "vault",
        ConnErrorKey::PrivateKeyMissing => "private_key_missing",
        ConnErrorKey::VaultSecretMissing => "vault_secret_missing",
        ConnErrorKey::InvalidConfig => "invalid_config",
    }
}

pub fn format_conn_error(err: &ConnError) -> ConnErrorDisplay {
    let s = suffix(err.i18n_key());
    ConnErrorDisplay {
        title: i18n::t(&format!("err.conn.{s}.title")),
        detail: err.detail(),
        hint: i18n::t(&format!("err.conn.{s}.hint")),
    }
}

pub fn format_connect_failure(failure: &ConnectFailure) -> ConnErrorDisplay {
    let s = suffix(failure.key);
    ConnErrorDisplay {
        title: i18n::t(&format!("err.conn.{s}.title")),
        detail: failure.detail.clone(),
        hint: i18n::t(&format!("err.conn.{s}.hint")),
    }
}
