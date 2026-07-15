use crate::backend::{SshBackend, SshChannel, SshSession};
use crate::error::ConnError;
use crate::ssh_io::SshIoSession;
use async_trait::async_trait;
use parking_lot::Mutex;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use session_tree::{AuthConfig, BackendKind, SessionConfig};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use vault::Vault;

/// Resolved secrets for one connect attempt (never logged).
#[derive(Debug, Default, Clone)]
pub struct AuthMaterial {
    pub password: Option<String>,
    pub passphrase: Option<String>,
}

impl AuthMaterial {
    pub fn needs_pty_feed(&self) -> bool {
        self.password.is_some() || self.passphrase.is_some()
    }
}

/// System OpenSSH backend driven by portable-pty.
pub struct SystemSshBackend;

impl SystemSshBackend {
    pub fn new() -> Self {
        Self
    }

    pub fn is_available() -> bool {
        which_ssh().is_some()
    }

    pub fn ssh_path() -> Option<PathBuf> {
        which_ssh()
    }

    /// Open an interactive shell over system `ssh`, wired to [`SshIoSession`].
    pub async fn open_interactive(
        config: &SessionConfig,
        vault: Option<&Vault>,
        interactive_password: Option<String>,
        cols: u16,
        rows: u16,
    ) -> Result<SshIoSession, ConnError> {
        preflight(
            config,
            vault,
            PreflightOpts::connecting(interactive_password.is_some()),
        )?;
        let auth = resolve_auth(config, vault, interactive_password)?;
        let runtime = connect_session(config, &auth).await?;
        let child = Arc::clone(&runtime.inner.lock().child);
        if auth.password.is_some() || auth.passphrase.is_some() {
            let prelude = authenticate_and_collect_prelude(&runtime, &auth)?;
            let (reader, writer) = runtime.take_io(cols, rows)?;
            let runtime_resize = Arc::clone(&runtime.inner);
            let resize = Arc::new(move |cols: u16, rows: u16| {
                runtime_resize.lock().resize_pty_sync(cols, rows)
            });
            return SshIoSession::spawn(
                reader,
                writer,
                cols,
                rows,
                Some(resize),
                sanitize_shell_prelude(&prelude),
                Some(child),
            );
        }
        let (reader, writer) = runtime.take_io(cols, rows)?;
        let runtime_resize = Arc::clone(&runtime.inner);
        let resize = Arc::new(move |cols: u16, rows: u16| {
            runtime_resize.lock().resize_pty_sync(cols, rows)
        });
        SshIoSession::spawn(
            reader,
            writer,
            cols,
            rows,
            Some(resize),
            Vec::new(),
            Some(child),
        )
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
        preflight(config, None, PreflightOpts::connecting(false))?;
        let auth = AuthMaterial::default();
        let runtime = connect_session(config, &auth).await?;
        Ok(Box::new(SystemSshSessionAdapter { runtime }))
    }
}

struct SystemSshRuntime {
    inner: Arc<Mutex<SystemSshSessionInner>>,
}

impl SystemSshRuntime {
    fn take_io(
        &self,
        cols: u16,
        rows: u16,
    ) -> Result<(Box<dyn Read + Send>, Box<dyn Write + Send>), ConnError> {
        self.inner.lock().take_io(cols, rows)
    }
}

struct SystemSshSessionInner {
    master: Box<dyn MasterPty + Send>,
    writer: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
    writer_taken: AtomicBool,
    alive: Arc<AtomicBool>,
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
}

struct SystemSshSessionAdapter {
    runtime: SystemSshRuntime,
}

async fn connect_session(
    config: &SessionConfig,
    auth: &AuthMaterial,
) -> Result<SystemSshRuntime, ConnError> {
    let ssh = which_ssh().ok_or_else(system_ssh_missing)?;
    let cmd = build_ssh_command(&ssh, config, auth)?;

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

    drop(pair.slave);

    let writer = pair
        .master
        .take_writer()
        .map_err(|e| ConnError::Term(e.to_string()))?;
    let writer_slot = Arc::new(Mutex::new(Some(writer)));

    Ok(SystemSshRuntime {
        inner: Arc::new(Mutex::new(SystemSshSessionInner {
            master: pair.master,
            writer: writer_slot,
            writer_taken: AtomicBool::new(false),
            alive: Arc::new(AtomicBool::new(true)),
            child: Arc::new(Mutex::new(child)),
        })),
    })
}

