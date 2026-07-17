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
use russh_sftp::protocol::FileAttributes;

/// Cipher negotiation order. AES-256-GCM first so we use CPU AES instructions
/// (AES-NI) instead of software chacha20-poly1305, then fall back to CTR ciphers,
/// with chacha20 kept last for servers that lack AES-GCM.
static CIPHER_PREFERENCE: &[russh::cipher::Name] = &[
    russh::cipher::AES_256_GCM,
    russh::cipher::AES_256_CTR,
    russh::cipher::AES_192_CTR,
    russh::cipher::AES_128_CTR,
    russh::cipher::CHACHA20_POLY1305,
];
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

const RUSSH_WORKER_THREADS: usize = 4;

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
            // SFTP transfers + interactive shell share this runtime; keep enough
            // workers so disk IO and packet pumping do not serialize on 2 threads.
            .worker_threads(RUSSH_WORKER_THREADS)
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
        // Channel window for SFTP throughput. Keep modest: russh 0.50 stores
        // each CHANNEL_DATA as a CryptoVec; a large window + deep mpsc queue
        // multiplies peak RAM during pipelined downloads.
        window_size: 2 * 1024 * 1024,
        maximum_packet_size: 65535,
        // In-flight channel messages before TCP backpressure. Match the
        // download pipeline so we do not queue far ahead of the disk writer.
        channel_buffer_size: 32,
        // Prefer AES-256-GCM: it uses AES-NI hardware acceleration on x86/ARM,
        // whereas russh's default (chacha20-poly1305) is software-only and caps
        // a single connection at ~15 MB/s. This is the main throughput lever.
        preferred: russh::Preferred {
            cipher: std::borrow::Cow::Borrowed(CIPHER_PREFERENCE),
            ..russh::Preferred::DEFAULT
        },
        // Default rekey is 1 GiB; on a fast LAN that pauses a ~1 GiB download
        // right at the end. AES-GCM is safe well above this; set 8 GiB.
        limits: russh::Limits {
            rekey_write_limit: 8 * 1024 * 1024 * 1024,
            rekey_read_limit: 8 * 1024 * 1024 * 1024,
            rekey_time_limit: Duration::from_secs(3600),
        },
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
        sftp: tokio::sync::Mutex::new(None),
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
        AuthResult::Failure {
            remaining_methods, ..
        } => {
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

    // Unbounded: a full sync channel blocks the Tokio worker on `send`, which
    // stalls shell writes and SSH reads on the same runtime as SFTP.
    let (out_tx, out_rx) = mpsc::channel::<Vec<u8>>();
    let (ctrl_tx, mut ctrl_rx) = tmpsc::unbounded_channel::<ShellCtrl>();
    let alive = Arc::new(AtomicBool::new(true));
    let alive_bridge = Arc::clone(&alive);
    let latency = Arc::new(ParkingMutex::new(None::<std::time::Instant>));
    let bridge_latency = Arc::clone(&latency);

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
                            if std::env::var_os("VSTERM_DIAG").is_some() {
                                if let Some(sent) = *bridge_latency.lock() {
                                    tracing::warn!(
                                        "VSTERM_DIAG: terminal input dispatched to SSH in {:.1} ms ({} bytes)",
                                        sent.elapsed().as_secs_f64() * 1000.0,
                                        buf.len()
                                    );
                                }
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
                            if let Some(sent) = bridge_latency.lock().take() {
                                if std::env::var_os("VSTERM_DIAG").is_some() {
                                    tracing::warn!(
                                        "VSTERM_DIAG: terminal first SSH output in {:.1} ms ({} bytes)",
                                        sent.elapsed().as_secs_f64() * 1000.0,
                                        data.len()
                                    );
                                }
                            }
                            if out_tx.send(data.to_vec()).is_err() {
                                break;
                            }
                        }
                        Some(ChannelMsg::ExtendedData { ref data, .. }) => {
                            if let Some(sent) = bridge_latency.lock().take() {
                                if std::env::var_os("VSTERM_DIAG").is_some() {
                                    tracing::warn!(
                                        "VSTERM_DIAG: terminal first SSH output in {:.1} ms ({} bytes stderr)",
                                        sent.elapsed().as_secs_f64() * 1000.0,
                                        data.len()
                                    );
                                }
                            }
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
        latency,
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
    latency: Arc<ParkingMutex<Option<std::time::Instant>>>,
}

impl Write for PipeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !self.alive.load(Ordering::SeqCst) {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "shell closed"));
        }
        *self.latency.lock() = Some(std::time::Instant::now());
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
    /// One SFTP subsystem for the life of this SSH connection. Opening a new
    /// channel per list/download leaked ~10 MiB RSS each time (russh channel
    /// window + buffers; `close_session` / `Drop` try_send Close is best-effort).
    sftp: tokio::sync::Mutex<Option<SharedSftp>>,
}

