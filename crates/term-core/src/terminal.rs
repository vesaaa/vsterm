use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Color, NamedColor, Processor};
use parking_lot::Mutex;
use std::sync::Arc;

use crate::TermError;

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
        let term = Term::new(Config::default(), &size, VoidListener);
        Self {
            inner: Arc::new(Mutex::new(TermState {
                term,
                parser: Processor::new(),
                cols: cols_usize,
                rows: rows_usize,
            })),
        }
    }

    pub fn size(&self) -> (u16, u16) {
        let state = self.inner.lock();
        (state.cols as u16, state.rows as u16)
    }

    pub fn advance_bytes(&self, bytes: &[u8]) {
        let mut state = self.inner.lock();
        let TermState { term, parser, .. } = &mut *state;
        parser.advance(term, bytes);
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
        let mut cells = Vec::with_capacity(cols * rows);

        for line in 0..rows {
            for col in 0..cols {
                let point = Point::new(Line(line as i32), Column(col));
                let cell = &state.term.grid()[point];
                let ch = cell.c;
                let flags = cell.flags;
                let fg = resolve_color(cell.fg, false);
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

        let cursor = state.term.grid().cursor.point;
        let cursor_visible = state.term.mode().contains(TermMode::SHOW_CURSOR);

        TerminalSnapshot {
            cols,
            rows,
            cells,
            cursor: (cursor.column.0, cursor.line.0.max(0) as usize),
            cursor_visible,
        }
    }
}

impl Clone for TerminalHandle {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
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

fn named_to_rgb(named: NamedColor, is_bg: bool) -> Rgb {
    match named {
        NamedColor::Black | NamedColor::Background if is_bg => Rgb::new(40, 42, 54),
        NamedColor::Black => Rgb::new(33, 34, 44),
        NamedColor::Red => Rgb::new(255, 85, 85),
        NamedColor::Green => Rgb::new(80, 250, 123),
        NamedColor::Yellow => Rgb::new(241, 250, 140),
        NamedColor::Blue => Rgb::new(139, 233, 253),
        NamedColor::Magenta => Rgb::new(255, 121, 198),
        NamedColor::Cyan => Rgb::new(139, 233, 253),
        NamedColor::White | NamedColor::Foreground => Rgb::new(248, 248, 242),
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
                Rgb::new(40, 42, 54)
            } else {
                Rgb::new(248, 248, 242)
            }
        }
    }
}

fn indexed_to_rgb(idx: u8) -> Rgb {
    match idx {
        0 => Rgb::new(33, 34, 44),
        1 => Rgb::new(255, 85, 85),
        2 => Rgb::new(80, 250, 123),
        3 => Rgb::new(241, 250, 140),
        4 => Rgb::new(139, 233, 253),
        5 => Rgb::new(255, 121, 198),
        6 => Rgb::new(139, 233, 253),
        7 => Rgb::new(248, 248, 242),
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
