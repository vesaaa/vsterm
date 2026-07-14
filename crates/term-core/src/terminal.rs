use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Point};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{point_to_viewport, viewport_to_point, Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Color, NamedColor, Processor};
use parking_lot::Mutex;
use std::sync::Arc;

use crate::TermError;

/// Invoked (possibly from a PTY reader thread) after the grid accepts new bytes.
pub type OutputHook = Arc<dyn Fn() + Send + Sync>;

const SCROLLBACK_LINES: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[derive(Debug, Clone)]
pub struct CellAttr {
    pub ch: char,
    pub fg: Rgb,
    pub bg: Rgb,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
}

#[derive(Debug, Clone)]
pub struct TerminalSnapshot {
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<CellAttr>,
    pub cursor: (usize, usize),
    pub cursor_visible: bool,
    /// Invisible scrollback lines currently available.
    pub history_size: usize,
    /// How far the viewport is scrolled into history (0 = live bottom).
    pub display_offset: usize,
}

struct TermSize {
    cols: usize,
    rows: usize,
}

impl Dimensions for TermSize {
    fn total_lines(&self) -> usize {
        self.screen_lines()
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

/// Thread-safe terminal emulator wrapping alacritty_terminal::Term.
pub struct TerminalHandle {
    inner: Arc<Mutex<TermState>>,
    output_hook: Arc<Mutex<Option<OutputHook>>>,
}

struct TermState {
    term: Term<VoidListener>,
    parser: Processor,
    cols: usize,
    rows: usize,
}

impl TerminalHandle {
    pub fn new(cols: u16, rows: u16) -> Self {
        let cols_usize = cols.max(1) as usize;
        let rows_usize = rows.max(1) as usize;
        let size = TermSize {
            cols: cols_usize,
            rows: rows_usize,
        };
        let mut config = Config::default();
        config.scrolling_history = SCROLLBACK_LINES;
        let term = Term::new(config, &size, VoidListener);
        Self {
            inner: Arc::new(Mutex::new(TermState {
                term,
                parser: Processor::new(),
                cols: cols_usize,
                rows: rows_usize,
            })),
            output_hook: Arc::new(Mutex::new(None)),
        }
    }

    /// UI sets this so PTY reader threads can wake the egui event loop.
    pub fn set_output_hook(&self, hook: Option<OutputHook>) {
        *self.output_hook.lock() = hook;
    }

    pub fn size(&self) -> (u16, u16) {
        let state = self.inner.lock();
        (state.cols as u16, state.rows as u16)
    }

    /// Positive `lines` scrolls into history (up); negative toward the live bottom.
    /// No-op on the alternate screen where the remote application owns the viewport.
    pub fn scroll_lines(&self, lines: i32) {
        if lines == 0 {
            return;
        }
        let mut state = self.inner.lock();
        if state.term.mode().contains(TermMode::ALT_SCREEN) {
            return;
        }
        state.term.scroll_display(Scroll::Delta(lines));
    }

    pub fn scroll_page_up(&self) {
        let mut state = self.inner.lock();
        if state.term.mode().contains(TermMode::ALT_SCREEN) {
            return;
        }
        state.term.scroll_display(Scroll::PageUp);
    }

    pub fn scroll_page_down(&self) {
        let mut state = self.inner.lock();
        if state.term.mode().contains(TermMode::ALT_SCREEN) {
            return;
        }
        state.term.scroll_display(Scroll::PageDown);
    }

    pub fn scroll_to_bottom(&self) {
        let mut state = self.inner.lock();
        state.term.scroll_display(Scroll::Bottom);
    }

    /// Absolute display offset into scrollback (clamped).
    pub fn set_display_offset(&self, offset: usize) {
        let mut state = self.inner.lock();
        if state.term.mode().contains(TermMode::ALT_SCREEN) {
            return;
        }
        let cur = state.term.grid().display_offset();
        let delta = offset as i32 - cur as i32;
        if delta != 0 {
            state.term.scroll_display(Scroll::Delta(delta));
        }
    }

    pub fn advance_bytes(&self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        {
            let mut state = self.inner.lock();
            let TermState { term, parser, .. } = &mut *state;
            parser.advance(term, bytes);
        }
        if let Some(hook) = self.output_hook.lock().clone() {
            hook();
        }
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), TermError> {
        let cols_usize = cols.max(1) as usize;
        let rows_usize = rows.max(1) as usize;
        let size = TermSize {
            cols: cols_usize,
            rows: rows_usize,
        };
        let mut state = self.inner.lock();
        state.cols = cols_usize;
        state.rows = rows_usize;
        state.term.resize(size);
        Ok(())
    }

    pub fn snapshot(&self) -> TerminalSnapshot {
        let state = self.inner.lock();
        let cols = state.cols;
        let rows = state.rows;
        let display_offset = state.term.grid().display_offset();
        let history_size = state.term.history_size();
        let mut cells = Vec::with_capacity(cols * rows);

        for line in 0..rows {
            for col in 0..cols {
                let point =
                    viewport_to_point(display_offset, Point::new(line, Column(col)));
                let cell = &state.term.grid()[point];
                let ch = cell.c;
                let flags = cell.flags;
                // Many CLI tools use bold as "bright color"; map that when rendering.
                let fg_color = brighten_if_bold(cell.fg, flags.contains(Flags::BOLD));
                let fg = resolve_color(fg_color, false);
                let bg = resolve_color(cell.bg, true);
                cells.push(CellAttr {
                    ch: if ch == '\0' { ' ' } else { ch },
                    fg,
                    bg,
                    bold: flags.contains(Flags::BOLD),
                    dim: flags.contains(Flags::DIM),
                    italic: flags.contains(Flags::ITALIC),
                    underline: flags.contains(Flags::UNDERLINE),
                    inverse: flags.contains(Flags::INVERSE),
                });
            }
        }

        let cursor_point = state.term.grid().cursor.point;
        let cursor_visible = state.term.mode().contains(TermMode::SHOW_CURSOR)
            && display_offset == 0;
        let cursor = point_to_viewport(display_offset, cursor_point)
            .map(|p| (p.column.0, p.line))
            .unwrap_or((0, 0));

        TerminalSnapshot {
            cols,
            rows,
            cells,
            cursor,
            cursor_visible,
            history_size,
            display_offset,
        }
    }
}

impl Clone for TerminalHandle {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            output_hook: Arc::clone(&self.output_hook),
        }
    }
}

