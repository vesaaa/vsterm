//! SSH interactive I/O bridged to a [`TerminalHandle`] (reader thread + writer).

use crate::error::ConnError;
use parking_lot::Mutex;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use term_core::{TerminalHandle, ZmodemBridge};

/// Keeps SSH shell channel I/O alive and feeds the terminal grid.
pub struct SshIoSession {
    terminal: TerminalHandle,
    writer: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
    alive: Arc<AtomicBool>,
    reader_thread: Option<JoinHandle<()>>,
    resize: Option<Arc<dyn Fn(u16, u16) -> Result<(), ConnError> + Send + Sync>>,
    zmodem: Arc<ZmodemBridge>,
}

impl SshIoSession {
    pub fn spawn(
        reader: Box<dyn Read + Send>,
        writer: Box<dyn Write + Send>,
        cols: u16,
        rows: u16,
        resize: Option<Arc<dyn Fn(u16, u16) -> Result<(), ConnError> + Send + Sync>>,
        initial_output: Vec<u8>,
    ) -> Result<Self, ConnError> {
        let terminal = TerminalHandle::new(cols, rows);
        let zmodem = Arc::new(ZmodemBridge::new());
        if !initial_output.is_empty() {
            // Initial banner is plain text — still run through the gate in case
            // a transfer was already mid-flight (unlikely on fresh connect).
            let rx = zmodem.on_rx(&initial_output);
            if !rx.to_terminal.is_empty() {
                terminal.advance_bytes(&rx.to_terminal);
            }
        }
        let alive = Arc::new(AtomicBool::new(true));
        let alive_reader = Arc::clone(&alive);
        let term_reader = terminal.clone();
        let writer_slot = Arc::new(Mutex::new(Some(writer)));
        let writer_for_reader = Arc::clone(&writer_slot);
        let zmodem_reader = Arc::clone(&zmodem);

        let reader_thread = thread::Builder::new()
            .name("vsterm-ssh-reader".into())
            .spawn(move || {
                let mut reader = reader;
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
                        // The russh PipeReader already waits up to 20 ms for
                        // data. Sleeping after a timeout creates an avoidable
                        // blind window and makes remote echo feel behind.
                        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => continue,
                        Err(_) => break,
                    }
                }
                alive_reader.store(false, Ordering::SeqCst);
            })
            .map_err(|e| ConnError::Term(e.to_string()))?;

        Ok(Self {
            terminal,
            writer: writer_slot,
            alive,
            reader_thread: Some(reader_thread),
            resize,
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

    pub fn write_all(&self, data: &[u8]) -> Result<(), ConnError> {
        if !self.is_alive() {
            return Err(ConnError::NotConnected);
        }
        // Drop keystrokes while a ZMODEM session owns the channel.
        if self.zmodem.is_transferring() {
            return Ok(());
        }
        let mut guard = self.writer.lock();
        let writer = guard.as_mut().ok_or(ConnError::NotConnected)?;
        writer.write_all(data).map_err(ConnError::Io)?;
        writer.flush().map_err(ConnError::Io)?;
        Ok(())
    }

    /// Write raw bytes even during a transfer (protocol ACKs / cancel).
    pub fn write_raw(&self, data: &[u8]) -> Result<(), ConnError> {
        if !self.is_alive() {
            return Err(ConnError::NotConnected);
        }
        let mut guard = self.writer.lock();
        let writer = guard.as_mut().ok_or(ConnError::NotConnected)?;
        writer.write_all(data).map_err(ConnError::Io)?;
        writer.flush().map_err(ConnError::Io)?;
        Ok(())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), ConnError> {
        self.terminal
            .resize(cols, rows)
            .map_err(|e| ConnError::Term(e.to_string()))?;
        if let Some(resize) = &self.resize {
            resize(cols, rows)?;
        }
        Ok(())
    }

    fn shutdown_io(&mut self) {
        self.alive.store(false, Ordering::SeqCst);
        // Dropping the writer closes one end of the shell ctrl channel; also
        // drop `resize` (it holds the other UnboundedSender clone) so the
        // russh shell task observes `None` and exits promptly.
        *self.writer.lock() = None;
        self.resize = None;
        // Detach egui wake hook so the terminal grid can free without
        // retaining UI callbacks across the close path.
        self.terminal.set_output_hook(None);
    }
}

impl Drop for SshIoSession {
    fn drop(&mut self) {
        self.shutdown_io();
        if let Some(handle) = self.reader_thread.take() {
            let (tx, rx) = mpsc::channel();
            thread::spawn(move || {
                let _ = handle.join();
                let _ = tx.send(());
            });
            // Reader uses a 20 ms recv timeout and checks `alive`; give it a
            // moment, then detach so tab close never blocks the UI long.
            let _ = rx.recv_timeout(Duration::from_millis(300));
        }
    }
}
