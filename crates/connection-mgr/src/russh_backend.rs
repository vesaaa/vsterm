//! Builtin SSH engine based on [`russh`].
//!
//! Interactive shell and one-shot [`RemoteExec`] share one authenticated
//! [`client::Handle`] living on a process-global multi-thread Tokio runtime
//! (so the UI connect thread can return without dropping the session).

use crate::backend::RemoteExec;
use crate::known_hosts::{self, HostKeyCheck};
use crate::remote_exec::RemoteSession;
use crate::remote_fs::{ArcProgress, RemoteDirEntry, RemoteFs};
use crate::remote_tree;
use crate::ssh_io::SshIoSession;
use crate::system_ssh::{expand_user_path, preflight, resolve_auth, AuthMaterial, PreflightOpts};
use crate::ConnError;
use parking_lot::Mutex as ParkingMutex;
use russh::client::{AuthResult, Handle, KeyboardInteractiveAuthResponse};
use russh::keys::{load_secret_key, PrivateKeyWithHashAlg, PublicKey};
use russh::MethodKind;
use russh::{client, ChannelMsg};
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::FileType;
use session_tree::{AuthConfig, SessionConfig};
use std::io::{self, Cursor, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::mpsc as tmpsc;
use vault::Vault;

/// Result of a successful builtin connect.
pub struct RusshEstablished {
    pub io: SshIoSession,
    pub remote: RemoteSession,
}

pub struct RusshBackend;

impl RusshBackend {
    pub fn new() -> Self {
        Self
    }

    pub fn is_available() -> bool {
        true
    }

    /// Authenticate and open an interactive shell + shared [`RemoteExec`].
    pub async fn open_interactive(
        config: &SessionConfig,
        vault: Option<&Vault>,
        interactive_password: Option<String>,
        cols: u16,
        rows: u16,
    ) -> Result<RusshEstablished, ConnError> {
        preflight(
            config,
            vault,
            PreflightOpts::connecting(interactive_password.is_some()),
        )?;
        let auth = resolve_auth(config, vault, interactive_password)?;

        // Run the whole connect + shell bootstrap on the dedicated runtime so
        // channels keep alive after the UI's current-thread runtime is gone.
        let cfg = config.clone();
        let auth = auth.clone();
        tokio::task::spawn_blocking(move || {
            runtime().block_on(connect_and_shell(cfg, auth, cols, rows))
        })
        .await
        .map_err(|e| ConnError::Backend(format!("russh worker join: {e}")))?
    }
}

impl Default for RusshBackend {
    fn default() -> Self {
        Self::new()
    }
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .thread_name("vsterm-russh")
            .build()
            .expect("vsterm russh runtime")
    })
}

async fn connect_and_shell(
    config: SessionConfig,
    auth: AuthMaterial,
    cols: u16,
    rows: u16,
) -> Result<RusshEstablished, ConnError> {
    let host_key_err: Arc<ParkingMutex<Option<ConnError>>> = Arc::new(ParkingMutex::new(None));
    let handler = ClientHandler {
        host: config.host.clone(),
        port: config.port,
        host_key_err: Arc::clone(&host_key_err),
    };

    let conf = Arc::new(client::Config {
        inactivity_timeout: None,
        keepalive_interval: Some(Duration::from_secs(30)),
        ..Default::default()
    });

    let mut handle = client::connect(conf, (config.host.as_str(), config.port), handler)
        .await
        .map_err(|e| {
            if let Some(err) = host_key_err.lock().take() {
                return err;
            }
            ConnError::Connect(format!("{e}"))
        })?;

    authenticate(&mut handle, &config, &auth).await?;

    let session = Arc::new(handle);
    let shared = Arc::new(RusshRemoteExec {
        session: Arc::clone(&session),
        host: config.host.clone(),
    });
    let remote = RemoteSession::from_exec_fs(
        config.username.clone(),
        config.host.clone(),
        shared.clone() as Arc<dyn RemoteExec>,
        shared as Arc<dyn RemoteFs>,
    );

    let io = open_shell_io(Arc::clone(&session), &config.term_type, cols, rows).await?;
    Ok(RusshEstablished { io, remote })
}

