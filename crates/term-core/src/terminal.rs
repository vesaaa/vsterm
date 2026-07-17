use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Color, NamedColor, Processor};
use chrono::{NaiveTime, Timelike};
use parking_lot::Mutex;
use std::sync::Arc;

use crate::osc133::{Osc133Filter, Osc133Kind, OscEvent};
use crate::shell_marks::ShellMarks;
use crate::TermError;

/// Invoked (possibly from a PTY reader thread) after the grid accepts new bytes.
pub type OutputHook = Arc<dyn Fn() + Send + Sync>;

/// Default scrollback depth. 10k lines of a wide grid can cost ~40–50 MB per
/// busy session; 5k keeps useful history while roughly halving that ceiling.
const SCROLLBACK_LINES: usize = 5_000;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldControl {
    /// Block is expanded — click to collapse.
    Collapse,
    /// Block is folded — click to expand.
    Expand,
}

/// How to draw the fold tree under an expanded command block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldGuide {
    /// Header row: boxed −/+ ; vertical stem starts below the box when expanded.
    Header,
    /// Middle output row: vertical stem only.
    Middle,
    /// Last output row: vertical stem ending in a right-angle hook.
    End,
}

#[derive(Debug, Clone)]
pub struct GutterInfo {
    pub abs: usize,
    /// Absolute 1-based scrollback line number.
    pub lineno: Option<u32>,
    /// Wall-clock time when this line first appeared.
    pub time_hm: Option<(u8, u8, u8)>,
    pub fold: Option<FoldControl>,
    /// Fold tree stem for expanded blocks (None when folded or unmarked).
    pub fold_guide: Option<FoldGuide>,
    pub block_id: Option<u64>,
    /// Show boxed "···" after the command when folded.
    pub collapsed_mark: bool,
    /// Command text for collapsed summary (also present in cells when expanded).
    pub command: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TerminalSnapshot {
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<CellAttr>,
    pub gutters: Vec<GutterInfo>,
    pub cursor: (usize, usize),
    pub cursor_visible: bool,
    /// Lines above the live screen in alacritty history.
    pub history_size: usize,
    /// Scroll offset from the live bottom into the **virtual** (fold-filtered) line list.
    pub display_offset: usize,
    /// Length of the virtual line list (for scrollbar).
    pub virtual_len: usize,
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
    osc: Osc133Filter,
    marks: ShellMarks,
    /// Last OSC 7 working directory (absolute path), if reported.
    cwd: Option<String>,
    /// Increments on every OSC 7 update (including same path).
    cwd_generation: u64,
    /// Virtual scroll from bottom (fold-aware); kept in sync when no folds.
    view_offset: usize,
    /// First-seen wall-clock time per absolute scrollback line (0 = oldest).
    line_times: Vec<Option<NaiveTime>>,
    /// Cursor abs after the previous advance (for stamping line ranges).
    prev_cursor_abs: usize,
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
                osc: Osc133Filter::new(),
                marks: ShellMarks::new(),
                cwd: None,
                cwd_generation: 0,
                view_offset: 0,
                line_times: Vec::new(),
                prev_cursor_abs: 0,
            })),
            output_hook: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_output_hook(&self, hook: Option<OutputHook>) {
        *self.output_hook.lock() = hook;
    }

    pub fn size(&self) -> (u16, u16) {
        let state = self.inner.lock();
        (state.cols as u16, state.rows as u16)
    }

    pub fn toggle_fold(&self, block_id: u64) -> bool {
        let mut state = self.inner.lock();
        state.marks.toggle_fold(block_id)
    }

    /// Interactive shell cwd from OSC 7, if known.
    pub fn cwd(&self) -> Option<String> {
        self.inner.lock().cwd.clone()
    }

    /// Monotonic counter bumped on each OSC 7 update.
    pub fn cwd_generation(&self) -> u64 {
        self.inner.lock().cwd_generation
    }

    /// Mark a command block when the user submits Enter locally.
    ///
    /// Used when the remote shell does not emit OSC 133. Once any remote OSC
    /// 133 mark is observed, this becomes a no-op so dual-marking is avoided.
    pub fn on_client_enter(&self) {
        let mut state = self.inner.lock();
        if state.marks.remote_osc {
            return;
        }
        if state.term.mode().contains(TermMode::ALT_SCREEN) {
            return;
        }
        let history_size = state.term.history_size();
        let abs = cursor_abs(&state.term, history_size);
        let cmd = line_text(&state.term, abs, history_size, state.cols);
        state.marks.on_output_start(abs, cmd);
        stamp_abs_range(&mut state, abs, abs);
    }

    /// Positive `lines` scrolls into history (up); negative toward the live bottom.
    pub fn scroll_lines(&self, lines: i32) {
        if lines == 0 {
            return;
        }
        let mut state = self.inner.lock();
        if state.term.mode().contains(TermMode::ALT_SCREEN) {
            return;
        }
        let vlen = virtual_len(&state);
        let max_off = vlen.saturating_sub(state.rows);
        let next = (state.view_offset as i32 + lines).clamp(0, max_off as i32) as usize;
        state.view_offset = next;
        sync_alacritty_scroll(&mut state);
    }

    pub fn scroll_page_up(&self) {
        let rows = self.inner.lock().rows as i32;
        self.scroll_lines(rows);
    }

    pub fn scroll_page_down(&self) {
        let rows = self.inner.lock().rows as i32;
        self.scroll_lines(-rows);
    }

    pub fn scroll_to_bottom(&self) {
        let mut state = self.inner.lock();
        state.view_offset = 0;
        state.term.scroll_display(Scroll::Bottom);
    }

    pub fn set_display_offset(&self, offset: usize) {
        let mut state = self.inner.lock();
        if state.term.mode().contains(TermMode::ALT_SCREEN) {
            return;
        }
        let vlen = virtual_len(&state);
        let max_off = vlen.saturating_sub(state.rows);
        state.view_offset = offset.min(max_off);
        sync_alacritty_scroll(&mut state);
    }

    pub fn advance_bytes(&self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        {
            let mut state = self.inner.lock();
            let mut clean = Vec::with_capacity(bytes.len());
            let mut events = Vec::new();
            state.osc.push(bytes, &mut clean, &mut events);

            let hist_before = state.term.history_size();
            let cursor_before = cursor_abs(&state.term, hist_before);

            if !clean.is_empty() {
                let TermState {
                    term, parser, ..
                } = &mut *state;
                parser.advance(term, &clean);
            }

            // Apply OSC events against the post-advance cursor position.
            for ev in events {
                match ev {
                    OscEvent::Mark(mark) => apply_mark(&mut state, mark),
                    OscEvent::Cwd(path) => {
                        state.cwd = Some(path);
                        state.cwd_generation = state.cwd_generation.saturating_add(1);
                    }
                }
            }

            let history_size = state.term.history_size();
            if history_size < hist_before {
                let dropped = hist_before - history_size;
                state.marks.note_history_trim(dropped);
                if dropped >= state.line_times.len() {
                    state.line_times.clear();
                } else {
                    state.line_times.drain(0..dropped);
                }
            }

            // Keep the open block's end at the current cursor (output growth).
            let abs = cursor_abs(&state.term, history_size);
            state.marks.grow_open_to(abs);

            // Stamp every absolute line the cursor crossed (new output rows).
            let from = cursor_before.min(state.prev_cursor_abs).min(abs);
            let to = abs.max(cursor_before);
            stamp_abs_range(&mut state, from, to);
            state.prev_cursor_abs = abs;

            if state.view_offset == 0 {
                state.term.scroll_display(Scroll::Bottom);
            }
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
        let history_size = state.term.history_size();
        let total_abs = history_size + rows;
        let virtual_lines = build_virtual_abs(&state.marks, total_abs);
        let virtual_len = virtual_lines.len();
        let max_off = virtual_len.saturating_sub(rows);
        let view_offset = state.view_offset.min(max_off);

        let start = virtual_len.saturating_sub(rows + view_offset);
        let end = virtual_len.saturating_sub(view_offset);
        let window = &virtual_lines[start..end];

        let cursor_point = state.term.grid().cursor.point;
        let cursor_abs =
            (history_size as i32 + cursor_point.line.0).max(0) as usize;
        // Don't number empty screen rows below the live cursor / last written line.
        let content_extent = content_extent_abs(&state, cursor_abs);

        let mut cells = Vec::with_capacity(cols * rows);
        let mut gutters = Vec::with_capacity(rows);

        for (view_row, &abs) in window.iter().enumerate() {
            let line = Line(abs as i32 - history_size as i32);
            for col in 0..cols {
                let cell = &state.term.grid()[Point::new(line, Column(col))];
                let ch = cell.c;
                let flags = cell.flags;
                let fg_color = brighten_if_bold(cell.fg, flags.contains(Flags::BOLD));
                cells.push(CellAttr {
                    ch: if ch == '\0' { ' ' } else { ch },
                    fg: resolve_color(fg_color, false),
                    bg: resolve_color(cell.bg, true),
                    bold: flags.contains(Flags::BOLD),
                    dim: flags.contains(Flags::DIM),
                    italic: flags.contains(Flags::ITALIC),
                    underline: flags.contains(Flags::UNDERLINE),
                    inverse: flags.contains(Flags::INVERSE),
                });
            }
            gutters.push(gutter_for_abs(
                &state.marks,
                &state.line_times,
                abs,
                content_extent,
            ));
            let _ = view_row;
        }
        // Pad if virtual window shorter than screen (startup).
        while gutters.len() < rows {
            for _ in 0..cols {
                cells.push(CellAttr {
                    ch: ' ',
                    fg: Rgb::new(230, 230, 230),
                    bg: Rgb::new(0, 0, 0),
                    bold: false,
                    dim: false,
                    italic: false,
                    underline: false,
                    inverse: false,
                });
            }
            gutters.push(GutterInfo {
                abs: 0,
                lineno: None,
                time_hm: None,
                fold: None,
                fold_guide: None,
                block_id: None,
                collapsed_mark: false,
                command: None,
            });
        }

        let cursor_visible = state.term.mode().contains(TermMode::SHOW_CURSOR)
            && view_offset == 0;
        let cursor = window
            .iter()
            .position(|&a| a == cursor_abs)
            .map(|r| (cursor_point.column.0.min(cols.saturating_sub(1)), r))
            .unwrap_or((0, 0));

        TerminalSnapshot {
            cols,
            rows,
            cells,
            gutters,
            cursor,
            cursor_visible,
            history_size,
            display_offset: view_offset,
            virtual_len,
        }
    }
}

