//! Encrypted credential vault for passwords and key passphrases.

mod error;
mod store;

pub use error::VaultError;
pub use store::{SecretRef, Vault};

/// Parse `vault://entry-id` references used in session YAML.
pub fn parse_vault_ref(value: &str) -> Option<&str> {
    value.strip_prefix("vault://").filter(|s| !s.is_empty())
}

pub fn format_vault_ref(entry_id: &str) -> String {
    format!("vault://{entry_id}")
}
