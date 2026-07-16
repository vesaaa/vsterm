//! One-shot remote command execution (metrics / routes) and optional SFTP.
//!
//! Callers go through [`RemoteSession`] → [`RemoteExec`] / [`RemoteFs`].

use crate::backend::RemoteExec;
use crate::error::ConnError;
use crate::process;
use crate::remote_fs::{
    ArcProgress, RemoteDirEntry, RemoteFs, UnsupportedRemoteFs,
};
use crate::system_ssh::{resolve_auth, system_ssh_missing, which_ssh, AuthMaterial};
use session_tree::{AuthConfig, SessionConfig};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
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
    /// System OpenSSH: each call may spawn `ssh.exe` (always via [`process::command`]).
    /// SFTP is not supported yet — [`Self::sftp_supported`] is false.
    pub fn system(config: SessionConfig, interactive_password: Option<String>) -> Self {
        Self {
            username: config.username.clone(),
            host: config.host.clone(),
            exec: Arc::new(SystemRemoteExec {
                config,
                interactive_password,
            }),
            fs: Arc::new(UnsupportedRemoteFs),
        }
    }

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
}

/// System-backend exec: `ssh -T user@host 'sh -s <<…'`.
struct SystemRemoteExec {
    config: SessionConfig,
    interactive_password: Option<String>,
}

impl RemoteExec for SystemRemoteExec {
    fn run_command(&self, vault: Option<&Vault>, remote_cmd: &str) -> Result<String, ConnError> {
        let auth = resolve_auth(
            &self.config,
            vault,
            self.interactive_password.clone(),
        )?;
        run_system_ssh(&self.config, &auth, remote_cmd)
    }
}

fn run_system_ssh(
    config: &SessionConfig,
    auth: &AuthMaterial,
    remote_cmd: &str,
) -> Result<String, ConnError> {
    let ssh = which_ssh().ok_or_else(system_ssh_missing)?;
    let wrapped = wrap_sh_c(remote_cmd);

    let askpass_exe = if auth.password.is_some() || auth.passphrase.is_some() {
        Some(
            std::env::current_exe()
                .map_err(|e| ConnError::Backend(format!("current_exe: {e}")))?,
        )
    } else {
        None
    };
    let secret = auth
        .password
        .clone()
        .or_else(|| auth.passphrase.clone());

    // Unified spawn entry — CREATE_NO_WINDOW on Windows.
    let mut cmd = process::command(&ssh);
    cmd.arg("-T")
        .arg("-o")
        .arg("StrictHostKeyChecking=accept-new")
        .arg("-o")
        .arg("BatchMode=no")
        .arg("-o")
        .arg("NumberOfPasswordPrompts=1")
        .arg("-o")
        .arg("ConnectTimeout=12")
        .arg("-p")
        .arg(config.port.to_string());

    if auth.password.is_some() {
        cmd.arg("-o")
            .arg("PreferredAuthentications=password,keyboard-interactive")
            .arg("-o")
            .arg("PubkeyAuthentication=no");
    }

    if let AuthConfig::Publickey {
        private_key_path, ..
    } = &config.auth
    {
        cmd.arg("-i")
            .arg(expand_tilde(private_key_path.to_string_lossy().as_ref()));
    }

    if let (Some(ask), Some(secret)) = (&askpass_exe, &secret) {
        cmd.env("SSH_ASKPASS", ask);
        cmd.env("SSH_ASKPASS_REQUIRE", "force");
        cmd.env("DISPLAY", "1");
        cmd.env("VSTERM_ASKPASS_MODE", "1");
        cmd.env("VSTERM_ASKPASS_SECRET", secret);
        cmd.env_remove("SSH_AUTH_SOCK");
    }

    cmd.arg(format!("{}@{}", config.username, config.host))
        .arg(&wrapped)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| ConnError::Connect(format!("spawn ssh: {e}")))?;

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| ConnError::Backend("missing stdout".into()))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| ConnError::Backend("missing stderr".into()))?;

    let out_h = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let err_h = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr.read_to_end(&mut buf);
        buf
    });

    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(st) = child.try_wait().map_err(ConnError::Io)? {
            break st;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ConnError::Connect("remote command timed out".into()));
        }
        std::thread::sleep(Duration::from_millis(40));
    };

    let out = out_h.join().unwrap_or_default();
    let err = err_h.join().unwrap_or_default();

    let text = String::from_utf8_lossy(&out).into_owned();
    let err_s = String::from_utf8_lossy(&err);

    if looks_like_auth_failure(&text) || looks_like_auth_failure(&err_s) {
        return Err(ConnError::AuthFailed("authentication failed".into()));
    }
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
    if !status.success() && !body.contains("VSTERM_BEGIN") && body.trim().is_empty() {
        return Err(ConnError::Connect(format!(
            "ssh exit {status}: {}",
            truncate(&body, 240)
        )));
    }
    Ok(body)
}

/// Feed a multi-line script without nested-quote breakage (Windows OpenSSH + dash).
fn wrap_sh_c(script: &str) -> String {
    format!(
        "sh -s <<'VSTERM_EOF'\n{}\nVSTERM_EOF",
        script.trim()
    )
}

fn looks_like_auth_failure(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    (lower.contains("permission denied")
        || lower.contains("authentication failed")
        || lower.contains("too many authentication failures")
        || lower.contains("access denied"))
        && !lower.contains("vsterm_begin")
}

fn truncate(s: &str, max: usize) -> String {
    let t = s.trim().replace('\n', " ");
    if t.chars().count() <= max {
        t
    } else {
        format!("{}…", t.chars().take(max).collect::<String>())
    }
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}
