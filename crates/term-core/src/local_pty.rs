use crate::error::TermError;
use crate::terminal::TerminalHandle;
use crate::zmodem::ZmodemBridge;
use parking_lot::Mutex;
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Local PTY session (shell or system ssh) feeding a TerminalHandle.
pub struct LocalPtySession {
    terminal: TerminalHandle,
    writer: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
    master: Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>,
    alive: Arc<AtomicBool>,
    reader_thread: Option<JoinHandle<()>>,
    child_killer: Option<Box<dyn ChildKiller + Send + Sync>>,
    zmodem: Arc<ZmodemBridge>,
}

impl LocalPtySession {
    pub fn spawn_shell(cols: u16, rows: u16) -> Result<Self, TermError> {
        #[cfg(windows)]
        let mut cmd = CommandBuilder::new("powershell.exe");
        #[cfg(not(windows))]
        let mut cmd = {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into());
            CommandBuilder::new(shell)
        };
        // Advertise a color-capable terminal so shells / tools emit ANSI colors.
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        Self::spawn_command(cmd, cols, rows)
    }

    pub fn spawn_command(cmd: CommandBuilder, cols: u16, rows: u16) -> Result<Self, TermError> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| TermError::Pty(e.to_string()))?;

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| TermError::Pty(e.to_string()))?;
        let killer = child.clone_killer();

        let terminal = TerminalHandle::new(cols, rows);
        let zmodem = Arc::new(ZmodemBridge::new());
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| TermError::Pty(e.to_string()))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| TermError::Pty(e.to_string()))?;

        let alive = Arc::new(AtomicBool::new(true));
        let alive_reader = Arc::clone(&alive);
        let term_reader = terminal.clone();
        let writer_slot = Arc::new(Mutex::new(Some(writer)));
        let writer_for_reader = Arc::clone(&writer_slot);
        let zmodem_reader = Arc::clone(&zmodem);

        let reader_thread = thread::Builder::new()
            .name("vsterm-pty-reader".into())
            .spawn(move || {
                let mut buf = [0u8; 8192];
                while alive_reader.load(Ordering::SeqCst) {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let rx = zmodem_reader.on_rx(&buf[..n]);
                            if !rx.to_wire.is_empty() {
                                let mut guard = writer_for_reader.lock();
                                if let Some(w) = guard.as_mut() {
                                    let _ = w.write_all(&rx.to_wire);
                                    let _ = w.flush();
                                }
                            }
                            if !rx.to_terminal.is_empty() {
                                term_reader.advance_bytes(&rx.to_terminal);
                            }
                        }
                        Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                        // Closing the PTY (Drop/shutdown) unblocks read with an error on Windows.
                        Err(_) => break,
                    }
                }
                alive_reader.store(false, Ordering::SeqCst);
            })
            .map_err(|e| TermError::Pty(e.to_string()))?;

        drop(pair.slave);
        drop(child);

        Ok(Self {
            terminal,
            writer: writer_slot,
            master: Arc::new(Mutex::new(Some(pair.master))),
            alive,
            reader_thread: Some(reader_thread),
            child_killer: Some(killer),
            zmodem,
        })
    }

    pub fn terminal(&self) -> &TerminalHandle {
        &self.terminal
    }

    pub fn zmodem(&self) -> &Arc<ZmodemBridge> {
        &self.zmodem
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    pub fn write_all(&self, data: &[u8]) -> Result<(), TermError> {
        if !self.is_alive() {
            return Err(TermError::NotRunning);
        }
        if self.zmodem.is_transferring() {
            return Ok(());
        }
        let mut guard = self.writer.lock();
        let writer = guard.as_mut().ok_or(TermError::NotRunning)?;
        writer.write_all(data).map_err(TermError::from)?;
        writer.flush().map_err(TermError::from)?;
        Ok(())
    }

    /// Write raw bytes even during a transfer (protocol ACKs / cancel).
    pub fn write_raw(&self, data: &[u8]) -> Result<(), TermError> {
        if !self.is_alive() {
            return Err(TermError::NotRunning);
        }
        let mut guard = self.writer.lock();
        let writer = guard.as_mut().ok_or(TermError::NotRunning)?;
        writer.write_all(data).map_err(TermError::from)?;
        writer.flush().map_err(TermError::from)?;
        Ok(())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), TermError> {
        self.terminal.resize(cols, rows)?;
        let mut guard = self.master.lock();
        let master = guard.as_mut().ok_or(TermError::NotRunning)?;
        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| TermError::Pty(e.to_string()))?;
        Ok(())
    }

    /// Kill child and close PTY handles so a blocked `read()` can return.
    fn shutdown_io(&mut self) {
        self.alive.store(false, Ordering::SeqCst);
        if let Some(mut killer) = self.child_killer.take() {
            let _ = killer.kill();
        }
        // Dropping writer/master closes ConPTY pipes and unblocks the reader thread.
        *self.writer.lock() = None;
        *self.master.lock() = None;
    }
}

impl Drop for LocalPtySession {
    fn drop(&mut self) {
        self.shutdown_io();

        if let Some(handle) = self.reader_thread.take() {
            // Never block forever on join — ConPTY read may hang even after kill.
            let (tx, rx) = mpsc::channel();
            thread::spawn(move || {
                let _ = handle.join();
                let _ = tx.send(());
            });
            match rx.recv_timeout(Duration::from_millis(150)) {
                Ok(()) => {}
                Err(_) => {
                    tracing::warn!("pty reader did not exit in time; detaching");
                }
            }
        }
    }
}