struct ClientHandler {
    host: String,
    port: u16,
    host_key_err: Arc<ParkingMutex<Option<ConnError>>>,
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        match known_hosts::check(&self.host, self.port, server_public_key) {
            Ok(HostKeyCheck::Match) => Ok(true),
            Ok(HostKeyCheck::Unknown) => {
                if let Err(e) = known_hosts::learn(&self.host, self.port, server_public_key) {
                    *self.host_key_err.lock() = Some(e);
                    return Ok(false);
                }
                tracing::info!(
                    host = %self.host,
                    port = self.port,
                    fingerprint = %known_hosts::fingerprint_sha256(server_public_key),
                    "accepted new host key (trust on first use)"
                );
                Ok(true)
            }
            Ok(HostKeyCheck::Mismatch) => {
                *self.host_key_err.lock() = Some(ConnError::HostKeyMismatch(format!(
                    "{}:{} fingerprint {}",
                    self.host,
                    self.port,
                    known_hosts::fingerprint_sha256(server_public_key)
                )));
                Ok(false)
            }
            Err(e) => {
                *self.host_key_err.lock() = Some(e);
                Ok(false)
            }
        }
    }
}

async fn authenticate(
    handle: &mut Handle<ClientHandler>,
    config: &SessionConfig,
    auth: &AuthMaterial,
) -> Result<(), ConnError> {
    let user = config.username.as_str();
    match &config.auth {
        AuthConfig::Password { .. } => {
            let password = auth
                .password
                .as_deref()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| ConnError::AuthFailed("password required".into()))?;
            auth_with_password(handle, user, password).await
        }
        AuthConfig::Publickey {
            private_key_path, ..
        } => {
            let path = expand_user_path(private_key_path);
            let passphrase = auth.passphrase.as_deref();
            let key = load_secret_key(&path, passphrase).map_err(|e| {
                let msg = e.to_string();
                if msg.to_ascii_lowercase().contains("password")
                    || msg.to_ascii_lowercase().contains("passphrase")
                    || msg.to_ascii_lowercase().contains("encrypted")
                {
                    ConnError::AuthFailed(format!("private key passphrase: {e}"))
                } else {
                    ConnError::AuthFailed(format!("load private key {}: {e}", path.display()))
                }
            })?;
            let hash = handle
                .best_supported_rsa_hash()
                .await
                .map_err(|e| ConnError::Connect(format!("{e}")))?
                .flatten();
            let key = PrivateKeyWithHashAlg::new(Arc::new(key), hash);
            match handle
                .authenticate_publickey(user, key)
                .await
                .map_err(|e| ConnError::Connect(format!("{e}")))?
            {
                AuthResult::Success => Ok(()),
                AuthResult::Failure { .. } => {
                    Err(ConnError::AuthFailed("publickey authentication failed".into()))
                }
            }
        }
    }
}

async fn auth_with_password(
    handle: &mut Handle<ClientHandler>,
    user: &str,
    password: &str,
) -> Result<(), ConnError> {
    match handle
        .authenticate_password(user, password)
        .await
        .map_err(|e| ConnError::Connect(format!("{e}")))?
    {
        AuthResult::Success => return Ok(()),
        AuthResult::Failure { remaining_methods } => {
            if !remaining_methods
                .iter()
                .any(|m| *m == MethodKind::KeyboardInteractive)
            {
                return Err(ConnError::AuthFailed("password authentication failed".into()));
            }
        }
    }

    let mut resp = handle
        .authenticate_keyboard_interactive_start(user, None)
        .await
        .map_err(|e| ConnError::Connect(format!("{e}")))?;

    for _ in 0..8 {
        match resp {
            KeyboardInteractiveAuthResponse::Success => return Ok(()),
            KeyboardInteractiveAuthResponse::Failure { .. } => {
                return Err(ConnError::AuthFailed(
                    "keyboard-interactive authentication failed".into(),
                ));
            }
            KeyboardInteractiveAuthResponse::InfoRequest { prompts, .. } => {
                let answers = prompts.iter().map(|_| password.to_string()).collect();
                resp = handle
                    .authenticate_keyboard_interactive_respond(answers)
                    .await
                    .map_err(|e| ConnError::Connect(format!("{e}")))?;
            }
        }
    }
    Err(ConnError::AuthFailed(
        "keyboard-interactive authentication incomplete".into(),
    ))
}