/// Highest absolute line that should show gutter chrome (lineno / time).
fn content_extent_abs(state: &TermState, cursor_abs: usize) -> usize {
    let mut extent = cursor_abs;
    if let Some(last) = state.line_times.iter().rposition(|t| t.is_some()) {
        extent = extent.max(last);
    }
    for b in state.marks.blocks() {
        extent = extent.max(b.end_abs.max(b.header_abs));
    }
    extent
}

fn gutter_for_abs(
    marks: &ShellMarks,
    line_times: &[Option<NaiveTime>],
    abs: usize,
    content_extent: usize,
) -> GutterInfo {
    if abs > content_extent {
        return GutterInfo {
            abs,
            lineno: None,
            time_hm: None,
            fold: None,
            fold_guide: None,
            block_id: None,
            collapsed_mark: false,
            command: None,
        };
    }

    let time_hm = line_times.get(abs).and_then(|t| {
        t.map(|tm| {
            (
                tm.hour() as u8,
                tm.minute() as u8,
                tm.second() as u8,
            )
        })
    });
    // Continuous absolute scrollback line numbers (1-based).
    let lineno = Some((abs + 1) as u32);

    let covering = marks.block_covering(abs);
    let header = marks.header_at(abs);

    let (fold, fold_guide, block_id, collapsed_mark, command) = if let Some(b) = header {
        let foldable = b.end_abs > b.header_abs;
        let fold = if foldable {
            Some(if b.folded {
                FoldControl::Expand
            } else {
                FoldControl::Collapse
            })
        } else {
            None
        };
        let fold_guide = if foldable && !b.folded {
            Some(FoldGuide::Header)
        } else {
            None
        };
        (
            fold,
            fold_guide,
            Some(b.id),
            foldable && b.folded,
            Some(b.command.clone()),
        )
    } else if let Some(b) = covering.filter(|b| !b.folded) {
        let guide = if abs == b.end_abs {
            FoldGuide::End
        } else {
            FoldGuide::Middle
        };
        (None, Some(guide), Some(b.id), false, None)
    } else {
        (None, None, None, false, None)
    };

    let time_hm = time_hm.or_else(|| {
        header.map(|b| {
            (
                b.time.hour() as u8,
                b.time.minute() as u8,
                b.time.second() as u8,
            )
        })
    });

    GutterInfo {
        abs,
        lineno,
        time_hm,
        fold,
        fold_guide,
        block_id,
        collapsed_mark,
        command,
    }
}

