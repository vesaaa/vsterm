//! Recursive SFTP tree transfer (files + directories).
//!
//! Download uses pipelined `SSH_FXP_READ` via [`RawSftpSession`] so throughput
//! is not capped at one round-trip per chunk. Upload uses `SftpSession`'s
//! write pipeline and avoids per-file `fsync`.

use crate::remote_fs::{join_remote, ArcProgress, RemoteDirEntry};
use crate::ConnError;
use futures::stream::{FuturesUnordered, StreamExt};
use russh_sftp::client::error::Error as SftpError;
use russh_sftp::client::rawsession::Limits;
use russh_sftp::client::{Config as SftpConfig, RawSftpSession};
use russh_sftp::extensions;
use russh_sftp::protocol::{FileAttributes, FileType, OpenFlags, StatusCode};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Default max SFTP packet size (capped further by server `limits@openssh.com`).
pub const SFTP_MAX_PACKET_LEN: u32 = 1024 * 1024;
/// In-flight WRITE requests for uploads.
pub const SFTP_MAX_CONCURRENT_WRITES: usize = 64;
/// In-flight READ requests for downloads.
/// Kept modest: each reply is a ~256 KiB buffer. 16 × 256 KiB ≈ 4 MiB peak.
const DOWNLOAD_PIPELINE: usize = 16;
/// Fallback chunk when server does not advertise read limits (~256 KiB − overhead).
const DEFAULT_READ_CHUNK: u32 = 256 * 1024 - 9;
/// Local buffer size for uploads (russh-sftp splits + pipelines internally).
const UPLOAD_BUF: usize = 512 * 1024;
/// Chunks queued for the background disk writer (~4 MiB at 256 KiB/chunk).
/// Deliberately small: progress is counted at disk-commit, so the queue only
/// needs to absorb write jitter — not decouple the bar from the disk. A large
/// queue lets the network race ~hundreds of MiB ahead of the disk, which shows
/// a bar that sprints then freezes and pins that RAM for the whole transfer.
const DOWNLOAD_WRITE_QUEUE: usize = 16;
/// Max chunks waiting for in-order reassembly (reorder map). Caps RAM and
/// applies read backpressure so a slow disk cannot pull the file into memory.
const DOWNLOAD_REORDER_CAP: usize = 32;
/// Buffered-writer capacity for the background disk writer.
const DOWNLOAD_WRITE_BUF: usize = 512 * 1024;
/// Flush the BufWriter to the OS every this many bytes (does **not** fsync).
/// Periodic `sync_data` was removed: on Windows it stalls the whole process for
/// ~1s per burst (VSTERM_DIAG FRAME GAP ≈ 1014 ms during downloads) while the
/// cache manager drains dirty pages — menus and typing freeze even though
/// `App::update` itself stays cheap.
const DOWNLOAD_FLUSH_INTERVAL: u64 = 16 * 1024 * 1024;

fn open_download_file(path: &Path) -> std::io::Result<std::fs::File> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Hint the cache manager that we write the file once, sequentially —
        // reduces mid-transfer cache flushes that stall progress.
        const FILE_FLAG_SEQUENTIAL_SCAN: u32 = 0x0800_0000;
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .custom_flags(FILE_FLAG_SEQUENTIAL_SCAN)
            .open(path)
    }
    #[cfg(not(windows))]
    {
        std::fs::File::create(path)
    }
}

/// Partial path beside the final destination. Writing here then renaming avoids
/// truncating an existing file that Windows Defender / Indexer may still hold
/// after a previous download — that lock showed up as “save confirmed, progress
/// bar appears seconds later, then finishes immediately” on the 2nd+ overwrite.
fn download_partial_path(final_path: &Path) -> PathBuf {
    let mut name = final_path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_else(|| "download".into());
    name.push(".vsterm.partial");
    final_path.with_file_name(name)
}

