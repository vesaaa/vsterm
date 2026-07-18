//! ZMODEM (rz/sz) support over the interactive PTY/SSH shell channel.
//!
//! Remote `sz` → local receive (download). Remote `rz` → local send (upload).
//!
//! **Dialog pause:** after the remote filename is known (`sz`) or ZRINIT is
//! seen (`rz`), the protocol stalls until the UI confirms a path / file list.
//! We do not ACK / send file data while a dialog is open — that avoids the
//! "already finished before OK" race and the post-confirm freeze.

use parking_lot::Mutex;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use zmodem2::{Action, Event, FileInfo, Position, Receiver, Sender};

/// Result of feeding remote bytes through the ZMODEM gate.
#[derive(Debug, Default)]
pub struct RxResult {
    pub to_terminal: Vec<u8>,
    pub to_wire: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum ZmodemStatus {
    Idle,
    /// Remote `sz`: filename known, waiting for Save As.
    AwaitingSaveAs {
        suggested_name: String,
        total: Option<u64>,
    },
    Receiving {
        file_name: String,
        bytes: u64,
        total: Option<u64>,
    },
    /// Remote `rz`: waiting for local file picker.
    AwaitingUpload,
    Sending {
        file_name: String,
        bytes: u64,
        total: Option<u64>,
    },
    Done {
        summary: String,
    },
    Failed {
        message: String,
    },
}

impl ZmodemStatus {
    /// Progress fraction in `0.0..=1.0` when total size is known.
    pub fn progress_fraction(&self) -> Option<f32> {
        match self {
            Self::Receiving {
                bytes, total: Some(t), ..
            }
            | Self::Sending {
                bytes, total: Some(t), ..
            } if *t > 0 => Some((*bytes as f32 / *t as f32).clamp(0.0, 1.0)),
            Self::AwaitingSaveAs {
                total: Some(t), ..
            } if *t > 0 => Some(0.0),
            _ => None,
        }
    }
}

struct RecvState {
    engine: Receiver,
    file: Option<File>,
    path: Option<PathBuf>,
    name: String,
    written: u64,
    total: Option<u64>,
    inbox: Vec<u8>,
    /// True until Save As confirms a destination path.
    awaiting_save: bool,
}

struct SendState {
    engine: Sender,
    files: Vec<PathBuf>,
    index: usize,
    file: Option<File>,
    name: String,
    sent: u64,
    total: Option<u64>,
    inbox: Vec<u8>,
    offered: bool,
}

enum Stage {
    Idle { pending: Vec<u8> },
    Recv(RecvState),
    AwaitSend { zrinit: Vec<u8> },
    Send(SendState),
}

pub struct ZmodemBridge {
    inner: Mutex<Inner>,
}

struct Inner {
    stage: Stage,
    status: ZmodemStatus,
    downloads: PathBuf,
}

impl ZmodemBridge {
    pub fn new() -> Self {
        let downloads = dirs::download_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            inner: Mutex::new(Inner {
                stage: Stage::Idle {
                    pending: Vec::new(),
                },
                status: ZmodemStatus::Idle,
                downloads,
            }),
        }
    }

    pub fn status(&self) -> ZmodemStatus {
        self.inner.lock().status.clone()
    }

    pub fn is_transferring(&self) -> bool {
        !matches!(
            self.inner.lock().status,
            ZmodemStatus::Idle | ZmodemStatus::Done { .. } | ZmodemStatus::Failed { .. }
        )
    }

    pub fn clear_finished_status(&self) {
        let mut g = self.inner.lock();
        if matches!(
            g.status,
            ZmodemStatus::Done { .. } | ZmodemStatus::Failed { .. }
        ) {
            g.status = ZmodemStatus::Idle;
        }
    }