/// First-write wall-clock stamp for absolute lines in `[from, to]` (inclusive).
fn stamp_abs_range(state: &mut TermState, from: usize, to: usize) {
    if to < from {
        return;
    }
    let now = chrono::Local::now().time();
    if state.line_times.len() <= to {
        state.line_times.resize(to + 1, None);
    }
    for abs in from..=to {
        if state.line_times[abs].is_none() {
            state.line_times[abs] = Some(now);
        }
    }
}

fn apply_mark(state: &mut TermState, mark: Osc133Kind) {
    let history_size = state.term.history_size();
    let abs = cursor_abs(&state.term, history_size);
    match mark {
        Osc133Kind::OutputStart => {
            state.marks.remote_osc = true;
            // Command text usually sits on the line above after Enter.
            let header_abs = abs.saturating_sub(1);
            let cmd = line_text(&state.term, header_abs, history_size, state.cols);
            let cmd = if cmd.is_empty() {
                line_text(&state.term, abs, history_size, state.cols)
            } else {
                cmd
            };
            state.marks.on_output_start(header_abs, cmd);
        }
        Osc133Kind::CommandEnd { exit } => {
            state.marks.remote_osc = true;
            let end = abs.saturating_sub(1).max(0);
            state.marks.on_command_end(end, exit);
        }
        Osc133Kind::PromptStart | Osc133Kind::InputStart => {
            // Prompt begins: finalize any open block on the line above.
            if state.marks.has_open() {
                let end = abs.saturating_sub(1);
                state.marks.on_command_end(end, None);
            }
        }
        Osc133Kind::Property => {}
    }
}

