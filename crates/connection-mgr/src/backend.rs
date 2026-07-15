use crate::ConnError;
use async_trait::async_trait;
use session_tree::SessionConfig;
use std::io;
use vault::Vault;

/// Byte-oriented interactive shell channel.
pub trait SshChannel: Send {
    fn reader(&mut self) -> &mut dyn io::Read;
    fn writer(&mut self) -> &mut dyn io::Write;
}

#[async_trait]
pub trait SshSession: Send + Sync {
    async fn open_shell(&mut self, term_size: (u16, u16)) -> Result<Box<dyn SshChannel>, ConnError>;
    async fn resize_pty(&mut self, cols: u16, rows: u16) -> Result<(), ConnError>;
    async fn disconnect(&mut self) -> Result<(), ConnError>;
    fn is_alive(&self) -> bool;
}

#[async_trait]
pub trait SshBackend: Send + Sync {
    async fn connect(&self, config: &SessionConfig) -> Result<Box<dyn SshSession>, ConnError>;
}

/// One-shot remote command runner (metrics / routes / probes).
///
/// Interactive shell and one-shot exec are separate by design:
/// - **System engine**: new `ssh host cmd` process (via [`crate::process::command`]).
/// - **Builtin (russh)**: open an `exec` channel on the existing authenticated session
///   (no console process; implement this trait and attach via [`crate::RemoteSession::from_exec`]).
pub trait RemoteExec: Send + Sync {
    fn run_command(&self, vault: Option<&Vault>, remote_cmd: &str) -> Result<String, ConnError>;
}