struct SharedSftp {
    raw: Arc<russh_sftp::client::RawSftpSession>,
    max_read: u32,
}

impl Drop for RusshRemoteExec {
    fn drop(&mut self) {
        if let Ok(mut g) = self.sftp.try_lock() {
            if let Some(s) = g.take() {
                let _ = s.raw.close_session();
            }
        }
    }
}

impl RusshRemoteExec {
    /// Lazily open (or reuse) the connection-scoped SFTP session.
    async fn shared_sftp(
        &self,
    ) -> Result<(Arc<russh_sftp::client::RawSftpSession>, u32), ConnError> {
        let mut guard = self.sftp.lock().await;
        if let Some(s) = guard.as_ref() {
            return Ok((Arc::clone(&s.raw), s.max_read));
        }
        let (raw, max_read) = open_raw_sftp(Arc::clone(&self.session), &self.host).await?;
        *guard = Some(SharedSftp {
            raw: Arc::clone(&raw),
            max_read,
        });
        Ok((raw, max_read))
    }

    /// Drop a dead SFTP session so the next call re-opens cleanly.
    async fn invalidate_sftp(&self) {
        let mut guard = self.sftp.lock().await;
        if let Some(s) = guard.take() {
            let _ = s.raw.close_session();
        }
    }
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
        let path = path.to_string();
        runtime().block_on(async move {
            let (raw, _) = self.shared_sftp().await?;
            match remote_tree::list_dir_raw(&raw, &path).await {
                Ok(entries) => Ok(entries),
                Err(e) => {
                    if sftp_session_dead(&e) {
                        self.invalidate_sftp().await;
                    }
                    Err(e)
                }
            }
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
        let remote_path = remote_path.to_string();
        let local_path = local_path.to_path_buf();
        let progress_ui = progress.cloned();
        let progress_worker = progress_ui.clone();
        let result: Result<(), ConnError> = runtime().block_on(async move {
            let (raw, max_read) = self.shared_sftp().await?;
            match remote_tree::run_download_raw(
                &raw,
                max_read,
                &remote_path,
                &local_path,
                progress_worker.as_ref(),
            )
            .await
            {
                Ok(()) => Ok(()),
                Err(e) => {
                    if sftp_session_dead(&e) {
                        self.invalidate_sftp().await;
                    }
                    Err(e)
                }
            }
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
        let remote_path = remote_path.to_string();
        let local_path = local_path.to_path_buf();
        let progress_ui = progress.cloned();
        let progress_worker = progress_ui.clone();
        let result: Result<(), ConnError> = runtime().block_on(async move {
            let (raw, _) = self.shared_sftp().await?;
            match remote_tree::run_upload_raw(
                &raw,
                &local_path,
                &remote_path,
                progress_worker.as_ref(),
            )
            .await
            {
                Ok(()) => Ok(()),
                Err(e) => {
                    if sftp_session_dead(&e) {
                        self.invalidate_sftp().await;
                    }
                    Err(e)
                }
            }
        });
        finish_transfer(&result, progress_ui.as_ref());
        result
    }

    fn remove(&self, remote_path: &str, is_dir: bool) -> Result<(), ConnError> {
        let remote_path = remote_path.to_string();
        runtime().block_on(async move {
            let (raw, _) = self.shared_sftp().await?;
            let res = if is_dir {
                raw.rmdir(&remote_path)
                    .await
                    .map_err(|e| ConnError::Connect(format!("sftp rmdir {remote_path}: {e}")))
            } else {
                raw.remove(&remote_path)
                    .await
                    .map_err(|e| ConnError::Connect(format!("sftp rm {remote_path}: {e}")))
            };
            match res {
                Ok(_) => Ok(()),
                Err(e) => {
                    if sftp_session_dead(&e) {
                        self.invalidate_sftp().await;
                    }
                    Err(e)
                }
            }
        })
    }

    fn rename(&self, from: &str, to: &str) -> Result<(), ConnError> {
        let from = from.to_string();
        let to = to.to_string();
        runtime().block_on(async move {
            let (raw, _) = self.shared_sftp().await?;
            match raw.rename(&from, &to).await {
                Ok(_) => Ok(()),
                Err(e) => {
                    let err = ConnError::Connect(format!("sftp rename {from} → {to}: {e}"));
                    if sftp_session_dead(&err) {
                        self.invalidate_sftp().await;
                    }
                    Err(err)
                }
            }
        })
    }

    fn mkdir(&self, remote_path: &str) -> Result<(), ConnError> {
        let remote_path = remote_path.to_string();
        runtime().block_on(async move {
            let (raw, _) = self.shared_sftp().await?;
            match raw
                .mkdir(&remote_path, FileAttributes::default())
                .await
            {
                Ok(_) => Ok(()),
                Err(e) => {
                    let err = ConnError::Connect(format!("sftp mkdir {remote_path}: {e}"));
                    if sftp_session_dead(&err) {
                        self.invalidate_sftp().await;
                    }
                    Err(err)
                }
            }
        })
    }

    fn write_file(&self, remote_path: &str, data: &[u8]) -> Result<(), ConnError> {
        let remote_path = remote_path.to_string();
        let data = data.to_vec();
        runtime().block_on(async move {
            let (raw, _) = self.shared_sftp().await?;
            match remote_tree::write_file_raw(&raw, &remote_path, &data).await {
                Ok(()) => Ok(()),
                Err(e) => {
                    if sftp_session_dead(&e) {
                        self.invalidate_sftp().await;
                    }
                    Err(e)
                }
            }
        })
    }
}

fn finish_transfer(result: &Result<(), ConnError>, progress: Option<&ArcProgress>) {
    // Always release the UI transfer state first. Heap reclaim must never delay
    // finish_ok — the barrier-based collector starved the shared russh runtime
    // (interactive shell + next download open) for multiple seconds.
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
    schedule_transfer_heap_reclaim();
}

/// Best-effort mimalloc reclaim after SFTP churn.
///
/// Runs on a dedicated thread only. Never touch the shared russh worker threads:
/// those also drive the interactive shell, and even fire-and-forget `mi_collect`
/// there contended with PTY echo right after downloads.
fn schedule_transfer_heap_reclaim() {
    let _ = std::thread::Builder::new()
        .name("vsterm-mi-reclaim".into())
        .spawn(|| {
            // `false` = non-forced: return pages without a full heap walk that
            // stalls other threads holding the allocator.
            unsafe {
                libmimalloc_sys::mi_collect(false);
            }
        });
}

fn sftp_session_dead(err: &ConnError) -> bool {
    let s = err.to_string().to_ascii_lowercase();
    s.contains("session closed")
        || s.contains("channel closed")
        || s.contains("broken pipe")
        || s.contains("disconnected")
        || s.contains("not connected")
}

async fn open_raw_sftp(
    session: Arc<Handle<ClientHandler>>,
    host: &str,
) -> Result<(Arc<russh_sftp::client::RawSftpSession>, u32), ConnError> {
    let channel = session
        .channel_open_session()
        .await
        .map_err(|e| ConnError::Connect(format!("sftp channel {host}: {e}")))?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|e| ConnError::Connect(format!("sftp subsystem {host}: {e}")))?;
    remote_tree::init_raw_sftp(channel.into_stream()).await
}

fn wrap_sh_c(script: &str) -> String {
    format!(
        "sh -s <<'VSTERM_EOF'\n{}\nVSTERM_EOF",
        script.trim()
    )
}
