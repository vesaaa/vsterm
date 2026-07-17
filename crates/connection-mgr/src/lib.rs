//! Dual connection lifecycle management over the builtin russh SSH engine.

mod auth;
mod backend;
mod error;
mod known_hosts;
mod manager;
mod posix_text;
mod process;
mod remote_exec;
mod remote_fs;
mod remote_tree;
mod shell_integration;
mod ssh_ident;
mod ssh_io;
mod user_error;

pub use auth::{expand_user_path, preflight, resolve_auth, AuthMaterial, PreflightOpts};
pub use backend::RemoteExec;
pub use error::ConnError;
pub use manager::{
    ActiveConnection, ConnectionId, ConnectionManager, ConnectionMeta, ConnectionState,
    EstablishedSsh, SharedConnectionManager,
};
pub use process::{command as gui_command, hide_console};
pub use remote_exec::RemoteSession;
pub use remote_fs::{
    join_remote, normalize_remote, parent_remote, sftp_unsupported_msg, ArcProgress,
    RemoteDirEntry, RemoteFs, TransferProgressState, UnsupportedRemoteFs,
};
pub use ssh_ident::probe_ssh_software_ident;
pub use user_error::{ConnErrorKey, ConnectFailure};

pub mod russh_backend;
pub use russh_backend::RusshBackend;
