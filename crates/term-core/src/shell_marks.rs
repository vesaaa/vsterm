//! Command blocks driven by OSC 133 marks (wall-clock time + fold ranges).

use chrono::{Local, NaiveTime};

#[derive(Debug, Clone)]
pub struct CommandBlock {
    pub id: u64,
    /// Monotonic line label shown in the gutter (WindTerm-style).
    pub lineno: u32,
    /// Local wall-clock time when the command started (OSC 133 C).
    pub time: NaiveTime,
    /// Absolute scrollback index of the command/header line (0 = oldest).
    pub header_abs: usize,
    /// Inclusive absolute index of the last output line (same as header until closed).
    pub end_abs: usize,
    pub command: String,
    pub folded: bool,
    pub exit: Option<i32>,
}

#[derive(Debug, Default)]
pub struct ShellMarks {
    blocks: Vec<CommandBlock>,
    next_id: u64,
    next_lineno: u32,
    /// Header abs while waiting for D (open block).
    open: Option<u64>,
    /// True after any OSC 133 mark from the PTY — client Enter marks are skipped.
    pub remote_osc: bool,
}

impl ShellMarks {
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            next_id: 1,
            next_lineno: 1,
            open: None,
            remote_osc: false,
        }
    }

    #[allow(dead_code)] // reserved for future mark queries / UI tooling
    pub fn blocks(&self) -> &[CommandBlock] {
        &self.blocks
    }

    pub fn block_mut(&mut self, id: u64) -> Option<&mut CommandBlock> {
        self.blocks.iter_mut().find(|b| b.id == id)
    }

    pub fn toggle_fold(&mut self, id: u64) -> bool {
        if let Some(b) = self.block_mut(id) {
            if b.end_abs > b.header_abs {
                b.folded = !b.folded;
                return true;
            }
        }
        false
    }

    pub fn has_open(&self) -> bool {
        self.open.is_some()
    }

    /// Extend the open block's end as more output lands on screen.
    ///
    /// Uses `abs.saturating_sub(1)` so the live cursor line (usually the next
    /// shell prompt) is not included — the fold stem ends on the last output
    /// line above the current prompt.
    pub fn grow_open_to(&mut self, abs: usize) {
        let Some(id) = self.open else {
            return;
        };
        if let Some(b) = self.block_mut(id) {
            let end = abs.saturating_sub(1).max(b.header_abs);
            if end > b.end_abs {
                b.end_abs = end;
            }
        }
    }

    /// OSC 133 C — command begins; `header_abs` is the line holding the typed command.
    pub fn on_output_start(&mut self, header_abs: usize, command: String) {
        // Close any dangling open block at the previous line.
        if let Some(id) = self.open.take() {
            if let Some(b) = self.block_mut(id) {
                if b.end_abs < header_abs {
                    b.end_abs = header_abs.saturating_sub(1).max(b.header_abs);
                }
            }
        }
        let id = self.next_id;
        self.next_id += 1;
        let lineno = self.next_lineno;
        self.next_lineno = self.next_lineno.saturating_add(1);
        self.blocks.push(CommandBlock {
            id,
            lineno,
            time: Local::now().time(),
            header_abs,
            end_abs: header_abs,
            command,
            folded: false,
            exit: None,
        });
        self.open = Some(id);
        // Cap memory for long sessions.
        if self.blocks.len() > 2_000 {
            let drop_n = self.blocks.len() - 2_000;
            self.blocks.drain(0..drop_n);
        }
    }

    /// OSC 133 D — extend open block to `end_abs` (last line of output).
    pub fn on_command_end(&mut self, end_abs: usize, exit: Option<i32>) {
        let Some(id) = self.open.take() else {
            return;
        };
        if let Some(b) = self.block_mut(id) {
            b.end_abs = end_abs.max(b.header_abs);
            b.exit = exit;
        }
    }

    /// Find a block whose header is `abs`, if any.
    pub fn header_at(&self, abs: usize) -> Option<&CommandBlock> {
        self.blocks.iter().find(|b| b.header_abs == abs)
    }

    /// Find an expanded (or foldable) block that covers `abs` (header..=end).
    pub fn block_covering(&self, abs: usize) -> Option<&CommandBlock> {
        self.blocks
            .iter()
            .find(|b| abs >= b.header_abs && abs <= b.end_abs && b.end_abs > b.header_abs)
    }

    /// True if `abs` is output inside a folded block (not the header).
    pub fn is_folded_away(&self, abs: usize) -> bool {
        self.blocks.iter().any(|b| {
            b.folded && abs > b.header_abs && abs <= b.end_abs
        })
    }

    /// Adjust absolute indices after history is truncated at the top.
    #[allow(dead_code)] // wired when scrollback rotisserie detection is added
    pub fn note_history_trim(&mut self, dropped: usize) {
        if dropped == 0 {
            return;
        }
        for b in &mut self.blocks {
            b.header_abs = b.header_abs.saturating_sub(dropped);
            b.end_abs = b.end_abs.saturating_sub(dropped);
        }
        self.blocks.retain(|b| b.end_abs > 0 || b.header_abs > 0);
    }
}