    pub fn cancel_bytes() -> &'static [u8] {
        &[0x18, 0x18, 0x18, 0x18, 0x18]
    }

    pub fn cancel(&self) -> Vec<u8> {
        let mut g = self.inner.lock();
        if let Stage::Recv(recv) = &mut g.stage {
            drop(recv.file.take());
            // Remove a partial download if we already opened the Save As path.
            if let Some(path) = recv.path.take() {
                let _ = fs::remove_file(path);
            }
        }
        g.stage = Stage::Idle {
            pending: Vec::new(),
        };
        g.status = ZmodemStatus::Failed {
            message: "ZMODEM transfer cancelled".into(),
        };
        Self::cancel_bytes().to_vec()
    }

    pub fn default_download_dir(&self) -> PathBuf {
        self.inner.lock().downloads.clone()
    }

    /// Confirm Save As for remote `sz`. `None` cancels.
    ///
    /// Until this returns Ok with a path, the receiver does **not** ACK the
    /// file (ZRPOS stays queued) so the remote cannot finish early.
    pub fn provide_download_path(&self, path: Option<PathBuf>) -> Result<Vec<u8>, String> {
        let mut g = self.inner.lock();
        let Stage::Recv(recv) = &mut g.stage else {
            return Err("no pending ZMODEM download".into());
        };
        if !recv.awaiting_save {
            return Err("no pending ZMODEM Save As".into());
        }

        if path.is_none() {
            g.stage = Stage::Idle {
                pending: Vec::new(),
            };
            g.status = ZmodemStatus::Failed {
                message: "ZMODEM download cancelled".into(),
            };
            return Ok(Self::cancel_bytes().to_vec());
        }

        let dest = path.unwrap();
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("mkdir {parent:?}: {e}"))?;
        }
        let f = File::create(&dest).map_err(|e| format!("create {dest:?}: {e}"))?;
        recv.file = Some(f);
        recv.path = Some(dest.clone());
        recv.awaiting_save = false;
        g.status = ZmodemStatus::Receiving {
            file_name: recv.name.clone(),
            bytes: 0,
            total: recv.total,
        };
        tracing::info!("ZMODEM receiving → {}", dest.display());

        let mut out = Vec::new();
        run_recv(&mut g, &[], &mut out)?;
        Ok(out)
    }

    /// Start upload for remote `rz`. Empty `paths` cancels.
    pub fn provide_upload_files(&self, paths: Vec<PathBuf>) -> Result<Vec<u8>, String> {
        let mut g = self.inner.lock();
        let Stage::AwaitSend { zrinit } = &g.stage else {
            return Err("no pending ZMODEM upload".into());
        };
        if paths.is_empty() {
            g.stage = Stage::Idle {
                pending: Vec::new(),
            };
            g.status = ZmodemStatus::Failed {
                message: "ZMODEM upload cancelled".into(),
            };
            return Ok(Self::cancel_bytes().to_vec());
        }
        let zrinit = zrinit.clone();
        let mut engine = Sender::new().map_err(|e| format!("zmodem sender: {e:?}"))?;
        let _ = engine.submit_wire(&zrinit);
        g.stage = Stage::Send(SendState {
            engine,
            files: paths,
            index: 0,
            file: None,
            name: String::new(),
            sent: 0,
            total: None,
            inbox: Vec::new(),
            offered: false,
        });
        g.status = ZmodemStatus::Sending {
            file_name: String::new(),
            bytes: 0,
            total: None,
        };
        let mut out = Vec::new();
        run_send(&mut g, &[], &mut out)?;
        Ok(out)
    }

    pub fn on_rx(&self, data: &[u8]) -> RxResult {
        let mut g = self.inner.lock();
        let mut to_wire = Vec::new();
        let to_terminal = match &mut g.stage {
            Stage::Idle { pending } => {
                pending.extend_from_slice(data);
                match classify_pending(pending) {
                    Detect::NeedMore => {
                        let hold = ambiguous_suffix_len(pending);
                        if pending.len() > hold {
                            let flush_len = pending.len() - hold;
                            pending.drain(..flush_len).collect()
                        } else {
                            Vec::new()
                        }
                    }
                    Detect::None => std::mem::take(pending),
                    Detect::RemoteSend(prefix_len) => {
                        let text = pending[..prefix_len].to_vec();
                        let frame = pending[prefix_len..].to_vec();
                        pending.clear();
                        let downloads = g.downloads.clone();
                        match start_recv(frame, &downloads, &mut to_wire) {
                            Ok((stage, status)) => {
                                g.stage = stage;
                                g.status = status;
                            }
                            Err(msg) => {
                                g.stage = Stage::Idle {
                                    pending: Vec::new(),
                                };
                                g.status = ZmodemStatus::Failed { message: msg };
                            }
                        }
                        text
                    }
                    Detect::RemoteRecv(prefix_len) => {
                        let text = pending[..prefix_len].to_vec();
                        let frame = pending[prefix_len..].to_vec();
                        pending.clear();
                        // Stall: do not create a Sender until the UI picks files.
                        g.stage = Stage::AwaitSend { zrinit: frame };
                        g.status = ZmodemStatus::AwaitingUpload;
                        text
                    }
                }
            }
            Stage::Recv(_) | Stage::Send(_) => {
                if let Err(msg) = match &g.stage {
                    Stage::Recv(_) => run_recv(&mut g, data, &mut to_wire),
                    Stage::Send(_) => run_send(&mut g, data, &mut to_wire),
                    _ => Ok(()),
                } {
                    g.status = ZmodemStatus::Failed { message: msg };
                    g.stage = Stage::Idle {
                        pending: Vec::new(),
                    };
                    to_wire.extend_from_slice(Self::cancel_bytes());
                }
                Vec::new()
            }
            Stage::AwaitSend { zrinit } => {
                // Keep buffering remote bytes; still no Sender until UI confirms.
                zrinit.extend_from_slice(data);
                Vec::new()
            }
        };
        RxResult {
            to_terminal,
            to_wire,
        }
    }
}

