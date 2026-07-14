//! Dual SSH backend abstraction and connection lifecycle management.

mod backend;
mod error;
mod manager;
mod system_ssh;

pub use backend::{SshBackend, SshChannel, SshSession};
pub use error::ConnError;
pub use manager::{
    ActiveConnection, ConnectionId, ConnectionManager, ConnectionMeta, ConnectionState,
    SharedConnectionManager,
};
pub use system_ssh::SystemSshBackend;

// Russh backend will be fully wired in stage 4; stub is exported for the trait surface.
pub mod russh_backend;
pub use russh_backend::RusshBackend;