fn resolve_color(color: Color, is_bg: bool) -> Rgb {
    match color {
        Color::Named(named) => named_to_rgb(named, is_bg),
        Color::Spec(rgb) => Rgb::new(rgb.r, rgb.g, rgb.b),
        Color::Indexed(idx) => indexed_to_rgb(idx),
    }
}

fn brighten_if_bold(color: Color, bold: bool) -> Color {
    if !bold {
        return color;
    }
    match color {
        Color::Named(named) => Color::Named(match named {
            NamedColor::Black => NamedColor::BrightBlack,
            NamedColor::Red => NamedColor::BrightRed,
            NamedColor::Green => NamedColor::BrightGreen,
            NamedColor::Yellow => NamedColor::BrightYellow,
            NamedColor::Blue => NamedColor::BrightBlue,
            NamedColor::Magenta => NamedColor::BrightMagenta,
            NamedColor::Cyan => NamedColor::BrightCyan,
            NamedColor::White => NamedColor::BrightWhite,
            other => other,
        }),
        Color::Indexed(idx) if idx < 8 => Color::Indexed(idx + 8),
        other => other,
    }
}

fn named_to_rgb(named: NamedColor, is_bg: bool) -> Rgb {
    match named {
        // Pure black terminal background (WindTerm-style).
        NamedColor::Black | NamedColor::Background if is_bg => Rgb::new(0, 0, 0),
        NamedColor::Black => Rgb::new(80, 80, 80), // visible "black" text on black bg
        NamedColor::Red => Rgb::new(255, 85, 85),
        NamedColor::Green => Rgb::new(80, 250, 123),
        NamedColor::Yellow => Rgb::new(241, 250, 140),
        NamedColor::Blue => Rgb::new(139, 233, 253),
        NamedColor::Magenta => Rgb::new(255, 121, 198),
        NamedColor::Cyan => Rgb::new(139, 233, 253),
        NamedColor::White | NamedColor::Foreground => Rgb::new(230, 230, 230),
        NamedColor::BrightBlack => Rgb::new(98, 114, 164),
        NamedColor::BrightRed => Rgb::new(255, 110, 110),
        NamedColor::BrightGreen => Rgb::new(105, 255, 150),
        NamedColor::BrightYellow => Rgb::new(255, 255, 170),
        NamedColor::BrightBlue => Rgb::new(160, 240, 255),
        NamedColor::BrightMagenta => Rgb::new(255, 160, 220),
        NamedColor::BrightCyan => Rgb::new(160, 240, 255),
        NamedColor::BrightWhite => Rgb::new(255, 255, 255),
        _ => {
            if is_bg {
                Rgb::new(0, 0, 0)
            } else {
                Rgb::new(230, 230, 230)
            }
        }
    }
}

fn indexed_to_rgb(idx: u8) -> Rgb {
    match idx {
        0 => Rgb::new(0, 0, 0),
        1 => Rgb::new(255, 85, 85),
        2 => Rgb::new(80, 250, 123),
        3 => Rgb::new(241, 250, 140),
        4 => Rgb::new(139, 233, 253),
        5 => Rgb::new(255, 121, 198),
        6 => Rgb::new(139, 233, 253),
        7 => Rgb::new(230, 230, 230),
        8..=15 => named_to_rgb(
            match idx {
                8 => NamedColor::BrightBlack,
                9 => NamedColor::BrightRed,
                10 => NamedColor::BrightGreen,
                11 => NamedColor::BrightYellow,
                12 => NamedColor::BrightBlue,
                13 => NamedColor::BrightMagenta,
                14 => NamedColor::BrightCyan,
                _ => NamedColor::BrightWhite,
            },
            false,
        ),
        16..=231 => {
            let n = idx - 16;
            let r = n / 36;
            let g = (n % 36) / 6;
            let b = n % 6;
            let conv = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            Rgb::new(conv(r), conv(g), conv(b))
        }
        232..=255 => {
            let gray = 8 + (idx - 232) * 10;
            Rgb::new(gray, gray, gray)
        }
    }
}