impl Default for ZmodemBridge {
    fn default() -> Self {
        Self::new()
    }
}

enum Detect {
    NeedMore,
    None,
    RemoteSend(usize),
    RemoteRecv(usize),
}

fn classify_pending(buf: &[u8]) -> Detect {
    if let Some(i) = find_subslice(buf, &[b'*', b'*', 0x18, b'B']) {
        if buf.len() < i + 6 {
            return Detect::NeedMore;
        }
        let t0 = buf[i + 4];
        let t1 = buf[i + 5];
        return match (t0, t1) {
            (b'0', b'0') => Detect::RemoteSend(i),
            (b'0', b'1') => Detect::RemoteRecv(i),
            _ => Detect::RemoteSend(i),
        };
    }
    if let Some(i) = find_zbin_start(buf) {
        if buf.len() < i + 3 {
            return Detect::NeedMore;
        }
        return Detect::RemoteSend(i);
    }
    if ambiguous_suffix_len(buf) > 0 {
        return Detect::NeedMore;
    }
    Detect::None
}

fn ambiguous_suffix_len(buf: &[u8]) -> usize {
    if buf.is_empty() {
        return 0;
    }
    if buf.ends_with(&[b'*', b'*', 0x18, b'B']) {
        return 4;
    }
    if buf.ends_with(&[b'*', b'*', 0x18]) {
        return 3;
    }
    if buf.ends_with(b"**") {
        return 2;
    }
    if buf.ends_with(b"*") {
        return 1;
    }
    if buf.last() == Some(&0x18) {
        return 1;
    }
    0
}

fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn find_zbin_start(buf: &[u8]) -> Option<usize> {
    for i in 0..buf.len().saturating_sub(1) {
        if buf[i] == b'*' && buf[i + 1] == 0x18 {
            match buf.get(i + 2) {
                None => return None,
                Some(&enc) if enc == b'A' || enc == b'B' || enc == b'C' => return Some(i),
                Some(_) => {}
            }
        }
    }
    None
}

fn start_recv(
    frame: Vec<u8>,
    downloads: &Path,
    to_wire: &mut Vec<u8>,
) -> Result<(Stage, ZmodemStatus), String> {
    let engine = Receiver::new().map_err(|e| format!("zmodem receiver: {e:?}"))?;
    let mut inner = Inner {
        stage: Stage::Recv(RecvState {
            engine,
            file: None,
            path: None,
            name: String::new(),
            written: 0,
            total: None,
            inbox: frame,
            awaiting_save: false,
        }),
        status: ZmodemStatus::Receiving {
            file_name: String::new(),
            bytes: 0,
            total: None,
        },
        downloads: downloads.to_path_buf(),
    };
    run_recv(&mut inner, &[], to_wire)?;
    Ok((inner.stage, inner.status))
}