impl SystemSshSessionInner {
    fn take_io(
        &mut self,
        cols: u16,
        rows: u16,
    ) -> Result<(Box<dyn Read + Send>, Box<dyn Write + Send>), ConnError> {
        self.resize_pty_sync(cols, rows)?;
        let reader = self
            .master
            .try_clone_reader()
            .map_err(|e| ConnError::Term(e.to_string()))?;
        if self.writer_taken.swap(true, Ordering::SeqCst) {
            return Err(ConnError::Backend("shell channel already opened".into()));
        }
        let writer = self
            .writer
            .lock()
            .take()
            .ok_or_else(|| ConnError::Backend("shell writer unavailable".into()))?;
        Ok((reader, writer))
    }

    fn resize_pty_sync(&self, cols: u16, rows: u16) -> Result<(), ConnError> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| ConnError::Term(e.to_string()))
    }
}

#[async_trait]
impl SshSession for SystemSshSessionAdapter {
    async fn open_shell(
        &mut self,
        term_size: (u16, u16),
    ) -> Result<Box<dyn SshChannel>, ConnError> {
        let (reader, writer) = self.runtime.take_io(term_size.0, term_size.1)?;
        Ok(Box::new(SystemSshChannel { reader, writer }))
    }

    async fn resize_pty(&mut self, cols: u16, rows: u16) -> Result<(), ConnError> {
        self.runtime.inner.lock().resize_pty_sync(cols, rows)
    }

    async fn disconnect(&mut self) -> Result<(), ConnError> {
        self.runtime
            .inner
            .lock()
            .alive
            .store(false, Ordering::Relaxed);
        Ok(())
    }

