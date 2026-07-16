//! Recursive SFTP tree transfer (files + directories).

use crate::remote_fs::{join_remote, ArcProgress};
use crate::ConnError;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::FileType;
use std::io::{Read, Write};
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct TransferCtx {
    progress: Option<ArcProgress>,
    transferred: u64,
    total: Option<u64>,
}

impl TransferCtx {
    fn new(progress: Option<ArcProgress>, total: Option<u64>) -> Self {
        if let Some(p) = &progress {
            p.set(0, total);
        }
        Self {
            progress,
            transferred: 0,
            total,
        }
    }

    fn check_cancel(&self) -> Result<(), ConnError> {
        if self
            .progress
            .as_ref()
            .is_some_and(|p| p.is_cancelled())
        {
            Err(ConnError::Connect("sftp transfer cancelled".into()))
        } else {
            Ok(())
        }
    }

    fn add_bytes(&mut self, n: u64) {
        self.transferred += n;
        if let Some(p) = &self.progress {
            p.set(self.transferred, self.total);
        }
    }
}

pub async fn remote_tree_bytes(sftp: &SftpSession, remote_path: &str) -> Result<u64, ConnError> {
    let meta = sftp
        .metadata(remote_path)
        .await
        .map_err(|e| ConnError::Connect(format!("sftp stat {remote_path}: {e}")))?;
    if meta.file_type() == FileType::Symlink {
        return Ok(0);
    }
    if meta.file_type().is_dir() {
        let mut total = 0u64;
        let dir = sftp
            .read_dir(remote_path)
            .await
            .map_err(|e| ConnError::Connect(format!("sftp list {remote_path}: {e}")))?;
        for entry in dir {
            let name = entry.file_name();
            if name == "." || name == ".." {
                continue;
            }
            total += Box::pin(remote_tree_bytes(
                sftp,
                &join_remote(remote_path, &name),
            ))
            .await?;
        }
        Ok(total)
    } else {
        Ok(meta.size.unwrap_or(0))
    }
}

pub async fn download_path(
    sftp: &SftpSession,
    remote_path: &str,
    local_path: &Path,
    ctx: &mut TransferCtx,
) -> Result<(), ConnError> {
    ctx.check_cancel()?;
    let meta = sftp
        .metadata(remote_path)
        .await
        .map_err(|e| ConnError::Connect(format!("sftp stat {remote_path}: {e}")))?;
    if meta.file_type() == FileType::Symlink {
        return Ok(());
    }
    if meta.file_type().is_dir() {
        std::fs::create_dir_all(local_path).map_err(ConnError::Io)?;
        let dir = sftp
            .read_dir(remote_path)
            .await
            .map_err(|e| ConnError::Connect(format!("sftp list {remote_path}: {e}")))?;
        for entry in dir {
            let name = entry.file_name();
            if name == "." || name == ".." {
                continue;
            }
            let child_remote = join_remote(remote_path, &name);
            let child_local = local_path.join(&name);
            Box::pin(download_path(
                sftp,
                &child_remote,
                &child_local,
                ctx,
            ))
            .await?;
        }
        return Ok(());
    }
    download_file(sftp, remote_path, local_path, ctx).await
}

async fn download_file(
    sftp: &SftpSession,
    remote_path: &str,
    local_path: &Path,
    ctx: &mut TransferCtx,
) -> Result<(), ConnError> {
    let mut remote = sftp
        .open(remote_path)
        .await
        .map_err(|e| ConnError::Connect(format!("sftp open {remote_path}: {e}")))?;
    if let Some(parent) = local_path.parent() {
        std::fs::create_dir_all(parent).map_err(ConnError::Io)?;
    }
    let mut local = std::fs::File::create(local_path).map_err(ConnError::Io)?;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        ctx.check_cancel()?;
        let n = remote
            .read(&mut buf)
            .await
            .map_err(|e| ConnError::Connect(format!("sftp read {remote_path}: {e}")))?;
        if n == 0 {
            break;
        }
        local.write_all(&buf[..n]).map_err(ConnError::Io)?;
        ctx.add_bytes(n as u64);
    }
    local.flush().map_err(ConnError::Io)?;
    let _ = remote.shutdown().await;
    Ok(())
}