fn run_recv(inner: &mut Inner, data: &[u8], to_wire: &mut Vec<u8>) -> Result<(), String> {
    let Stage::Recv(recv) = &mut inner.stage else {
        return Ok(());
    };
    if !data.is_empty() {
        recv.inbox.extend_from_slice(data);
    }

    // Paused for Save As: keep wire bytes in inbox / engine, but do not ACK
    // (WriteWire) or accept file data until a path is chosen.
    if recv.awaiting_save && recv.file.is_none() {
        return Ok(());
    }

    loop {
        // Re-check pause (e.g. multi-file next Save As).
        let Stage::Recv(recv) = &mut inner.stage else {
            return Ok(());
        };
        if recv.awaiting_save && recv.file.is_none() {
            return Ok(());
        }

        if !recv.inbox.is_empty() {
            let n = recv
                .engine
                .submit_wire(&recv.inbox)
                .map_err(|e| format!("zmodem rx: {e:?}"))?;
            let n = n.min(recv.inbox.len());
            recv.inbox.drain(..n);
        }
        match recv.engine.poll() {
            Action::WriteWire(bytes) => {
                to_wire.extend_from_slice(bytes);
                let n = bytes.len();
                recv.engine.wire_written(n);
            }
            Action::WriteFile(bytes) => {
                let owned = bytes.to_vec();
                let f = recv
                    .file
                    .as_mut()
                    .ok_or_else(|| "zmodem data before Save As".to_string())?;
                f.write_all(&owned)
                    .map_err(|e| format!("write download: {e}"))?;
                recv.written += owned.len() as u64;
                recv.engine
                    .file_written(owned.len())
                    .map_err(|e| format!("zmodem file_written: {e:?}"))?;
                inner.status = ZmodemStatus::Receiving {
                    file_name: recv.name.clone(),
                    bytes: recv.written,
                    total: recv.total,
                };
            }
            Action::Event(Event::FileStarted(info)) => {
                let safe = sanitize_name(info.name);
                let total = info.size.map(|p| u64::from(p.get()));
                recv.name = safe.clone();
                recv.written = 0;
                recv.total = total;
                recv.path = None;
                drop(recv.file.take());
                recv.awaiting_save = true;
                inner.status = ZmodemStatus::AwaitingSaveAs {
                    suggested_name: safe,
                    total,
                };
                // Leave ZRPOS queued inside the engine until Save As confirms.
                return Ok(());
            }
            Action::Event(Event::FileCompleted) => {
                if let Some(f) = recv.file.take() {
                    let _ = f.sync_all();
                }
            }
            Action::Event(Event::SessionCompleted) => {
                let summary = recv
                    .path
                    .as_ref()
                    .map(|p| format!("ZMODEM saved {}", p.display()))
                    .unwrap_or_else(|| "ZMODEM receive complete".into());
                inner.status = ZmodemStatus::Done { summary };
                inner.stage = Stage::Idle {
                    pending: Vec::new(),
                };
                return Ok(());
            }
            Action::Event(Event::Aborted) => {
                return Err("ZMODEM receive aborted".into());
            }
            Action::Event(_) => {}
            Action::ReadFile { .. } => {
                return Err("unexpected ReadFile on receiver".into());
            }
            Action::Idle => {
                if recv.inbox.is_empty() {
                    break;
                }
            }
            _ => break,
        }
    }
    Ok(())
}