    fn is_alive(&self) -> bool {
        self.runtime.inner.lock().alive.load(Ordering::Relaxed)
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

fn apply_terminal_env(cmd: &mut CommandBuilder, term_type: &str) {
    let term = if term_type.trim().is_empty() {
        "xterm-256color"
    } else {
        term_type.trim()
    };
    // OpenSSH sends the local TERM in the pty request; without this, Windows often
    // has no TERM and remotes fall back to monochrome (e.g. `ip` / `ls` without color).
    cmd.env("TERM", term);
    cmd.env("COLORTERM", "truecolor");
}

fn build_ssh_command(
    ssh: &Path,
    config: &SessionConfig,
    auth: &AuthMaterial,
) -> Result<CommandBuilder, ConnError> {
    if let (Some(_pwd), Some(sshpass)) = (&auth.password, which_sshpass()) {
        let mut cmd = CommandBuilder::new(sshpass);
        apply_terminal_env(&mut cmd, &config.term_type);
        cmd.arg("-p");
        cmd.arg(auth.password.as_deref().unwrap_or_default());
        cmd.arg(ssh);
        push_ssh_args(&mut cmd, config, auth, true, None);
        return Ok(cmd);
    }

    let mut cmd = CommandBuilder::new(ssh);
    apply_terminal_env(&mut cmd, &config.term_type);
    push_ssh_args(&mut cmd, config, auth, false, None);
    Ok(cmd)
}

fn push_ssh_args(
    cmd: &mut CommandBuilder,
    config: &SessionConfig,
    auth: &AuthMaterial,
    via_sshpass: bool,
    remote_cmd: Option<&str>,
) {
    if remote_cmd.is_some() {
        cmd.arg("-T");
    } else {
        cmd.arg("-tt");
    }
    cmd.arg("-o");
    cmd.arg("StrictHostKeyChecking=accept-new");
    cmd.arg("-o");
    cmd.arg("BatchMode=no");
    if auth.password.is_some() && !via_sshpass {
        cmd.arg("-o");
        cmd.arg("PreferredAuthentications=password,keyboard-interactive,publickey");
    }
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
    if let Some(script) = remote_cmd {
        cmd.arg(script);
    }
}

/// Build a non-interactive `ssh host command` for remote metrics / routes.
pub fn build_exec_command(
    config: &SessionConfig,
    auth: &AuthMaterial,
    remote_cmd: &str,
) -> Result<CommandBuilder, ConnError> {
    let ssh = which_ssh().ok_or_else(system_ssh_missing)?;
    if let (Some(_pwd), Some(sshpass)) = (&auth.password, which_sshpass()) {
        let mut cmd = CommandBuilder::new(sshpass);
        cmd.arg("-p");
        cmd.arg(auth.password.as_deref().unwrap_or_default());
        cmd.arg(ssh);
        push_ssh_args(&mut cmd, config, auth, true, Some(remote_cmd));
        return Ok(cmd);
    }
    let mut cmd = CommandBuilder::new(ssh);
    push_ssh_args(&mut cmd, config, auth, false, Some(remote_cmd));
    Ok(cmd)
}

pub fn auth_failure_text(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("permission denied")
        || lower.contains("authentication failed")
        || lower.contains("too many authentication failures")
        || lower.contains("access denied")
}

fn authenticate_and_collect_prelude(
    runtime: &SystemSshRuntime,
    auth: &AuthMaterial,
) -> Result<Vec<u8>, ConnError> {
    let inner = runtime.inner.lock();
    let mut reader = inner
        .master
        .try_clone_reader()
        .map_err(|e| ConnError::Term(e.to_string()))?;
    let writer = Arc::clone(&inner.writer);
    drop(inner);

    let auth = auth.clone();
    let using_sshpass = auth.password.is_some() && which_sshpass().is_some();
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut buf = [0u8; 4096];
    let mut prelude = Vec::new();
    let mut password_sent = auth.password.is_none();
    let mut passphrase_sent = auth.passphrase.is_none();
    let mut post_auth = Vec::new();

    if using_sshpass {
        password_sent = true;
    }

    while Instant::now() < deadline {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                prelude.extend_from_slice(&buf[..n]);
                if password_sent && passphrase_sent {
                    post_auth.extend_from_slice(&buf[..n]);
                }
                let lower = String::from_utf8_lossy(&prelude).to_ascii_lowercase();
                let post_lower = String::from_utf8_lossy(&post_auth).to_ascii_lowercase();
                if auth_failure_text(&lower) || auth_failure_text(&post_lower) {
                    return Err(ConnError::AuthFailed(
                        "wrong password or authentication failed".into(),
                    ));
                }
                if !using_sshpass
                    && !password_sent
                    && (lower.contains("password:") || lower.contains("password for"))
                {
                    if let Some(pwd) = &auth.password {
                        if let Some(w) = writer.lock().as_mut() {
                            let _ = w.write_all(pwd.as_bytes());
                            let _ = w.write_all(b"\n");
                            let _ = w.flush();
                        }
                    }
                    password_sent = true;
                    post_auth.clear();
                }
                if !passphrase_sent && lower.contains("passphrase") {
                    if let Some(pp) = &auth.passphrase {
                        if let Some(w) = writer.lock().as_mut() {
                            let _ = w.write_all(pp.as_bytes());
                            let _ = w.write_all(b"\n");
                            let _ = w.flush();
                        }
                    }
                    passphrase_sent = true;
                    post_auth.clear();
                }
                if password_sent
                    && (post_lower.contains("password:") || post_lower.contains("password for"))
                {
                    return Err(ConnError::AuthFailed("wrong password".into()));
                }
                if password_sent && passphrase_sent && shell_ready(&lower) {
                    return Ok(prelude);
                }
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }

    let lower = String::from_utf8_lossy(&prelude).to_ascii_lowercase();
    if auth_failure_text(&lower) {
        return Err(ConnError::AuthFailed("wrong password".into()));
    }
    if auth.password.is_some() && !password_sent {
        return Err(ConnError::AuthFailed("password prompt not received".into()));
    }
    if auth.passphrase.is_some() && !passphrase_sent {
        return Err(ConnError::AuthFailed("passphrase prompt not received".into()));
    }
    if (auth.password.is_some() || auth.passphrase.is_some()) && !shell_ready(&lower) {
        return Err(ConnError::AuthFailed(
            "authentication failed or timed out".into(),
        ));
    }
    Ok(prelude)
}

/// Drop SSH login prompts from bytes shown in the terminal grid.
fn sanitize_shell_prelude(prelude: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(prelude);
    let lower = text.to_ascii_lowercase();
    if let Some(idx) = lower.rfind("last login") {
        return prelude[idx..].to_vec();
    }
    for marker in ["\r\n$ ", "\n$ ", "\r\n# ", "\n# ", "\r\n> ", "\n> "] {
        if let Some(idx) = lower.rfind(marker) {
            return prelude[idx + marker.len() - 2..].to_vec();
        }
    }
    if lower.contains("password:") || lower.contains("password for") {
        return Vec::new();
    }
    prelude.to_vec()
}

fn shell_ready(lower: &str) -> bool {
    lower.contains("last login")
        || lower.ends_with("$ ")
        || lower.ends_with("# ")
        || lower.contains("\r\n$ ")
        || lower.contains("\n$ ")
        || lower.contains("\r\n# ")
        || lower.contains("\n# ")
        || lower.contains("\r\n> ")
        || lower.contains("\n> ")
}

pub fn resolve_auth(
    config: &SessionConfig,
    vault: Option<&Vault>,
    interactive_password: Option<String>,
) -> Result<AuthMaterial, ConnError> {
    let mut auth = AuthMaterial::default();
    match &config.auth {
        AuthConfig::Password { password_ref } => {
            // Interactive password wins when the user typed one in the dialog.
            if let Some(pwd) = interactive_password.filter(|p| !p.is_empty()) {
                auth.password = Some(pwd);
            } else if let Some(r) = password_ref.as_ref().filter(|r| !r.trim().is_empty()) {
                auth.password = Some(load_secret(vault, r)?);
            } else {
                return Err(ConnError::InvalidConfig {
                    field: "password".into(),
                    reason: "password required (enter in dialog or set password_ref)".into(),
                });
            }
        }
        AuthConfig::Publickey {
            passphrase_ref: Some(r),
            ..
        } => {
            auth.passphrase = Some(load_secret(vault, r)?);
        }
        AuthConfig::Publickey { .. } => {}
    }
    Ok(auth)
}

pub fn preflight(
    config: &SessionConfig,
    vault: Option<&Vault>,
    opts: PreflightOpts,
) -> Result<(), ConnError> {
    if config.host.trim().is_empty() {
        return Err(ConnError::InvalidConfig {
            field: "host".into(),
            reason: "host is empty".into(),
        });
    }
    if config.username.trim().is_empty() && !opts.allow_empty_username {
        return Err(ConnError::InvalidConfig {
            field: "username".into(),
            reason: "username is empty".into(),
        });
    }

    match &config.auth {
        AuthConfig::Publickey {
            private_key_path, ..
        } => {
            if !opts.skip_key_file_check {
                let path = expand_tilde(private_key_path);
                if !path.exists() {
                    return Err(ConnError::PrivateKeyMissing {
                        path,
                        configured_auth:
                            "auth.type=publickey — switch to password if you use password login"
                                .into(),
                    });
                }
            }
            if let AuthConfig::Publickey {
                passphrase_ref: Some(r),
                ..
            } = &config.auth
            {
                ensure_vault_secret(vault, r)?;
            }
        }
        AuthConfig::Password { password_ref } => {
            if password_ref
                .as_ref()
                .map(|r| !r.trim().is_empty())
                .unwrap_or(false)
            {
                ensure_vault_secret(vault, password_ref.as_ref().unwrap())?;
            } else if !opts.has_interactive_password {
                // UI will collect password before connect; nothing to check here.
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PreflightOpts {
    pub has_interactive_password: bool,
    pub allow_empty_username: bool,
    pub skip_key_file_check: bool,
}

impl PreflightOpts {
    pub fn connecting(has_interactive_password: bool) -> Self {
        Self {
            has_interactive_password,
            allow_empty_username: false,
            skip_key_file_check: false,
        }
    }

    pub fn before_prompt() -> Self {
        Self {
            has_interactive_password: false,
            allow_empty_username: true,
            skip_key_file_check: true,
        }
    }
}

/// Expand `~` in private key / home-relative paths.
pub fn expand_user_path(path: impl AsRef<std::path::Path>) -> PathBuf {
    expand_tilde(path.as_ref())
}

fn ensure_vault_secret(vault: Option<&Vault>, secret_ref: &str) -> Result<(), ConnError> {
    let Some(vault) = vault else {
        return Err(ConnError::VaultSecretMissing {
            secret_ref: secret_ref.to_string(),
        });
    };
    vault
        .get_ref(secret_ref)
        .map(|_| ())
        .map_err(|_| ConnError::VaultSecretMissing {
            secret_ref: secret_ref.to_string(),
        })
}

fn load_secret(vault: Option<&Vault>, secret_ref: &str) -> Result<String, ConnError> {
    let vault = vault.ok_or_else(|| ConnError::VaultSecretMissing {
        secret_ref: secret_ref.to_string(),
    })?;
    vault
        .get_ref(secret_ref)
        .map_err(|e| ConnError::Vault(format!("{secret_ref}: {e}")))
}

pub(crate) fn system_ssh_missing() -> ConnError {
    ConnError::Backend(format!("SYSTEM_SSH_MISSING:{}", system_ssh_install_hint()))
}

pub fn system_ssh_install_hint() -> &'static str {
    #[cfg(windows)]
    {
        "Windows: install OpenSSH Client (Settings → Apps → Optional features) or Git for Windows which includes ssh.exe."
    }
    #[cfg(target_os = "macos")]
    {
        "macOS: OpenSSH should be at /usr/bin/ssh. Reinstall Xcode Command Line Tools if missing."
    }
    #[cfg(target_os = "linux")]
    {
        "Linux: install openssh-client, e.g. sudo apt install openssh-client."
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        "Install the OpenSSH client and ensure `ssh` is on PATH."
    }
}

pub(crate) fn which_ssh() -> Option<PathBuf> {
    use std::sync::OnceLock;
    static CACHED: OnceLock<Option<PathBuf>> = OnceLock::new();
    CACHED.get_or_init(find_ssh).clone()
}

fn find_ssh() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        // Prefer absolute OpenSSH / Git paths first — no console spawn.
        for candidate in [
            r"C:\Windows\System32\OpenSSH\ssh.exe",
            r"C:\Program Files\Git\usr\bin\ssh.exe",
        ] {
            let p = PathBuf::from(candidate);
            if p.exists() {
                return Some(p);
            }
        }
        // Last resort: PATH lookup via `where` (hidden console).
        let mut cmd = crate::process::command("where");
        cmd.arg("ssh");
        if cmd
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(PathBuf::from("ssh"));
        }
        None
    }
    #[cfg(not(windows))]
    {
        for candidate in ["/usr/bin/ssh", "/usr/local/bin/ssh"] {
            let p = PathBuf::from(candidate);
            if p.exists() {
                return Some(p);
            }
        }
        if std::process::Command::new("sh")
            .arg("-c")
            .arg("command -v ssh")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
        {
            return Some(PathBuf::from("ssh"));
        }
        None
    }
}

fn which_sshpass() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        None
    }
    #[cfg(not(windows))]
    {
        for candidate in ["/usr/bin/sshpass", "/usr/local/bin/sshpass"] {
            let p = PathBuf::from(candidate);
            if p.exists() {
                return Some(p);
            }
        }
        if std::process::Command::new("sh")
            .arg("-c")
            .arg("command -v sshpass")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .is_some()
        {
            return Some(PathBuf::from("sshpass"));
        }
        None
    }
}

fn expand_tilde(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = dirs_home() {
            return home.join(rest);
        }
    }
    if s == "~" {
        if let Some(home) = dirs_home() {
            return home;
        }
    }
    path.to_path_buf()
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

/// Resolve backend kind including `auto` probing.
pub fn resolve_backend(kind: BackendKind) -> BackendKind {
    match kind {
        // Product default: built-in session (shared shell + exec). System OpenSSH
        // is an explicit escape hatch for unusual hosts / enterprise configs.
        BackendKind::Auto => BackendKind::Builtin,
        other => other,
    }
}

pub fn backend_unavailable_error(resolved: BackendKind) -> ConnError {
    match resolved {
        BackendKind::System => system_ssh_missing(),
        BackendKind::Builtin => ConnError::Backend(
            "BUILTIN_UNAVAILABLE:Built-in SSH engine failed to start.".into(),
        ),
        BackendKind::Auto => ConnError::Backend(format!(
            "BOTH_BACKENDS_UNAVAILABLE:Neither system OpenSSH nor the built-in engine is available. {}",
            system_ssh_install_hint()
        )),
    }
}