fn cursor_abs(term: &Term<VoidListener>, history_size: usize) -> usize {
    let line = term.grid().cursor.point.line.0;
    (history_size as i32 + line).max(0) as usize
}

fn line_text(
    term: &Term<VoidListener>,
    abs: usize,
    history_size: usize,
    cols: usize,
) -> String {
    let line = Line(abs as i32 - history_size as i32);
    let mut s = String::with_capacity(cols);
    for col in 0..cols {
        let ch = term.grid()[Point::new(line, Column(col))].c;
        s.push(if ch == '\0' { ' ' } else { ch });
    }
    s.trim_end().to_string()
}

fn build_virtual_abs(marks: &ShellMarks, total_abs: usize) -> Vec<usize> {
    if marks.blocks().is_empty() {
        return (0..total_abs).collect();
    }
    let blocks = marks.blocks();
    let mut out = Vec::with_capacity(total_abs);
    let mut bi = 0usize;
    for abs in 0..total_abs {
        while bi + 1 < blocks.len() && blocks[bi + 1].header_abs <= abs {
            bi += 1;
        }
        let folded = blocks[bi].header_abs <= abs
            && blocks[bi].folded
            && abs > blocks[bi].header_abs
            && abs <= blocks[bi].end_abs;
        if !folded {
            out.push(abs);
        }
    }
    out
}

fn virtual_len(state: &TermState) -> usize {
    let total = state.term.history_size() + state.rows;
    build_virtual_abs(&state.marks, total).len()
}

fn sync_alacritty_scroll(state: &mut TermState) {
    // Keep underlying grid near the same region for cursor metrics; virtual view
    // is authoritative for painting when folds exist.
    let hist = state.term.history_size();
    let cur = state.term.grid().display_offset();
    let target = state.view_offset.min(hist);
    let delta = target as i32 - cur as i32;
    if delta != 0 {
        state.term.scroll_display(Scroll::Delta(delta));
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
        NamedColor::Black | NamedColor::Background if is_bg => Rgb::new(0, 0, 0),
        NamedColor::Black => Rgb::new(80, 80, 80),
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
        8 => Rgb::new(98, 114, 164),
        9 => Rgb::new(255, 110, 110),
        10 => Rgb::new(105, 255, 150),
        11 => Rgb::new(255, 255, 170),
        12 => Rgb::new(160, 240, 255),
        13 => Rgb::new(255, 160, 220),
        14 => Rgb::new(160, 240, 255),
        15 => Rgb::new(255, 255, 255),
        16..=231 => {
            let n = idx - 16;
            let r = n / 36;
            let g = (n % 36) / 6;
            let b = n % 6;
            let map = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
            Rgb::new(map(r), map(g), map(b))
        }
        232..=255 => {
            let v = 8 + 10 * (idx - 232);
            Rgb::new(v, v, v)
        }
    }
}

