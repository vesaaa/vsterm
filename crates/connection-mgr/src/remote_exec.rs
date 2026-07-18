//! One-shot remote command execution (metrics / routes) and optional SFTP.
//!
//! Callers go through [`RemoteSession`] → [`RemoteExec`] / [`RemoteFs`].

use crate::backend::RemoteExec;
use crate::error::ConnError;
use crate::remote_fs::{ArcProgress, RemoteDirEntry, RemoteFs, UnsupportedRemoteFs};
use std::path::Path;
use std::sync::Arc;
use vault::Vault;

/// UI-facing handle for running commands / SFTP on the connected SSH host.
#[derive(Clone)]
pub struct RemoteSession {
    /// Stable identity for UI cache keys (metrics / routes / SFTP).
    pub username: String,
    pub host: String,
    exec: Arc<dyn RemoteExec>,
    fs: Arc<dyn RemoteFs>,
}

impl RemoteSession {
    /// Builtin russh (or any engine) with shared exec + filesystem.
    pub fn from_exec_fs(
        username: String,
        host: String,
        exec: Arc<dyn RemoteExec>,
        fs: Arc<dyn RemoteFs>,
    ) -> Self {
        Self {
            username,
            host,
            exec,
            fs,
        }
    }

    /// Attach exec only (SFTP unsupported).
    pub fn from_exec(username: String, host: String, exec: Arc<dyn RemoteExec>) -> Self {
        Self::from_exec_fs(username, host, exec, Arc::new(UnsupportedRemoteFs))
    }

    pub fn display_key(&self) -> String {
        format!("{}@{}", self.username, self.host)
    }

    pub fn run_command(&self, vault: Option<&Vault>, remote_cmd: &str) -> Result<String, ConnError> {
        self.exec.run_command(vault, remote_cmd)
    }

    pub fn sftp_supported(&self) -> bool {
        self.fs.sftp_supported()
    }

    pub fn list_dir(&self, path: &str) -> Result<Vec<RemoteDirEntry>, ConnError> {
        self.fs.list_dir(path)
    }

    pub fn get_file(
        &self,
        remote_path: &str,
        local_path: &Path,
        progress: Option<&ArcProgress>,
    ) -> Result<(), ConnError> {
        self.fs.get_file(remote_path, local_path, progress)
    }

    pub fn put_file(
        &self,
        local_path: &Path,
        remote_path: &str,
        progress: Option<&ArcProgress>,
    ) -> Result<(), ConnError> {
        self.fs.put_file(local_path, remote_path, progress)
    }

    pub fn get_path(
        &self,
        remote_path: &str,
        local_path: &Path,
        progress: Option<&ArcProgress>,
    ) -> Result<(), ConnError> {
        self.fs.get_path(remote_path, local_path, progress)
    }

    pub fn put_path(
        &self,
        local_path: &Path,
        remote_path: &str,
        progress: Option<&ArcProgress>,
    ) -> Result<(), ConnError> {
        self.fs.put_path(local_path, remote_path, progress)
    }

    pub fn remove(&self, remote_path: &str, is_dir: bool) -> Result<(), ConnError> {
        self.fs.remove(remote_path, is_dir)
    }

    pub fn rename(&self, from: &str, to: &str) -> Result<(), ConnError> {
        self.fs.rename(from, to)
    }

    pub fn mkdir(&self, remote_path: &str) -> Result<(), ConnError> {
        self.fs.mkdir(remote_path)
    }

    pub fn write_file(&self, remote_path: &str, data: &[u8]) -> Result<(), ConnError> {
        self.fs.write_file(remote_path, data)
    }

    /// Elevate the file-panel SFTP channel with `sudo` (separate from terminal `sudo -i`).
    pub fn elevate_sftp(&self, password: Option<String>) -> Result<(), ConnError> {
        self.fs.elevate_sftp(password)
    }

    pub fn demote_sftp(&self) -> Result<(), ConnError> {
        self.fs.demote_sftp()
    }

    pub fn sftp_elevated(&self) -> bool {
        self.fs.sftp_elevated()
    }
}
