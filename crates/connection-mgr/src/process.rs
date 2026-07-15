//! Unified GUI-safe process spawn helpers.
//!
//! **All** helper subprocesses (`ssh`, `where`, `route`, future tools) must be
//! created via [`command`] — never raw `std::process::Command::new` — so Windows
//! never flashes a console window when the GUI is running.

use std::ffi::OsStr;
use std::process::Command;

/// Create a [`Command`] that will not allocate a visible console on Windows.
///
/// This is the single entry point for connection-mgr / app-ui helper processes.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut cmd = Command::new(program);
    hide_console(&mut cmd);
    cmd
}

/// Apply `CREATE_NO_WINDOW` when a caller already holds a `Command`
/// (e.g. from a third-party builder). Prefer [`command`] for new code.
pub fn hide_console(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = cmd;
}
