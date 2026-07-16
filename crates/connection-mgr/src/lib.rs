//! Dual SSH backend abstraction and connection lifecycle management.

mod backend;
mod error;
mod known_hosts;
mod manager;
mod process;
mod remote_exec;
mod remote_fs;
mod remote_tree;
mod ssh_ident;
mod ssh_io;
mod system_ssh;
mod user_error;

pub use backend::{RemoteExec, SshBackend, SshChannel, SshSession};
pub use error::ConnError;
pub use manager::{
    ActiveConnection, ConnectionId, ConnectionManager, ConnectionMeta, ConnectionState,
    EstablishedSsh, SharedConnectionManager,
};
pub use process::{command as gui_command, hide_console};
pub use remote_exec::RemoteSession;
pub use remote_fs::{
    join_remote, normalize_remote, parent_remote, sftp_system_unsupported_msg, ArcProgress,
    RemoteDirEntry, RemoteFs, TransferProgressState, UnsupportedRemoteFs,
};
pub use ssh_ident::probe_ssh_software_ident;
pub use system_ssh::{
    auth_failure_text, backend_unavailable_error, expand_user_path, preflight, resolve_auth,
    resolve_backend, system_ssh_install_hint, AuthMaterial, PreflightOpts, SystemSshBackend,
};
pub use user_error::{ConnErrorKey, ConnectFailure};

pub mod russh_backend;
pub use russh_backend::RusshBackend;
