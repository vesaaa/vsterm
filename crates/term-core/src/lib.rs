//! Terminal emulation core: byte stream ↔ character grid via alacritty_terminal.

mod error;
mod local_pty;
mod osc133;
mod shell_marks;
mod terminal;
mod zmodem;

pub use error::TermError;
pub use local_pty::LocalPtySession;
pub use shell_marks::CommandBlock;
pub use terminal::{
    CellAttr, FoldControl, FoldGuide, GutterInfo, OutputHook, Rgb, TerminalHandle, TerminalSnapshot,
};
pub use zmodem::{RxResult, ZmodemBridge, ZmodemStatus};
