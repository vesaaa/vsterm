//! Dual SSH backend abstraction and connection lifecycle management.

mod backend;
mod error;
mod manager;
mod remote_exec;
mod ssh_io;
mod system_ssh;
mod user_error;

pub use backend::{SshBackend, SshChannel, SshSession};
pub use error::ConnError;
pub use manager::{
    ActiveConnection, ConnectionId, ConnectionManager, ConnectionMeta, ConnectionState,
    SharedConnectionManager,
};
pub use remote_exec::RemoteSession;
pub use system_ssh::{
    auth_failure_text, backend_unavailable_error, preflight, resolve_auth, resolve_backend,
    system_ssh_install_hint, AuthMaterial, SystemSshBackend,
};
pub use user_error::{ConnErrorKey, ConnectFailure};

// Russh backend will be fully wired in stage 4; stub is exported for the trait surface.
pub mod russh_backend;
pub use russh_backend::RusshBackend;
