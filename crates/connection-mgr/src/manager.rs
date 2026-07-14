use crate::backend::{SshBackend, SshSession};
use crate::error::ConnError;
use crate::russh_backend::RusshBackend;
use crate::system_ssh::{resolve_backend, SystemSshBackend};
use parking_lot::Mutex;
use session_tree::{BackendKind, SessionConfig};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use term_core::{LocalPtySession, TerminalHandle};
use uuid::Uuid;

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

/// Runtime active connection shown in the vertical list (left-2 panel).
pub struct ActiveConnection {
    pub id: ConnectionId,
    pub title: String,
    pub color_tag: Option<String>,
    pub state: ConnectionState,
    pub session_id: Option<String>,
    pub terminal: TerminalHandle,
    /// Local PTY (stage 1 local shell / system ssh). Held to keep the process alive.
    pub(crate) local_pty: Option<LocalPtySession>,
    /// Future: russh session handle (stage 4).
    #[allow(dead_code)]
    pub(crate) ssh_session: Option<Box<dyn SshSession>>,
}

impl ActiveConnection {
    pub fn write_input(&self, data: &[u8]) -> Result<(), ConnError> {
        if let Some(pty) = &self.local_pty {
            pty.write_all(data)
                .map_err(|e| ConnError::Term(e.to_string()))?;
        }
        Ok(())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), ConnError> {
        if let Some(pty) = &self.local_pty {
            pty.resize(cols, rows)
                .map_err(|e| ConnError::Term(e.to_string()))?;
        } else {
            self.terminal
                .resize(cols, rows)
                .map_err(|e| ConnError::Term(e.to_string()))?;
        }
        Ok(())
    }
}

/// Manages concurrent connections; UI only renders the selected one's terminal.
pub struct ConnectionManager {
    connections: Mutex<HashMap<ConnectionId, ActiveConnection>>,
    order: Mutex<Vec<ConnectionId>>,
    active: Mutex<Option<ConnectionId>>,
    generation: AtomicU64,
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
            local_pty: Some(pty),
            ssh_session: None,
        };
        self.insert(conn);
        Ok(id)
    }

    /// Open a connection from a saved session config (dual backend).
    pub async fn open_session(&self, config: &SessionConfig) -> Result<ConnectionId, ConnError> {
        let kind = resolve_backend(config.backend);
        let id = ConnectionId::new();
        let title = config.name.clone();
        let color_tag = config.color_tag.clone();

        match kind {
            BackendKind::System => {
                // System ssh currently returns a raw PTY session; for stage 1–2 we
                // also expose a local-shell path. Full channel→terminal wiring lands in stage 4.
                let backend = SystemSshBackend::new();
                let mut session = backend.connect(config).await?;
                let channel = session.open_shell((80, 24)).await?;
                drop(channel);
                let terminal = TerminalHandle::new(80, 24);
                let conn = ActiveConnection {
                    id,
                    title,
                    color_tag,
                    state: ConnectionState::Connected,
                    session_id: Some(config.id.clone()),
                    terminal,
                    local_pty: None,
                    ssh_session: Some(session),
                };
                self.insert(conn);
                Ok(id)
            }
            BackendKind::Builtin | BackendKind::Auto => {
                let backend = RusshBackend::new();
                let session = backend.connect(config).await?;
                let terminal = TerminalHandle::new(80, 24);
                let conn = ActiveConnection {
                    id,
                    title,
                    color_tag,
                    state: ConnectionState::Connecting,
                    session_id: Some(config.id.clone()),
                    terminal,
                    local_pty: None,
                    ssh_session: Some(session),
                };
                self.insert(conn);
                Ok(id)
            }
        }
    }

    fn insert(&self, conn: ActiveConnection) {
        let id = conn.id;
        self.connections.lock().insert(id, conn);
        self.order.lock().push(id);
        *self.active.lock() = Some(id);
        self.bump();
    }

    pub fn close(&self, id: ConnectionId) {
        // Drop connection outside the map lock so PTY shutdown cannot deadlock UI.
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
        drop(drained);
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
