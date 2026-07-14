//! Builtin SSH engine based on `russh` (fully wired in stage 4).

use crate::backend::{SshBackend, SshChannel, SshSession};
use crate::ConnError;
use async_trait::async_trait;
use session_tree::SessionConfig;
use std::io;

/// Placeholder russh backend. Stage 4 will implement TCP connect, auth, and shell channel.
pub struct RusshBackend;

impl RusshBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RusshBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SshBackend for RusshBackend {
    async fn connect(&self, config: &SessionConfig) -> Result<Box<dyn SshSession>, ConnError> {
        Err(ConnError::Backend(format!(
            "russh builtin backend not yet implemented (host={}:{}) — use system backend or local shell for now",
            config.host, config.port
        )))
    }
}

/// Reserved for stage 4 channel adapter.
#[allow(dead_code)]
struct RusshChannelStub;

impl SshChannel for RusshChannelStub {
    fn reader(&mut self) -> &mut dyn io::Read {
        unreachable!("russh channel stub")
    }

    fn writer(&mut self) -> &mut dyn io::Write {
        unreachable!("russh channel stub")
    }
}

#[allow(dead_code)]
struct RusshSessionStub {
    alive: bool,
}

#[async_trait]
impl SshSession for RusshSessionStub {
    async fn open_shell(
        &mut self,
        _term_size: (u16, u16),
    ) -> Result<Box<dyn SshChannel>, ConnError> {
        Err(ConnError::Backend("russh session stub".into()))
    }

    async fn resize_pty(&mut self, _cols: u16, _rows: u16) -> Result<(), ConnError> {
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), ConnError> {
        self.alive = false;
        Ok(())
    }

    fn is_alive(&self) -> bool {
        self.alive
    }
}
