//! One-shot remote command runner (metrics / routes / probes).
//!
//! Builtin russh opens an `exec` channel on the existing authenticated session
//! (no console process; implement this trait and attach via
//! [`crate::RemoteSession::from_exec`]).

use crate::ConnError;
use vault::Vault;

pub trait RemoteExec: Send + Sync {
    fn run_command(&self, vault: Option<&Vault>, remote_cmd: &str) -> Result<String, ConnError>;
}
