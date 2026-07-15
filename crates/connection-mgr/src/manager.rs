use crate::backend::SshSession;
use crate::error::ConnError;
use crate::remote_exec::RemoteSession;
use crate::russh_backend::RusshBackend;
use crate::ssh_io::SshIoSession;
use crate::system_ssh::{backend_unavailable_error, resolve_backend, SystemSshBackend};
use parking_lot::Mutex;
use session_tree::{BackendKind, SessionConfig};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use term_core::{LocalPtySession, OutputHook, TerminalHandle};
use uuid::Uuid;
use vault::Vault;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectionId(pub Uuid);

impl ConnectionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ConnectionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Connecting,
    Connected,
    Disconnected,
    Failed,
}

/// Authenticated SSH session ready to insert into the manager.
pub struct EstablishedSsh {
    pub io: SshIoSession,
    pub remote: RemoteSession,
}

enum ConnectionIo {
    Local(LocalPtySession),
    Ssh(SshIoSession),
}

impl ConnectionIo {
    fn terminal(&self) -> &TerminalHandle {
        match self {
            Self::Local(p) => p.terminal(),
            Self::Ssh(s) => s.terminal(),
        }
    }

    fn write_input(&self, data: &[u8]) -> Result<(), ConnError> {
        match self {
            Self::Local(p) => p
                .write_all(data)
                .map_err(|e| ConnError::Term(e.to_string())),
            Self::Ssh(s) => s.write_all(data),
        }
    }

    fn resize(&self, cols: u16, rows: u16) -> Result<(), ConnError> {
        match self {
            Self::Local(p) => p
                .resize(cols, rows)
                .map_err(|e| ConnError::Term(e.to_string())),
            Self::Ssh(s) => s.resize(cols, rows),
        }
    }

    fn is_alive(&self) -> bool {
        match self {
            Self::Local(p) => p.is_alive(),
            Self::Ssh(s) => s.is_alive(),
        }
    }
}

/// Runtime active connection shown in the vertical list (left-2 panel).
pub struct ActiveConnection {
    pub id: ConnectionId,
    pub title: String,
    pub color_tag: Option<String>,
    pub state: ConnectionState,
    pub session_id: Option<String>,
    pub terminal: TerminalHandle,
    pub(crate) io: Option<ConnectionIo>,
    #[allow(dead_code)]
    pub(crate) ssh_session: Option<Box<dyn SshSession>>,
    pub error_message: Option<String>,
    /// Set for SSH session connections (not local shell).
    pub remote: Option<RemoteSession>,
    pub is_local_shell: bool,
}

impl ActiveConnection {
    pub fn write_input(&self, data: &[u8]) -> Result<(), ConnError> {
        let Some(io) = &self.io else {
            return Err(ConnError::NotConnected);
        };
        io.write_input(data)
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), ConnError> {
        if let Some(io) = &self.io {
            io.resize(cols, rows)
        } else {
            self.terminal
                .resize(cols, rows)
                .map_err(|e| ConnError::Term(e.to_string()))
        }
    }
}

