//! SSH interactive I/O bridged to a [`TerminalHandle`] (reader thread + writer).

use crate::error::ConnError;
use parking_lot::Mutex;
use portable_pty::Child as PtyChild;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use term_core::TerminalHandle;

/// Keeps SSH shell channel I/O alive and feeds the terminal grid.
pub struct SshIoSession {
    terminal: TerminalHandle,
    writer: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
    alive: Arc<AtomicBool>,
    /// Shared ssh child — `try_wait` detects remote `exit` even if ConPTY read stalls.
    child: Option<Arc<Mutex<Box<dyn PtyChild + Send + Sync>>>>,
    reader_thread: Option<JoinHandle<()>>,
    resize: Option<Arc<dyn Fn(u16, u16) -> Result<(), ConnError> + Send + Sync>>,
}

impl SshIoSession {
    pub fn spawn(
        reader: Box<dyn Read + Send>,
        writer: Box<dyn Write + Send>,
        cols: u16,
        rows: u16,
        resize: Option<Arc<dyn Fn(u16, u16) -> Result<(), ConnError> + Send + Sync>>,
        initial_output: Vec<u8>,
        child: Option<Arc<Mutex<Box<dyn PtyChild + Send + Sync>>>>,
    ) -> Result<Self, ConnError> {
        let terminal = TerminalHandle::new(cols, rows);
        if !initial_output.is_empty() {
            terminal.advance_bytes(&initial_output);
        }
        let alive = Arc::new(AtomicBool::new(true));
        let alive_reader = Arc::clone(&alive);
        let term_reader = terminal.clone();
        let writer_slot = Arc::new(Mutex::new(Some(writer)));
        let child_wait = child.clone();

        let reader_thread = thread::Builder::new()
            .name("vsterm-ssh-reader".into())
            .spawn(move || {
                let mut reader = reader;
                let mut buf = [0u8; 8192];
                while alive_reader.load(Ordering::SeqCst) {
                    // Detect ssh.exe exit even when the PTY read keeps returning WouldBlock.
                    if let Some(child) = &child_wait {
                        if let Some(mut g) = child.try_lock() {
                            match g.try_wait() {
                                Ok(Some(_)) => {
                                    alive_reader.store(false, Ordering::SeqCst);
                                    break;
                                }
                                Ok(None) => {}
                                Err(_) => {
                                    alive_reader.store(false, Ordering::SeqCst);
                                    break;
                                }
                            }
                        }
                    }
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => term_reader.advance_bytes(&buf[..n]),
                        Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                        // The russh PipeReader already waits up to 20 ms for
                        // data. Sleeping another 15 ms after a timeout creates
                        // an avoidable blind window: output arriving there
                        // cannot wake this reader and makes remote echo feel
                        // noticeably behind the keyboard.
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
            child,
            reader_thread: Some(reader_thread),
            resize,
        })
    }

    pub fn terminal(&self) -> &TerminalHandle {
        &self.terminal
    }

    pub fn is_alive(&self) -> bool {
        if !self.alive.load(Ordering::SeqCst) {
            return false;
        }
        if let Some(child) = &self.child {
            if let Some(mut g) = child.try_lock() {
                match g.try_wait() {
                    Ok(Some(_)) => {
                        self.alive.store(false, Ordering::SeqCst);
                        return false;
                    }
                    Ok(None) => {}
                    Err(_) => {
                        self.alive.store(false, Ordering::SeqCst);
                        return false;
                    }
                }
            }
        }
        true
    }

    pub fn write_all(&self, data: &[u8]) -> Result<(), ConnError> {
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
        *self.writer.lock() = None;
        if let Some(child) = &self.child {
            if let Some(mut g) = child.try_lock() {
                let _ = g.kill();
            }
        }
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
            let _ = rx.recv_timeout(Duration::from_millis(150));
        }
    }
}