fn promote_download_partial(partial: &Path, final_path: &Path) -> std::io::Result<()> {
    let _ = std::fs::remove_file(final_path);
    match std::fs::rename(partial, final_path) {
        Ok(()) => Ok(()),
        Err(_) => {
            // Cross-volume or AV race: fall back to copy+remove.
            std::fs::copy(partial, final_path)?;
            let _ = std::fs::remove_file(partial);
            Ok(())
        }
    }
}

pub fn transfer_sftp_config() -> SftpConfig {
    SftpConfig {
        max_packet_len: SFTP_MAX_PACKET_LEN,
        max_concurrent_writes: SFTP_MAX_CONCURRENT_WRITES,
        // Large files / high latency must not trip the default 10s request timeout.
        request_timeout_secs: 600,
    }
}

/// Open a raw SFTP session with limits applied; returns `(session, max_read_len)`.
pub async fn init_raw_sftp<S>(stream: S) -> Result<(Arc<RawSftpSession>, u32), ConnError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let cfg = transfer_sftp_config();
    let max_packet = cfg.max_packet_len;
    let mut session = RawSftpSession::new_with_config(stream, cfg);
    let version = session
        .init()
        .await
        .map_err(|e| ConnError::Connect(format!("sftp init: {e}")))?;
    let has_limits = version
        .extensions
        .get(extensions::LIMITS)
        .is_some_and(|v| v == "1");
    let mut max_read = DEFAULT_READ_CHUNK.min(max_packet.saturating_sub(9));
    if has_limits {
        let ext = session
            .limits()
            .await
            .map_err(|e| ConnError::Connect(format!("sftp limits: {e}")))?;
        let limits = Limits::from(ext);
        if let Some(r) = limits.read_len {
            if r > 0 {
                // Cap even when the server allows 1 MiB reads: large chunks
                // inflate reorder/write-queue peak and cross-thread free churn,
                // which is the post-download UI locality problem.
                max_read = (r as u32)
                    .min(max_packet.saturating_sub(9))
                    .min(DEFAULT_READ_CHUNK)
                    .max(32 * 1024);
            }
        }
        session.set_limits(limits);
    }
    Ok((Arc::new(session), max_read))
}

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
        if self.progress.as_ref().is_some_and(|p| p.is_cancelled()) {
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

fn sftp_err(ctx: &str, e: SftpError) -> ConnError {
    ConnError::Connect(format!("{ctx}: {e}"))
}

async fn raw_tree_bytes(raw: &RawSftpSession, remote_path: &str) -> Result<u64, ConnError> {
    let attrs = raw
        .lstat(remote_path)
        .await
        .map_err(|e| sftp_err(&format!("sftp lstat {remote_path}"), e))?
        .attrs;
    if attrs.file_type() == FileType::Symlink {
        return Ok(0);
    }
    if attrs.file_type().is_dir() {
        let mut total = 0u64;
        for (name, child_attrs) in raw_read_dir(raw, remote_path).await? {
            if child_attrs.file_type() == FileType::Symlink {
                continue;
            }
            let child = join_remote(remote_path, &name);
            if child_attrs.file_type().is_dir() {
                total += Box::pin(raw_tree_bytes(raw, &child)).await?;
            } else {
                total += child_attrs.size.unwrap_or(0);
            }
        }
        Ok(total)
    } else {
        Ok(attrs.size.unwrap_or(0))
    }
}

async fn raw_read_dir(
    raw: &RawSftpSession,
    remote_path: &str,
) -> Result<Vec<(String, FileAttributes)>, ConnError> {
    let handle = raw
        .opendir(remote_path)
        .await
        .map_err(|e| sftp_err(&format!("sftp opendir {remote_path}"), e))?
        .handle;
    let mut out = Vec::new();
    loop {
        match raw.readdir(handle.as_str()).await {
            Ok(name) => {
                for f in name.files {
                    if f.filename == "." || f.filename == ".." {
                        continue;
                    }
                    out.push((f.filename, f.attrs));
                }
            }
            Err(SftpError::Status(status)) if status.status_code == StatusCode::Eof => break,
            Err(e) => {
                let _ = raw.close(handle.as_str()).await;
                return Err(sftp_err(&format!("sftp readdir {remote_path}"), e));
            }
        }
    }
    let _ = raw.close(handle.as_str()).await;
    Ok(out)
}

async fn download_path_raw(
    raw: &Arc<RawSftpSession>,
    remote_path: &str,
    local_path: &Path,
    max_read: u32,
    ctx: &mut TransferCtx,
) -> Result<(), ConnError> {
    ctx.check_cancel()?;
    let attrs = raw
        .lstat(remote_path)
        .await
        .map_err(|e| sftp_err(&format!("sftp lstat {remote_path}"), e))?
        .attrs;
    if attrs.file_type() == FileType::Symlink {
        return Ok(());
    }
    if attrs.file_type().is_dir() {
        tokio::fs::create_dir_all(local_path)
            .await
            .map_err(ConnError::Io)?;
        for (name, _) in raw_read_dir(raw, remote_path).await? {
            let child_remote = join_remote(remote_path, &name);
            let child_local = local_path.join(&name);
            Box::pin(download_path_raw(
                raw,
                &child_remote,
                &child_local,
                max_read,
                ctx,
            ))
            .await?;
        }
        return Ok(());
    }
    download_file_pipelined(
        raw,
        remote_path,
        local_path,
        max_read,
        attrs.size,
        ctx,
    )
    .await
}

async fn download_file_pipelined(
    raw: &Arc<RawSftpSession>,
    remote_path: &str,
    local_path: &Path,
    max_read: u32,
    known_size: Option<u64>,
    ctx: &mut TransferCtx,
) -> Result<(), ConnError> {
    let handle = raw
        .open(
            remote_path,
            OpenFlags::READ,
            FileAttributes::default(),
        )
        .await
        .map_err(|e| sftp_err(&format!("sftp open {remote_path}"), e))?
        .handle;

    if let Some(parent) = local_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(ConnError::Io)?;
    }

    // Disk writes run on a dedicated blocking thread. Progress is counted here —
    // when bytes are committed to the OS write cache, not when they arrive from
    // the network — so the bar reflects real write progress instead of sprinting
    // ahead on buffered reads. We flush the BufWriter periodically (no fsync);
    // a single sync_data runs once at EOF before rename.
    let base = ctx.transferred;
    let total = ctx.total;
    let progress = ctx.progress.clone();
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(DOWNLOAD_WRITE_QUEUE);
    // Write to a sibling partial file, then rename into place. Truncating the
    // final path on 2nd+ overwrite often blocks for seconds under Windows
    // Defender while the previous download is still being scanned.
    let partial_path = download_partial_path(local_path);
    let write_path = partial_path.clone();
    let writer: JoinHandle<std::io::Result<u64>> = tokio::task::spawn_blocking(move || {
        use std::io::Write as _;
        let file = open_download_file(&write_path)?;
        let mut w = std::io::BufWriter::with_capacity(DOWNLOAD_WRITE_BUF, file);
        let mut written: u64 = 0;
        let mut since_flush: u64 = 0;
        while let Some(buf) = rx.blocking_recv() {
            w.write_all(&buf)?;
            written += buf.len() as u64;
            since_flush += buf.len() as u64;
            if let Some(p) = &progress {
                p.set(base.saturating_add(written), total);
            }
            if since_flush >= DOWNLOAD_FLUSH_INTERVAL {
                w.flush()?;
                since_flush = 0;
            }
        }
        w.flush()?;
        // Do not sync_data here: on Windows it can stall the whole process for
        // 1–2 s (VSTERM_DIAG SLOW FRAME / FRAME GAP). Durability is handled
        // after rename on a background thread.
        if let Some(p) = &progress {
            p.set(base.saturating_add(written), total);
        }
        Ok(written)
    });

    let chunk = max_read.max(32 * 1024);
    let mut next_offset = 0u64;
    let mut expect_offset = 0u64;
    let mut stop_issuing = known_size == Some(0);
    let mut pending = FuturesUnordered::new();
    let mut reorder: BTreeMap<u64, Vec<u8>> = BTreeMap::new();

    let issue = |raw: Arc<RawSftpSession>, handle: String, offset: u64, len: u32| async move {
        let result = raw.read(handle, offset, len).await;
        (offset, len, result)
    };

    let mut read_err: Option<ConnError> = None;
    'download: loop {
        if let Err(e) = ctx.check_cancel() {
            read_err = Some(e);
            break;
        }

        // Backpressure: stop issuing once reorder is full so a slow disk cannot
        // pull the remainder of the file into RAM.
        while pending.len() < DOWNLOAD_PIPELINE
            && !stop_issuing
            && reorder.len() < DOWNLOAD_REORDER_CAP
        {
            if let Some(sz) = known_size {
                if next_offset >= sz {
                    stop_issuing = true;
                    break;
                }
            }
            let offset = next_offset;
            let this_len = match known_size {
                Some(sz) => ((sz - offset) as u32).min(chunk).max(1),
                None => chunk,
            };
            next_offset = next_offset.saturating_add(this_len as u64);
            pending.push(issue(
                Arc::clone(raw),
                handle.clone(),
                offset,
                this_len,
            ));
        }

        // Push contiguous bytes to the writer (progress already counted on recv).
        while let Some(buf) = reorder.remove(&expect_offset) {
            let n = buf.len() as u64;
            match tx.try_send(buf) {
                Ok(()) => {
                    expect_offset = expect_offset.saturating_add(n);
                }
                Err(mpsc::error::TrySendError::Full(buf)) => {
                    reorder.insert(expect_offset, buf);
                    break;
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    read_err = Some(ConnError::Io(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "download disk writer stopped",
                    )));
                    break 'download;
                }
            }
        }

        if stop_issuing && pending.is_empty() && reorder.is_empty() {
            break;
        }

        let reorder_full = reorder.len() >= DOWNLOAD_REORDER_CAP;
        let need_flush = reorder.contains_key(&expect_offset);

        if need_flush && (tx.capacity() == 0 || reorder_full) {
            // Wait for writer space. Only accept more reads if reorder still has room.
            if reorder_full {
                match tx.reserve().await {
                    Ok(permit) => {
                        let buf = reorder.remove(&expect_offset).expect("checked");
                        let n = buf.len() as u64;
                        permit.send(buf);
                        expect_offset = expect_offset.saturating_add(n);
                    }
                    Err(_) => {
                        read_err = Some(ConnError::Io(std::io::Error::new(
                            std::io::ErrorKind::BrokenPipe,
                            "download disk writer stopped",
                        )));
                        break;
                    }
                }
            } else {
                tokio::select! {
                    permit = tx.reserve() => {
                        match permit {
                            Ok(permit) => {
                                let buf = reorder.remove(&expect_offset).expect("checked");
                                let n = buf.len() as u64;
                                permit.send(buf);
                                expect_offset = expect_offset.saturating_add(n);
                            }
                            Err(_) => {
                                read_err = Some(ConnError::Io(std::io::Error::new(
                                    std::io::ErrorKind::BrokenPipe,
                                    "download disk writer stopped",
                                )));
                                break;
                            }
                        }
                    }
                    Some((offset, req_len, result)) = pending.next() => {
                        match handle_read_result(
                            offset,
                            req_len,
                            result,
                            chunk,
                            known_size,
                            &mut reorder,
                            &mut stop_issuing,
                            remote_path,
                        ) {
                            Ok(_) => {}
                            Err(e) => {
                                read_err = Some(e);
                                break;
                            }
                        }
                    }
                }
            }
            continue;
        }

        let Some((offset, req_len, result)) = pending.next().await else {
            break;
        };
        match handle_read_result(
            offset,
            req_len,
            result,
            chunk,
            known_size,
            &mut reorder,
            &mut stop_issuing,
            remote_path,
        ) {
            Ok(_) => {}
            Err(e) => {
                read_err = Some(e);
                break;
            }
        }
    }

    // Consume leftover in-flight reads before tearing down the writer. Dropping
    // the FuturesUnordered alone leaves oneshot receivers gone while russh still
    // holds CHANNEL_DATA CryptoVecs in the channel mpsc until the SFTP reader
    // delivers them — under load that can pin several MiB past transfer end.
    while pending.next().await.is_some() {}

    // Drop sender so the writer can finish (EOF on rx).
    drop(tx);
    let write_result = writer
        .await
        .map_err(|e| ConnError::Backend(format!("disk writer task: {e}")))?;
    let _ = raw.close(handle.as_str()).await;

    if let Some(e) = read_err {
        let _ = std::fs::remove_file(&partial_path);
        return Err(e);
    }
    let written = match write_result {
        Ok(n) => n,
        Err(e) => {
            let _ = std::fs::remove_file(&partial_path);
            return Err(ConnError::Io(e));
        }
    };
    if let Err(e) = promote_download_partial(&partial_path, local_path) {
        let _ = std::fs::remove_file(&partial_path);
        return Err(ConnError::Io(e));
    }
    // Best-effort durability off the download/UI path — never block rename or
    // finish_ok on FlushFileBuffers.
    let sync_path = local_path.to_path_buf();
    let _ = std::thread::Builder::new()
        .name("vsterm-dl-sync".into())
        .spawn(move || {
            if let Ok(f) = std::fs::OpenOptions::new().write(true).open(&sync_path) {
                let _ = f.sync_data();
            }
        });
    // Advance the running total so the next file in a tree download resumes from
    // the correct base (the writer drives the live bar during the transfer).
    ctx.transferred = base.saturating_add(written);

    if let Some(sz) = known_size {
        if written != sz {
            return Err(ConnError::Connect(format!(
                "sftp download size mismatch for {remote_path}: got {written} bytes, expected {sz}"
            )));
        }
    }
    Ok(())
}

