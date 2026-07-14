use crate::backend::{SshBackend, SshChannel, SshSession};
use crate::ConnError;
use async_trait::async_trait;
use parking_lot::Mutex;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use session_tree::{AuthConfig, BackendKind, SessionConfig};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// System OpenSSH backend driven by portable-pty.
pub struct SystemSshBackend;

impl SystemSshBackend {
    pub fn new() -> Self {
        Self
    }

    pub fn is_available() -> bool {
        which_ssh().is_some()
    }
}

impl Default for SystemSshBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SshBackend for SystemSshBackend {
    async fn connect(&self, config: &SessionConfig) -> Result<Box<dyn SshSession>, ConnError> {
        let ssh = which_ssh().ok_or_else(|| {
            ConnError::Backend("system ssh command not found in PATH".into())
        })?;

        let mut cmd = CommandBuilder::new(ssh);
        cmd.arg("-tt");
        cmd.arg("-o");
        cmd.arg("StrictHostKeyChecking=accept-new");
        cmd.arg("-p");
        cmd.arg(config.port.to_string());

        if let AuthConfig::Publickey {
            private_key_path, ..
        } = &config.auth
        {
            cmd.arg("-i");
            cmd.arg(expand_tilde(private_key_path));
        }

        cmd.arg(format!("{}@{}", config.username, config.host));

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| ConnError::Term(e.to_string()))?;

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| ConnError::Connect(e.to_string()))?;

        let session = SystemSshSession {
            master: Mutex::new(pair.master),
            writer_taken: AtomicBool::new(false),
            alive: Arc::new(AtomicBool::new(true)),
            _child: child,
            config_auth: config.auth.clone(),
        };

        // Password / passphrase auto-reply lands in stage 4.
        let _ = &session.config_auth;
        Ok(Box::new(session))
    }
}

struct SystemSshSession {
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer_taken: AtomicBool,
    alive: Arc<AtomicBool>,
    _child: Box<dyn portable_pty::Child + Send + Sync>,
    config_auth: AuthConfig,
}

// Safety: MasterPty is only accessed through the Mutex.
unsafe impl Sync for SystemSshSession {}

#[async_trait]
impl SshSession for SystemSshSession {
    async fn open_shell(
        &mut self,
        term_size: (u16, u16),
    ) -> Result<Box<dyn SshChannel>, ConnError> {
        self.resize_pty(term_size.0, term_size.1).await?;
        let reader = self
            .master
            .lock()
            .try_clone_reader()
            .map_err(|e| ConnError::Term(e.to_string()))?;
        if self.writer_taken.swap(true, Ordering::SeqCst) {
            return Err(ConnError::Backend("shell channel already opened".into()));
        }
        let writer = self
            .master
            .lock()
            .take_writer()
            .map_err(|e| ConnError::Term(e.to_string()))?;
        Ok(Box::new(SystemSshChannel { reader, writer }))
    }

    async fn resize_pty(&mut self, cols: u16, rows: u16) -> Result<(), ConnError> {
        self.master
            .lock()
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| ConnError::Term(e.to_string()))
    }

    async fn disconnect(&mut self) -> Result<(), ConnError> {
        self.alive.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }
}

struct SystemSshChannel {
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
}

impl SshChannel for SystemSshChannel {
    fn reader(&mut self) -> &mut dyn io::Read {
        &mut *self.reader
    }

    fn writer(&mut self) -> &mut dyn io::Write {
        &mut *self.writer
    }
}

fn which_ssh() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        for candidate in [
            "ssh.exe",
            r"C:\Windows\System32\OpenSSH\ssh.exe",
            r"C:\Program Files\Git\usr\bin\ssh.exe",
        ] {
            let p = PathBuf::from(candidate);
            if candidate == "ssh.exe" {
                if std::process::Command::new("where")
                    .arg("ssh")
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
                {
                    return Some(PathBuf::from("ssh"));
                }
            } else if p.exists() {
                return Some(p);
            }
        }
        None
    }
    #[cfg(not(windows))]
    {
        for candidate in ["/usr/bin/ssh", "/usr/local/bin/ssh", "ssh"] {
            if candidate == "ssh" {
                return Some(PathBuf::from("ssh"));
            }
            let p = PathBuf::from(candidate);
            if p.exists() {
                return Some(p);
            }
        }
        None
    }
}

fn expand_tilde(path: &std::path::Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = dirs_next_home() {
            return home.join(rest);
        }
    }
    path.to_path_buf()
}

fn dirs_next_home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

/// Resolve backend kind including `auto` probing.
pub fn resolve_backend(kind: BackendKind) -> BackendKind {
    match kind {
        BackendKind::Auto => {
            if SystemSshBackend::is_available() {
                BackendKind::System
            } else {
                BackendKind::Builtin
            }
        }
        other => other,
    }
}