/// Manages concurrent connections; UI only renders the selected one's terminal.
pub struct ConnectionManager {
    connections: Mutex<HashMap<ConnectionId, ActiveConnection>>,
    order: Mutex<Vec<ConnectionId>>,
    active: Mutex<Option<ConnectionId>>,
    generation: AtomicU64,
    /// Wakes the UI when any terminal receives output (set once by the app).
    repaint_wake: Mutex<Option<OutputHook>>,
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
            order: Mutex::new(Vec::new()),
            active: Mutex::new(None),
            generation: AtomicU64::new(0),
            repaint_wake: Mutex::new(None),
        }
    }

    /// Bind an egui wake callback so PTY reader threads can request a repaint.
    pub fn set_repaint_wake(&self, hook: OutputHook) {
        *self.repaint_wake.lock() = Some(Arc::clone(&hook));
        // Apply to existing terminals without holding wake + connections in nested order
        // that conflicts with finish_connect (wake → then connections).
        let terminals: Vec<TerminalHandle> = self
            .connections
            .lock()
            .values()
            .map(|c| c.terminal.clone())
            .collect();
        for t in terminals {
            t.set_output_hook(Some(Arc::clone(&hook)));
        }
    }

    fn wire_output_hook(&self, terminal: &TerminalHandle) {
        if let Some(hook) = self.repaint_wake.lock().as_ref() {
            terminal.set_output_hook(Some(Arc::clone(hook)));
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    fn bump(&self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Stage 1 helper: open a local shell for verifying PTY + terminal rendering.
    pub fn open_local_shell(&self, title: impl Into<String>) -> Result<ConnectionId, ConnError> {
        let id = ConnectionId::new();
        let pty = LocalPtySession::spawn_shell(80, 24)
            .map_err(|e| ConnError::Term(e.to_string()))?;
        let terminal = pty.terminal().clone();
        let conn = ActiveConnection {
            id,
            title: title.into(),
            color_tag: None,
            state: ConnectionState::Connected,
            session_id: None,
            terminal,
            io: Some(ConnectionIo::Local(pty)),
            ssh_session: None,
            error_message: None,
            remote: None,
            is_local_shell: true,
        };
        self.insert(conn);
        Ok(id)
    }

    /// Insert a placeholder while an async connect is in flight.
    pub fn insert_connecting(&self, config: &SessionConfig) -> ConnectionId {
        let id = ConnectionId::new();
        let terminal = TerminalHandle::new(80, 24);
        let conn = ActiveConnection {
            id,
            title: config.name.clone(),
            color_tag: config.color_tag.clone(),
            state: ConnectionState::Connecting,
            session_id: Some(config.id.clone()),
            terminal,
            io: None,
            ssh_session: None,
            error_message: None,
            remote: None,
            is_local_shell: false,
        };
        self.insert(conn);
        id
    }

    pub fn finish_connect(
        &self,
        id: ConnectionId,
        remote: RemoteSession,
        result: Result<SshIoSession, ConnError>,
    ) {
        let hook = self.repaint_wake.lock().clone();
        let mut conns = self.connections.lock();
        let Some(conn) = conns.get_mut(&id) else {
            return;
        };
        match result {
            Ok(io) => {
                conn.terminal = io.terminal().clone();
                if let Some(h) = hook {
                    conn.terminal.set_output_hook(Some(h));
                }
                conn.io = Some(ConnectionIo::Ssh(io));
                conn.state = ConnectionState::Connected;
                conn.error_message = None;
                conn.remote = Some(remote);
                conn.is_local_shell = false;
            }
            Err(err) => {
                conn.state = ConnectionState::Failed;
                conn.error_message = Some(err.to_string());
                conn.io = None;
                conn.remote = None;
            }
        }
        drop(conns);
        self.bump();
    }

    /// Insert a fully authenticated SSH session (UI should call only after success).
    pub fn insert_ssh_connected(
        &self,
        config: &SessionConfig,
        established: EstablishedSsh,
    ) -> ConnectionId {
        let id = ConnectionId::new();
        let terminal = established.io.terminal().clone();
        let conn = ActiveConnection {
            id,
            title: config.name.clone(),
            color_tag: config.color_tag.clone(),
            state: ConnectionState::Connected,
            session_id: Some(config.id.clone()),
            terminal,
            io: Some(ConnectionIo::Ssh(established.io)),
            ssh_session: None,
            error_message: None,
            remote: Some(established.remote),
            is_local_shell: false,
        };
        self.insert(conn);
        id
    }

    /// Establish SSH I/O without inserting a connection (for async UI flow).
    pub async fn establish_ssh(
        config: &SessionConfig,
        vault: Option<&Vault>,
        interactive_password: Option<String>,
        cols: u16,
        rows: u16,
    ) -> Result<EstablishedSsh, ConnError> {
        let resolved = resolve_backend(config.backend);
        match resolved {
            BackendKind::System => {
                let io = SystemSshBackend::open_interactive(
                    config,
                    vault,
                    interactive_password.clone(),
                    cols,
                    rows,
                )
                .await?;
                Ok(EstablishedSsh {
                    io,
                    remote: RemoteSession::system(config.clone(), interactive_password),
                })
            }
            BackendKind::Builtin => {
                let est = RusshBackend::open_interactive(
                    config,
                    vault,
                    interactive_password,
                    cols,
                    rows,
                )
                .await?;
                Ok(EstablishedSsh {
                    io: est.io,
                    remote: est.remote,
                })
            }
            BackendKind::Auto => Err(backend_unavailable_error(resolved)),
        }
    }

    /// Open a connection from a saved session config (dual backend).
    pub async fn open_session(
        &self,
        config: &SessionConfig,
        vault: Option<&Vault>,
    ) -> Result<ConnectionId, ConnError> {
        let established = Self::establish_ssh(config, vault, None, 80, 24).await?;
        Ok(self.insert_ssh_connected(config, established))
    }

    fn insert(&self, conn: ActiveConnection) {
        self.wire_output_hook(&conn.terminal);
        let id = conn.id;
        self.connections.lock().insert(id, conn);
        self.order.lock().push(id);
        *self.active.lock() = Some(id);
        self.bump();
    }

    pub fn active_remote(&self) -> Option<RemoteSession> {
        let id = self.active_id()?;
        let conns = self.connections.lock();
        let conn = conns.get(&id)?;
        if conn.state == ConnectionState::Connected && !conn.is_local_shell {
            conn.remote.clone()
        } else {
            None
        }
    }

    /// True when the active tab should show local host metrics (local shell or no SSH remote).
    pub fn active_local_metrics(&self) -> bool {
        let Some(id) = self.active_id() else {
            return false;
        };
        let conns = self.connections.lock();
        let Some(conn) = conns.get(&id) else {
            return false;
        };
        conn.state == ConnectionState::Connected
            && (conn.is_local_shell || conn.remote.is_none())
    }

    pub fn close(&self, id: ConnectionId) {
        let removed = self.connections.lock().remove(&id);
        self.order.lock().retain(|x| *x != id);
        {
            let mut active = self.active.lock();
            if *active == Some(id) {
                *active = self.order.lock().last().copied();
            }
        }
        drop(removed);
        self.bump();
    }

    pub fn close_all(&self) {
        let drained: Vec<ActiveConnection> = {
            let mut map = self.connections.lock();
            let mut order = self.order.lock();
            let mut active = self.active.lock();
            order.clear();
            *active = None;
            map.drain().map(|(_, c)| c).collect()
        };
        // Drop I/O off the UI/exit thread so window close stays snappy.
        if !drained.is_empty() {
            std::thread::spawn(move || {
                drop(drained);
            });
        }
        self.bump();
    }

    pub fn set_active(&self, id: ConnectionId) {
        if self.connections.lock().contains_key(&id) {
            *self.active.lock() = Some(id);
            self.bump();
        }
    }

    pub fn active_id(&self) -> Option<ConnectionId> {
        *self.active.lock()
    }

    pub fn list_meta(&self) -> Vec<ConnectionMeta> {
        let conns = self.connections.lock();
        let order = self.order.lock();
        order
            .iter()
            .filter_map(|id| {
                conns.get(id).map(|c| ConnectionMeta {
                    id: c.id,
                    title: c.title.clone(),
                    color_tag: c.color_tag.clone(),
                    state: c.state,
                })
            })
            .collect()
    }

    pub fn with_active<R>(&self, f: impl FnOnce(&ActiveConnection) -> R) -> Option<R> {
        let id = self.active_id()?;
        let conns = self.connections.lock();
        conns.get(&id).map(f)
    }

    pub fn with_connection_mut<R>(
        &self,
        id: ConnectionId,
        f: impl FnOnce(&mut ActiveConnection) -> R,
    ) -> Option<R> {
        let mut conns = self.connections.lock();
        conns.get_mut(&id).map(f)
    }

    pub fn write_to_active(&self, data: &[u8]) -> Result<(), ConnError> {
        let id = self.active_id().ok_or(ConnError::NotConnected)?;
        let conns = self.connections.lock();
        let conn = conns.get(&id).ok_or(ConnError::NotConnected)?;
        conn.write_input(data)
    }

    pub fn resize_active(&self, cols: u16, rows: u16) -> Result<(), ConnError> {
        let id = self.active_id().ok_or(ConnError::NotConnected)?;
        let conns = self.connections.lock();
        let conn = conns.get(&id).ok_or(ConnError::NotConnected)?;
        conn.resize(cols, rows)
    }

    /// Drop dead connections (SSH process exited). Keep the tab for reconnect.
    pub fn reap_dead(&self) {
        let dead: Vec<ConnectionId> = {
            let conns = self.connections.lock();
            conns
                .values()
                .filter(|c| {
                    c.state == ConnectionState::Connected
                        && c.io.as_ref().is_some_and(|io| !io.is_alive())
                })
                .map(|c| c.id)
                .collect()
        };
        for id in dead {
            if let Some(conn) = self.connections.lock().get_mut(&id) {
                conn.state = ConnectionState::Disconnected;
                conn.io = None;
                conn.remote = None;
                conn.ssh_session = None;
            }
            self.bump();
        }
    }

    pub fn session_id_of(&self, id: ConnectionId) -> Option<String> {
        self.connections
            .lock()
            .get(&id)
            .and_then(|c| c.session_id.clone())
    }

    pub fn connection_state(&self, id: ConnectionId) -> Option<ConnectionState> {
        self.connections.lock().get(&id).map(|c| c.state)
    }

    /// Mark an existing host tab as connecting (used by in-place reconnect).
    pub fn mark_connecting(&self, id: ConnectionId) -> bool {
        let mut conns = self.connections.lock();
        let Some(conn) = conns.get_mut(&id) else {
            return false;
        };
        if conn.is_local_shell {
            return false;
        }
        conn.state = ConnectionState::Connecting;
        conn.error_message = None;
        conn.io = None;
        conn.remote = None;
        conn.ssh_session = None;
        drop(conns);
        self.bump();
        true
    }

    pub fn mark_failed(&self, id: ConnectionId, message: impl Into<String>) {
        let mut conns = self.connections.lock();
        let Some(conn) = conns.get_mut(&id) else {
            return;
        };
        conn.state = ConnectionState::Failed;
        conn.error_message = Some(message.into());
        conn.io = None;
        conn.remote = None;
        conn.ssh_session = None;
        drop(conns);
        self.bump();
    }

    pub fn mark_disconnected(&self, id: ConnectionId) {
        let mut conns = self.connections.lock();
        let Some(conn) = conns.get_mut(&id) else {
            return;
        };
        if conn.is_local_shell {
            return;
        }
        conn.state = ConnectionState::Disconnected;
        conn.error_message = None;
        conn.io = None;
        conn.remote = None;
        conn.ssh_session = None;
        drop(conns);
        self.bump();
    }

    /// Replace I/O on an existing tab after a successful reconnect.
    /// Returns the established session if the tab no longer exists (caller may insert fresh).
    pub fn replace_ssh_connected(
        &self,
        id: ConnectionId,
        config: &SessionConfig,
        established: EstablishedSsh,
    ) -> Result<(), EstablishedSsh> {
        let hook = self.repaint_wake.lock().clone();
        let mut conns = self.connections.lock();
        let Some(conn) = conns.get_mut(&id) else {
            return Err(established);
        };
        conn.title = config.name.clone();
        conn.color_tag = config.color_tag.clone();
        conn.session_id = Some(config.id.clone());
        conn.terminal = established.io.terminal().clone();
        if let Some(h) = hook {
            conn.terminal.set_output_hook(Some(h));
        }
        conn.io = Some(ConnectionIo::Ssh(established.io));
        conn.remote = Some(established.remote);
        conn.ssh_session = None;
        conn.state = ConnectionState::Connected;
        conn.error_message = None;
        conn.is_local_shell = false;
        drop(conns);
        *self.active.lock() = Some(id);
        self.bump();
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionMeta {
    pub id: ConnectionId,
    pub title: String,
    pub color_tag: Option<String>,
    pub state: ConnectionState,
}

/// Shared handle for UI.
pub type SharedConnectionManager = Arc<ConnectionManager>;
