//! Terminal emulation core: byte stream ↔ character grid via alacritty_terminal.

mod error;
mod local_pty;
mod terminal;

pub use error::TermError;
pub use local_pty::LocalPtySession;
pub use terminal::{CellAttr, Rgb, TerminalHandle, TerminalSnapshot};