enum ShellCtrl {
    Data(Vec<u8>),
    Resize { cols: u16, rows: u16 },
}

async fn open_shell_io(
    session: Arc<Handle<ClientHandler>>,
    term_type: &str,
    cols: u16,
    rows: u16,
) -> Result<SshIoSession, ConnError> {
    let mut channel = session
        .channel_open_session()
        .await
        .map_err(|e| ConnError::Connect(format!("open shell channel: {e}")))?;

    channel
        .request_pty(
            true,
            if term_type.is_empty() {
                "xterm-256color"
            } else {
                term_type
            },
            cols.max(1) as u32,
            rows.max(1) as u32,
            0,
            0,
            &[],
        )
        .await
        .map_err(|e| ConnError::Connect(format!("request pty: {e}")))?;
    channel
        .request_shell(true)
        .await
        .map_err(|e| ConnError::Connect(format!("request shell: {e}")))?;

    let (out_tx, out_rx) = mpsc::sync_channel::<Vec<u8>>(512);
    let (ctrl_tx, mut ctrl_rx) = tmpsc::unbounded_channel::<ShellCtrl>();
    let alive = Arc::new(AtomicBool::new(true));
    let alive_bridge = Arc::clone(&alive);

    tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;
                ctrl = ctrl_rx.recv() => {
                    match ctrl {
                        Some(ShellCtrl::Data(buf)) => {
                            if buf.is_empty() {
                                continue;
                            }
                            if let Err(err) = channel.data(Cursor::new(buf)).await {
                                tracing::debug!("russh shell write: {err}");
                                break;
                            }
                        }
                        Some(ShellCtrl::Resize { cols, rows }) => {
                            let _ = channel
                                .window_change(cols.max(1) as u32, rows.max(1) as u32, 0, 0)
                                .await;
                        }
                        None => break,
                    }
                }
                msg = channel.wait() => {
                    match msg {
                        Some(ChannelMsg::Data { ref data }) => {
                            if out_tx.send(data.to_vec()).is_err() {
                                break;
                            }
                        }
                        Some(ChannelMsg::ExtendedData { ref data, .. }) => {
                            let bytes: &[u8] = data.as_ref();
                            if out_tx.send(bytes.to_vec()).is_err() {
                                break;
                            }
                        }
                        Some(ChannelMsg::Eof) | None => break,
                        Some(ChannelMsg::ExitStatus { .. }) => break,
                        _ => {}
                    }
                }
            }
        }
        alive_bridge.store(false, Ordering::SeqCst);
        let _ = channel.eof().await;
        let _ = channel.close().await;
    });

    let reader: Box<dyn Read + Send> = Box::new(PipeReader {
        rx: out_rx,
        leftover: Vec::new(),
        pos: 0,
        alive: Arc::clone(&alive),
    });
    let writer: Box<dyn Write + Send> = Box::new(PipeWriter {
        tx: ctrl_tx.clone(),
        alive: Arc::clone(&alive),
    });

    let resize_tx = ctrl_tx;
    let resize = Arc::new(move |c: u16, r: u16| -> Result<(), ConnError> {
        resize_tx
            .send(ShellCtrl::Resize { cols: c, rows: r })
            .map_err(|_| ConnError::NotConnected)?;
        Ok(())
    });

    SshIoSession::spawn(reader, writer, cols, rows, Some(resize), Vec::new(), None)
}

struct PipeReader {
    rx: mpsc::Receiver<Vec<u8>>,
    leftover: Vec<u8>,
    pos: usize,
    alive: Arc<AtomicBool>,
}

