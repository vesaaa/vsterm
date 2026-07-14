use crate::ConnError;
use async_trait::async_trait;
use session_tree::SessionConfig;
use std::io;

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