fn run_send(inner: &mut Inner, data: &[u8], to_wire: &mut Vec<u8>) -> Result<(), String> {
    {
        let Stage::Send(send) = &mut inner.stage else {
            return Ok(());
        };
        if !data.is_empty() {
            send.inbox.extend_from_slice(data);
        }
        try_offer_file(send, &mut inner.status)?;
    }

    // Bound work per call so the UI/reader never holds the mutex while
    // streaming an entire multi‑MB file in one go.
    const MAX_ACTIONS: usize = 64;
    for _ in 0..MAX_ACTIONS {
        let Stage::Send(send) = &mut inner.stage else {
            return Ok(());
        };
        if !send.inbox.is_empty() {
            let n = send
                .engine
                .submit_wire(&send.inbox)
                .map_err(|e| format!("zmodem tx: {e:?}"))?;
            let n = n.min(send.inbox.len());
            send.inbox.drain(..n);
        }
        match send.engine.poll() {
            Action::WriteWire(bytes) => {
                to_wire.extend_from_slice(bytes);
                let n = bytes.len();
                send.engine.wire_written(n);
            }
            Action::ReadFile { offset, max_len } => {
                let f = send
                    .file
                    .as_mut()
                    .ok_or_else(|| "zmodem read without open file".to_string())?;
                f.seek(SeekFrom::Start(u64::from(offset.get())))
                    .map_err(|e| format!("seek: {e}"))?;
                let mut buf = vec![0u8; max_len];
                let n = f.read(&mut buf).map_err(|e| format!("read: {e}"))?;
                send.engine
                    .submit_file(&buf[..n])
                    .map_err(|e| format!("submit_file: {e:?}"))?;
                send.sent = u64::from(offset.get()) + n as u64;
                inner.status = ZmodemStatus::Sending {
                    file_name: send.name.clone(),
                    bytes: send.sent,
                    total: send.total,
                };
            }
            Action::Event(Event::FileCompleted) => {
                send.file = None;
                send.offered = false;
                send.index += 1;
                if send.index >= send.files.len() {
                    send.engine.finish().map_err(|e| format!("finish: {e:?}"))?;
                } else {
                    try_offer_file(send, &mut inner.status)?;
                }
            }
            Action::Event(Event::SessionCompleted) => {
                inner.status = ZmodemStatus::Done {
                    summary: "ZMODEM upload complete".into(),
                };
                inner.stage = Stage::Idle {
                    pending: Vec::new(),
                };
                return Ok(());
            }
            Action::Event(Event::FileStarted(_)) => {}
            Action::Event(Event::Aborted) => return Err("ZMODEM send aborted".into()),
            Action::Event(_) => {}
            Action::WriteFile(_) => return Err("unexpected WriteFile on sender".into()),
            Action::Idle => {
                if send.inbox.is_empty() {
                    try_offer_file(send, &mut inner.status)?;
                    break;
                }
            }
            _ => break,
        }
    }
    Ok(())
}

fn try_offer_file(send: &mut SendState, status: &mut ZmodemStatus) -> Result<(), String> {
    if send.offered || send.index >= send.files.len() {
        return Ok(());
    }
    let path = &send.files[send.index];
    let meta = fs::metadata(path).map_err(|e| format!("stat {path:?}: {e}"))?;
    let fname = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .as_bytes();
    let size = Position::new(meta.len().min(u64::from(u32::MAX)) as u32);
    match send.engine.start_file(FileInfo::new(fname, Some(size))) {
        Ok(()) => {
            send.file = Some(File::open(path).map_err(|e| format!("open {path:?}: {e}"))?);
            send.name = String::from_utf8_lossy(fname).into_owned();
            send.sent = 0;
            send.total = Some(meta.len());
            send.offered = true;
            *status = ZmodemStatus::Sending {
                file_name: send.name.clone(),
                bytes: 0,
                total: send.total,
            };
        }
        Err(zmodem2::Error::InvalidState) => {}
        Err(e) => return Err(format!("start_file: {e:?}")),
    }
    Ok(())
}

fn sanitize_name(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    let base = Path::new(s.as_ref())
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download");
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        "download".into()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_zrqinit_hex() {
        let buf = b"**\x18B00".to_vec();
        assert!(matches!(classify_pending(&buf), Detect::RemoteSend(0)));
        let buf = b"hello**\x18B00xx".to_vec();
        assert!(matches!(classify_pending(&buf), Detect::RemoteSend(5)));
    }

    #[test]
    fn detects_zrinit_hex() {
        let buf = b"**\x18B01".to_vec();
        assert!(matches!(classify_pending(&buf), Detect::RemoteRecv(0)));
    }

    #[test]
    fn need_more_only_holds_suffix() {
        let mut pending = b"prompt*".to_vec();
        assert!(matches!(classify_pending(&pending), Detect::NeedMore));
        assert_eq!(ambiguous_suffix_len(&pending), 1);
        let hold = ambiguous_suffix_len(&pending);
        let flush: Vec<u8> = pending.drain(..pending.len() - hold).collect();
        assert_eq!(flush, b"prompt");
        assert_eq!(pending, b"*");
    }

    #[test]
    fn sanitize_strips_path() {
        assert_eq!(sanitize_name(b"../../etc/passwd"), "passwd");
        assert_eq!(sanitize_name(b"ok_file.txt"), "ok_file.txt");
    }

    #[test]
    fn progress_fraction_known_total() {
        let s = ZmodemStatus::Receiving {
            file_name: "a".into(),
            bytes: 50,
            total: Some(100),
        };
        assert!((s.progress_fraction().unwrap() - 0.5).abs() < f32::EPSILON);
    }
}