pub fn local_tree_bytes(local_path: &Path) -> Result<u64, ConnError> {
    let meta = std::fs::symlink_metadata(local_path).map_err(ConnError::Io)?;
    if meta.file_type().is_symlink() {
        return Ok(0);
    }
    if meta.is_dir() {
        let mut total = 0u64;
        for entry in std::fs::read_dir(local_path).map_err(ConnError::Io)? {
            let entry = entry.map_err(ConnError::Io)?;
            total += local_tree_bytes(&entry.path())?;
        }
        Ok(total)
    } else {
        Ok(meta.len())
    }
}

pub async fn upload_path(
    sftp: &SftpSession,
    local_path: &Path,
    remote_path: &str,
    ctx: &mut TransferCtx,
) -> Result<(), ConnError> {
    ctx.check_cancel()?;
    let meta = std::fs::symlink_metadata(local_path).map_err(ConnError::Io)?;
    if meta.file_type().is_symlink() {
        return Ok(());
    }
    if meta.is_dir() {
        if !sftp
            .try_exists(remote_path)
            .await
            .unwrap_or(false)
        {
            sftp.create_dir(remote_path)
                .await
                .map_err(|e| ConnError::Connect(format!("sftp mkdir {remote_path}: {e}")))?;
        }
        for entry in std::fs::read_dir(local_path).map_err(ConnError::Io)? {
            let entry = entry.map_err(ConnError::Io)?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let child_local = entry.path();
            let child_remote = join_remote(remote_path, &name);
            Box::pin(upload_path(
                sftp,
                &child_local,
                &child_remote,
                ctx,
            ))
            .await?;
        }
        return Ok(());
    }
    upload_file(sftp, local_path, remote_path, ctx).await
}

async fn upload_file(
    sftp: &SftpSession,
    local_path: &Path,
    remote_path: &str,
    ctx: &mut TransferCtx,
) -> Result<(), ConnError> {
    let mut local = std::fs::File::open(local_path).map_err(ConnError::Io)?;
    let mut remote = sftp
        .create(remote_path)
        .await
        .map_err(|e| ConnError::Connect(format!("sftp create {remote_path}: {e}")))?;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        ctx.check_cancel()?;
        let n = local.read(&mut buf).map_err(ConnError::Io)?;
        if n == 0 {
            break;
        }
        remote
            .write_all(&buf[..n])
            .await
            .map_err(|e| ConnError::Connect(format!("sftp write {remote_path}: {e}")))?;
        ctx.add_bytes(n as u64);
    }
    remote
        .flush()
        .await
        .map_err(|e| ConnError::Connect(format!("sftp flush {remote_path}: {e}")))?;
    let _ = remote.shutdown().await;
    Ok(())
}

pub async fn run_download(
    sftp: &SftpSession,
    remote_path: &str,
    local_path: &Path,
    progress: Option<&ArcProgress>,
) -> Result<(), ConnError> {
    let total = remote_tree_bytes(sftp, remote_path).await.ok();
    let mut ctx = TransferCtx::new(progress.cloned(), total);
    download_path(sftp, remote_path, local_path, &mut ctx).await
}

pub async fn run_upload(
    sftp: &SftpSession,
    local_path: &Path,
    remote_path: &str,
    progress: Option<&ArcProgress>,
) -> Result<(), ConnError> {
    let total = local_tree_bytes(local_path).ok();
    let mut ctx = TransferCtx::new(progress.cloned(), total);
    upload_path(sftp, local_path, remote_path, &mut ctx).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_tree_bytes_missing_is_error() {
        assert!(local_tree_bytes(Path::new("/nonexistent-vsterm-path-xyz")).is_err());
    }
}