fn handle_read_result(
    offset: u64,
    req_len: u32,
    result: Result<russh_sftp::protocol::Data, SftpError>,
    chunk: u32,
    known_size: Option<u64>,
    reorder: &mut BTreeMap<u64, Vec<u8>>,
    stop_issuing: &mut bool,
    remote_path: &str,
) -> Result<u64, ConnError> {
    match result {
        Ok(data) => {
            let bytes = data.data;
            let n = bytes.len() as u32;
            let counted = n as u64;
            if n > 0 {
                reorder.insert(offset, bytes);
            }
            if n == 0 {
                *stop_issuing = true;
            } else if n < req_len {
                // Short read: EOF for unknown size, or a hole if size was known.
                if let Some(sz) = known_size {
                    if offset.saturating_add(n as u64) < sz {
                        return Err(ConnError::Connect(format!(
                            "sftp short read for {remote_path} at offset {offset}: got {n}, wanted {req_len}"
                        )));
                    }
                }
                *stop_issuing = true;
            } else if known_size.is_none() && n < chunk {
                *stop_issuing = true;
            }
            Ok(counted)
        }
        Err(SftpError::Status(status)) if status.status_code == StatusCode::Eof => {
            *stop_issuing = true;
            Ok(0)
        }
        Err(e) => Err(sftp_err(&format!("sftp read {remote_path}"), e)),
    }
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

pub async fn run_download_raw(
    raw: &Arc<RawSftpSession>,
    max_read: u32,
    remote_path: &str,
    local_path: &Path,
    progress: Option<&ArcProgress>,
) -> Result<(), ConnError> {
    let total = raw_tree_bytes(raw, remote_path).await.ok();
    let mut ctx = TransferCtx::new(progress.cloned(), total);
    download_path_raw(raw, remote_path, local_path, max_read, &mut ctx).await
}

pub async fn list_dir_raw(
    raw: &RawSftpSession,
    path: &str,
) -> Result<Vec<RemoteDirEntry>, ConnError> {
    let mut entries = Vec::new();
    for (name, attrs) in raw_read_dir(raw, path).await? {
        let is_dir = attrs.file_type().is_dir();
        entries.push(RemoteDirEntry {
            name,
            is_dir,
            size: attrs.size,
            mtime: attrs.mtime.map(|t| t as u64),
        });
    }
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(entries)
}

pub async fn write_file_raw(
    raw: &RawSftpSession,
    remote_path: &str,
    data: &[u8],
) -> Result<(), ConnError> {
    let handle = raw
        .open(
            remote_path,
            OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
            FileAttributes::default(),
        )
        .await
        .map_err(|e| sftp_err(&format!("sftp create {remote_path}"), e))?
        .handle;
    if !data.is_empty() {
        let mut offset = 0u64;
        while offset < data.len() as u64 {
            let end = ((offset as usize) + UPLOAD_BUF).min(data.len());
            let chunk = data[offset as usize..end].to_vec();
            let n = chunk.len() as u64;
            raw.write(handle.as_str(), offset, chunk)
                .await
                .map_err(|e| sftp_err(&format!("sftp write {remote_path}"), e))?;
            offset += n;
        }
    }
    raw.close(handle.as_str())
        .await
        .map_err(|e| sftp_err(&format!("sftp close {remote_path}"), e))?;
    Ok(())
}

pub async fn run_upload_raw(
    raw: &RawSftpSession,
    local_path: &Path,
    remote_path: &str,
    progress: Option<&ArcProgress>,
) -> Result<(), ConnError> {
    let total = local_tree_bytes(local_path).ok();
    let mut ctx = TransferCtx::new(progress.cloned(), total);
    upload_path_raw(raw, local_path, remote_path, &mut ctx).await
}

async fn upload_path_raw(
    raw: &RawSftpSession,
    local_path: &Path,
    remote_path: &str,
    ctx: &mut TransferCtx,
) -> Result<(), ConnError> {
    ctx.check_cancel()?;
    let meta = tokio::fs::symlink_metadata(local_path)
        .await
        .map_err(ConnError::Io)?;
    if meta.file_type().is_symlink() {
        return Ok(());
    }
    if meta.is_dir() {
        if raw.stat(remote_path).await.is_err() {
            raw.mkdir(remote_path, FileAttributes::default())
                .await
                .map_err(|e| sftp_err(&format!("sftp mkdir {remote_path}"), e))?;
        }
        let mut rd = tokio::fs::read_dir(local_path)
            .await
            .map_err(ConnError::Io)?;
        while let Some(entry) = rd.next_entry().await.map_err(ConnError::Io)? {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let child_local = entry.path();
            let child_remote = join_remote(remote_path, &name);
            Box::pin(upload_path_raw(raw, &child_local, &child_remote, ctx)).await?;
        }
        return Ok(());
    }
    upload_file_raw(raw, local_path, remote_path, ctx).await
}

async fn upload_file_raw(
    raw: &RawSftpSession,
    local_path: &Path,
    remote_path: &str,
    ctx: &mut TransferCtx,
) -> Result<(), ConnError> {
    let mut local = tokio::fs::File::open(local_path)
        .await
        .map_err(ConnError::Io)?;
    let handle = raw
        .open(
            remote_path,
            OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
            FileAttributes::default(),
        )
        .await
        .map_err(|e| sftp_err(&format!("sftp create {remote_path}"), e))?
        .handle;
    let mut buf = vec![0u8; UPLOAD_BUF];
    let mut offset = 0u64;
    loop {
        ctx.check_cancel()?;
        let n = local.read(&mut buf).await.map_err(ConnError::Io)?;
        if n == 0 {
            break;
        }
        raw.write(handle.as_str(), offset, buf[..n].to_vec())
            .await
            .map_err(|e| ConnError::Connect(format!("sftp write {remote_path}: {e}")))?;
        offset += n as u64;
        ctx.add_bytes(n as u64);
    }
    raw.close(handle.as_str())
        .await
        .map_err(|e| sftp_err(&format!("sftp close {remote_path}"), e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_tree_bytes_missing_is_error() {
        assert!(local_tree_bytes(Path::new("/nonexistent-vsterm-path-xyz")).is_err());
    }
}