impl Read for PipeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos < self.leftover.len() {
            let n = (self.leftover.len() - self.pos).min(buf.len());
            buf[..n].copy_from_slice(&self.leftover[self.pos..self.pos + n]);
            self.pos += n;
            if self.pos >= self.leftover.len() {
                self.leftover.clear();
                self.pos = 0;
            }
            return Ok(n);
        }
        match self.rx.recv_timeout(Duration::from_millis(20)) {
            Ok(chunk) => {
                if chunk.is_empty() {
                    return Err(io::ErrorKind::WouldBlock.into());
                }
                let n = chunk.len().min(buf.len());
                buf[..n].copy_from_slice(&chunk[..n]);
                if n < chunk.len() {
                    self.leftover = chunk[n..].to_vec();
                    self.pos = 0;
                }
                Ok(n)
            }
            Err(RecvTimeoutError::Timeout) => {
                if !self.alive.load(Ordering::SeqCst) {
                    Ok(0)
                } else {
                    Err(io::ErrorKind::WouldBlock.into())
                }
            }
            Err(RecvTimeoutError::Disconnected) => Ok(0),
        }
    }
}

struct PipeWriter {
    tx: tmpsc::UnboundedSender<ShellCtrl>,
    alive: Arc<AtomicBool>,
}

impl Write for PipeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !self.alive.load(Ordering::SeqCst) {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "shell closed"));
        }
        self.tx
            .send(ShellCtrl::Data(buf.to_vec()))
            .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct RusshRemoteExec {
    session: Arc<Handle<ClientHandler>>,
    host: String,
}

impl RemoteExec for RusshRemoteExec {
    fn run_command(
        &self,
        _vault: Option<&Vault>,
        remote_cmd: &str,
    ) -> Result<String, ConnError> {
        let session = Arc::clone(&self.session);
        let wrapped = wrap_sh_c(remote_cmd);
        let host = self.host.clone();
        runtime()
            .block_on(async move {
                let mut channel = session.channel_open_session().await.map_err(|e| {
                    ConnError::Connect(format!("exec channel {host}: {e}"))
                })?;
                channel
                    .exec(true, wrapped)
                    .await
                    .map_err(|e| ConnError::Connect(format!("exec: {e}")))?;

                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                let mut status = None;
                loop {
                    match channel.wait().await {
                        Some(ChannelMsg::Data { ref data }) => stdout.extend_from_slice(data),
                        Some(ChannelMsg::ExtendedData { ref data, .. }) => {
                            stderr.extend_from_slice(data)
                        }
                        Some(ChannelMsg::ExitStatus { exit_status }) => status = Some(exit_status),
                        Some(ChannelMsg::Eof) | None => break,
                        _ => {}
                    }
                }
                let _ = channel.close().await;

                let text = String::from_utf8_lossy(&stdout).into_owned();
                let err_s = String::from_utf8_lossy(&stderr);
                let body = if text.contains("VSTERM_BEGIN") {
                    text
                } else if err_s.contains("VSTERM_BEGIN") {
                    err_s.into_owned()
                } else {
                    let mut combined = text;
                    if !err_s.is_empty() {
                        if !combined.is_empty() {
                            combined.push('\n');
                        }
                        combined.push_str(&err_s);
                    }
                    combined
                };

                if let Some(code) = status {
                    if code != 0 && !body.contains("VSTERM_BEGIN") && body.trim().is_empty() {
                        return Err(ConnError::Connect(format!(
                            "remote command exit {code}"
                        )));
                    }
                }
                Ok(body)
            })
    }
}

