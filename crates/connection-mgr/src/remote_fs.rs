//! Remote filesystem (SFTP) over an authenticated SSH session.

use crate::ConnError;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// One directory entry from a remote `ls`.
#[derive(Debug, Clone)]
pub struct RemoteDirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: Option<u64>,
    /// Unix seconds, when the server provides it.
    pub mtime: Option<u64>,
}

/// Progress: `(bytes_transferred, total_bytes_if_known)`.
/// Shared progress slot updated from the worker thread.
#[derive(Clone, Default)]
pub struct ArcProgress {
    inner: std::sync::Arc<parking_lot::Mutex<TransferProgressState>>,
    cancel: std::sync::Arc<AtomicBool>,
}

#[derive(Debug, Clone, Default)]
pub struct TransferProgressState {
    pub transferred: u64,
    pub total: Option<u64>,
    pub done: bool,
    pub error: Option<String>,
}

impl ArcProgress {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> TransferProgressState {
        self.inner.lock().clone()
    }

    pub fn set(&self, transferred: u64, total: Option<u64>) {
        let mut g = self.inner.lock();
        g.transferred = transferred;
        g.total = total;
    }

    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    pub fn finish_ok(&self) {
        let mut g = self.inner.lock();
        // Snap the bar to 100% so the UI does not flash a stale mid-transfer
        // fraction when the worker finishes between two paint frames.
        if let Some(t) = g.total {
            g.transferred = t;
        }
        g.done = true;
        g.error = None;
    }

    pub fn finish_err(&self, msg: impl Into<String>) {
        let mut g = self.inner.lock();
        g.done = true;
        g.error = Some(msg.into());
    }
}

/// Engine-agnostic remote filesystem ops (blocking; call off the UI thread).
pub trait RemoteFs: Send + Sync {
    /// `true` when this session can open a real SFTP subsystem.
    fn sftp_supported(&self) -> bool {
        true
    }

    fn list_dir(&self, path: &str) -> Result<Vec<RemoteDirEntry>, ConnError>;

    fn get_file(
        &self,
        remote_path: &str,
        local_path: &Path,
        progress: Option<&ArcProgress>,
    ) -> Result<(), ConnError>;

    fn put_file(
        &self,
        local_path: &Path,
        remote_path: &str,
        progress: Option<&ArcProgress>,
    ) -> Result<(), ConnError>;

    /// Download a remote file or directory tree to a local path.
    fn get_path(
        &self,
        remote_path: &str,
        local_path: &Path,
        progress: Option<&ArcProgress>,
    ) -> Result<(), ConnError> {
        self.get_file(remote_path, local_path, progress)
    }

    /// Upload a local file or directory tree to a remote path.
    fn put_path(
        &self,
        local_path: &Path,
        remote_path: &str,
        progress: Option<&ArcProgress>,
    ) -> Result<(), ConnError> {
        self.put_file(local_path, remote_path, progress)
    }

    fn remove(&self, remote_path: &str, is_dir: bool) -> Result<(), ConnError>;

    fn rename(&self, from: &str, to: &str) -> Result<(), ConnError>;

    /// Create an empty remote directory (`mkdir`).
    fn mkdir(&self, remote_path: &str) -> Result<(), ConnError>;

    /// Create/overwrite a remote file with raw bytes (UTF-8 text uses plain bytes).
    fn write_file(&self, remote_path: &str, data: &[u8]) -> Result<(), ConnError>;
}

/// Honest stub for exec-only sessions without a filesystem provider.
pub struct UnsupportedRemoteFs;

impl RemoteFs for UnsupportedRemoteFs {
    fn sftp_supported(&self) -> bool {
        false
    }

    fn list_dir(&self, _path: &str) -> Result<Vec<RemoteDirEntry>, ConnError> {
        Err(ConnError::Backend(sftp_unsupported_msg().into()))
    }

    fn get_file(
        &self,
        _remote_path: &str,
        _local_path: &Path,
        _progress: Option<&ArcProgress>,
    ) -> Result<(), ConnError> {
        Err(ConnError::Backend(sftp_unsupported_msg().into()))
    }

    fn put_file(
        &self,
        _local_path: &Path,
        _remote_path: &str,
        _progress: Option<&ArcProgress>,
    ) -> Result<(), ConnError> {
        Err(ConnError::Backend(sftp_unsupported_msg().into()))
    }

    fn get_path(
        &self,
        _remote_path: &str,
        _local_path: &Path,
        _progress: Option<&ArcProgress>,
    ) -> Result<(), ConnError> {
        Err(ConnError::Backend(sftp_unsupported_msg().into()))
    }

    fn put_path(
        &self,
        _local_path: &Path,
        _remote_path: &str,
        _progress: Option<&ArcProgress>,
    ) -> Result<(), ConnError> {
        Err(ConnError::Backend(sftp_unsupported_msg().into()))
    }

    fn remove(&self, _remote_path: &str, _is_dir: bool) -> Result<(), ConnError> {
        Err(ConnError::Backend(sftp_unsupported_msg().into()))
    }

    fn rename(&self, _from: &str, _to: &str) -> Result<(), ConnError> {
        Err(ConnError::Backend(sftp_unsupported_msg().into()))
    }

    fn mkdir(&self, _remote_path: &str) -> Result<(), ConnError> {
        Err(ConnError::Backend(sftp_unsupported_msg().into()))
    }

    fn write_file(&self, _remote_path: &str, _data: &[u8]) -> Result<(), ConnError> {
        Err(ConnError::Backend(sftp_unsupported_msg().into()))
    }
}

pub fn sftp_unsupported_msg() -> &'static str {
    "SFTP is unavailable for this remote session"
}

/// Join parent + name into a remote path (Unix-style).
pub fn join_remote(parent: &str, name: &str) -> String {
    let name = name.trim_matches('/');
    if name.is_empty() || name == "." {
        return normalize_remote(parent);
    }
    if name == ".." {
        return parent_remote(parent);
    }
    let p = parent.trim_end_matches('/');
    if p.is_empty() || p == "/" {
        format!("/{name}")
    } else {
        format!("{p}/{name}")
    }
}

pub fn parent_remote(path: &str) -> String {
    let p = normalize_remote(path);
    if p == "/" {
        return "/".into();
    }
    match p.rsplit_once('/') {
        Some(("", _)) => "/".into(),
        Some((parent, _)) if parent.is_empty() => "/".into(),
        Some((parent, _)) => parent.to_string(),
        None => "/".into(),
    }
}

pub fn normalize_remote(path: &str) -> String {
    let t = path.trim();
    if t.is_empty() {
        return "/".into();
    }
    let mut out = if t.starts_with('/') {
        t.to_string()
    } else {
        format!("/{t}")
    };
    while out.contains("//") {
        out = out.replace("//", "/");
    }
    if out.len() > 1 {
        out = out.trim_end_matches('/').to_string();
    }
    if out.is_empty() {
        "/".into()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_and_join() {
        assert_eq!(normalize_remote(""), "/");
        assert_eq!(normalize_remote("home"), "/home");
        assert_eq!(normalize_remote("/home/"), "/home");
        assert_eq!(join_remote("/", "etc"), "/etc");
        assert_eq!(join_remote("/home", "user"), "/home/user");
        assert_eq!(join_remote("/home/user", ".."), "/home");
        assert_eq!(parent_remote("/"), "/");
        assert_eq!(parent_remote("/a/b"), "/a");
    }
}