impl RemoteFs for RusshRemoteExec {
    fn list_dir(&self, path: &str) -> Result<Vec<RemoteDirEntry>, ConnError> {
        let session = Arc::clone(&self.session);
        let host = self.host.clone();
        let path = path.to_string();
        runtime().block_on(async move {
            let sftp = open_sftp(session, &host).await?;
            let mut entries = Vec::new();
            let dir = sftp
                .read_dir(&path)
                .await
                .map_err(|e| ConnError::Connect(format!("sftp list {path}: {e}")))?;
            for entry in dir {
                let name = entry.file_name();
                if name == "." || name == ".." {
                    continue;
                }
                let meta = entry.metadata();
                let is_dir = meta.file_type() == FileType::Dir;
                entries.push(RemoteDirEntry {
                    name,
                    is_dir,
                    size: meta.size,
                    mtime: meta.mtime.map(|t| t as u64),
                });
            }
            entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            });
            let _ = sftp.close().await;
            Ok(entries)
        })
    }

    fn get_file(
        &self,
        remote_path: &str,
        local_path: &Path,
        progress: Option<&ArcProgress>,
    ) -> Result<(), ConnError> {
        self.get_path(remote_path, local_path, progress)
    }

    fn get_path(
        &self,
        remote_path: &str,
        local_path: &Path,
        progress: Option<&ArcProgress>,
    ) -> Result<(), ConnError> {
        let session = Arc::clone(&self.session);
        let host = self.host.clone();
        let remote_path = remote_path.to_string();
        let local_path = local_path.to_path_buf();
        let progress_ui = progress.cloned();
        let progress_worker = progress_ui.clone();
        let result: Result<(), ConnError> = runtime().block_on(async move {
            let sftp = open_sftp(session, &host).await?;
            remote_tree::run_download(
                &sftp,
                &remote_path,
                &local_path,
                progress_worker.as_ref(),
            )
            .await?;
            let _ = sftp.close().await;
            Ok(())
        });
        finish_transfer(&result, progress_ui.as_ref());
        result
    }

    fn put_file(
        &self,
        local_path: &Path,
        remote_path: &str,
        progress: Option<&ArcProgress>,
    ) -> Result<(), ConnError> {
        self.put_path(local_path, remote_path, progress)
    }

    fn put_path(
        &self,
        local_path: &Path,
        remote_path: &str,
        progress: Option<&ArcProgress>,
    ) -> Result<(), ConnError> {
        let session = Arc::clone(&self.session);
        let host = self.host.clone();
        let remote_path = remote_path.to_string();
        let local_path = local_path.to_path_buf();
        let progress_ui = progress.cloned();
        let progress_worker = progress_ui.clone();
        let result: Result<(), ConnError> = runtime().block_on(async move {
            let sftp = open_sftp(session, &host).await?;
            remote_tree::run_upload(
                &sftp,
                &local_path,
                &remote_path,
                progress_worker.as_ref(),
            )
            .await?;
            let _ = sftp.close().await;
            Ok(())
        });
        finish_transfer(&result, progress_ui.as_ref());
        result
    }

    fn remove(&self, remote_path: &str, is_dir: bool) -> Result<(), ConnError> {
        let session = Arc::clone(&self.session);
        let host = self.host.clone();
        let remote_path = remote_path.to_string();
        runtime().block_on(async move {
            let sftp = open_sftp(session, &host).await?;
            let res = if is_dir {
                sftp.remove_dir(&remote_path)
                    .await
                    .map_err(|e| ConnError::Connect(format!("sftp rmdir {remote_path}: {e}")))
            } else {
                sftp.remove_file(&remote_path)
                    .await
                    .map_err(|e| ConnError::Connect(format!("sftp rm {remote_path}: {e}")))
            };
            let _ = sftp.close().await;
            res
        })
    }

    fn rename(&self, from: &str, to: &str) -> Result<(), ConnError> {
        let session = Arc::clone(&self.session);
        let host = self.host.clone();
        let from = from.to_string();
        let to = to.to_string();
        runtime().block_on(async move {
            let sftp = open_sftp(session, &host).await?;
            sftp.rename(&from, &to)
                .await
                .map_err(|e| ConnError::Connect(format!("sftp rename {from} → {to}: {e}")))?;
            let _ = sftp.close().await;
            Ok(())
        })
    }
}

fn finish_transfer(result: &Result<(), ConnError>, progress: Option<&ArcProgress>) {
    match result {
        Ok(()) => {
            if let Some(p) = progress {
                p.finish_ok();
            }
        }
        Err(e) => {
            if let Some(p) = progress {
                p.finish_err(e.to_string());
            }
        }
    }
}

async fn open_sftp(
    session: Arc<Handle<ClientHandler>>,
    host: &str,
) -> Result<SftpSession, ConnError> {
    let channel = session
        .channel_open_session()
        .await
        .map_err(|e| ConnError::Connect(format!("sftp channel {host}: {e}")))?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|e| ConnError::Connect(format!("sftp subsystem {host}: {e}")))?;
    SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| ConnError::Connect(format!("sftp init {host}: {e}")))
}

fn wrap_sh_c(script: &str) -> String {
    format!(
        "sh -s <<'VSTERM_EOF'\n{}\nVSTERM_EOF",
        script.trim()
    )
}
