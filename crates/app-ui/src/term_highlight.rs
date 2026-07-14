//! Client-side semantic highlighting (WindTerm-style lexer overlay).
//!
//! Only recolors cells that still use the default foreground — remote ANSI
//! colors always win.

use once_cell::sync::Lazy;
use regex::Regex;
use term_core::{CellAttr, Rgb, TerminalSnapshot};

const FG_DEFAULT: Rgb = Rgb::new(230, 230, 230);
const FG_BRIGHT_WHITE: Rgb = Rgb::new(255, 255, 255);

const COLOR_ERROR: Rgb = Rgb::new(255, 95, 95);
const COLOR_WARN: Rgb = Rgb::new(241, 200, 80);
const COLOR_OK: Rgb = Rgb::new(80, 220, 130);
const COLOR_NUMBER: Rgb = Rgb::new(189, 147, 249);
const COLOR_IP: Rgb = Rgb::new(130, 210, 240);

static RE_IP: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:(?:25[0-5]|2[0-4]\d|1?\d?\d)\.){3}(?:25[0-5]|2[0-4]\d|1?\d?\d)\b").unwrap()
});

static RE_NUMBER: Lazy<Regex> = Lazy::new(|| {
    // Integers / decimals; skip bare dots. Hex like 0xdead optional.
    Regex::new(r"(?x)
        \b0[xX][0-9a-fA-F]+\b
        | \b\d+\.\d+\b
        | \b\d+\b
    ").unwrap()
});

static RE_KW_ERROR: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)\b(?:UNKNOWN|DOWN|ERROR|ERR|FAILED|FAIL|FAILURE|FATAL|CRITICAL|DENIED|REFUSED|TIMEOUT|TIMED\s*OUT|OFFLINE|INACTIVE|DEAD|KILLED|CRASHED|INVALID|UNREACHABLE|NO\s*ROUTE)\b",
    )
    .unwrap()
});

static RE_KW_WARN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(?:WARNING|WARN|DEPRECATED|NOTICE|RETRY|PENDING|DEGRADED)\b").unwrap()
});

static RE_KW_OK: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)\b(?:UP|OK|OKAY|SUCCESS|SUCCESSFUL|RUNNING|ONLINE|ACTIVE|LISTENING|ESTABLISHED|CONNECTED|ENABLED|READY|PASSED|COMPLETE|COMPLETED)\b",
    )
    .unwrap()
});

/// Apply Linux-scheme style keyword / number / IP coloring on top of ANSI output.
pub fn apply_semantic(snap: &mut TerminalSnapshot) {
    if snap.cols == 0 || snap.rows == 0 {
        return;
    }
    for row in 0..snap.rows {
        highlight_line(snap, row);
    }
}

fn highlight_line(snap: &mut TerminalSnapshot, row: usize) {
    let start = row * snap.cols;
    let end = (start + snap.cols).min(snap.cells.len());
    if start >= end {
        return;
    }
    let line: String = snap.cells[start..end].iter().map(|c| c.ch).collect();
    if line.trim().is_empty() {
        return;
    }

    // Priority: error > warn > ok > ip > number (non-overlapping).
    let mut spans: Vec<(usize, usize, u8, Rgb)> = Vec::new();
    push_matches(&line, &RE_KW_ERROR, 0, COLOR_ERROR, &mut spans);
    push_matches(&line, &RE_KW_WARN, 1, COLOR_WARN, &mut spans);
    push_matches(&line, &RE_KW_OK, 2, COLOR_OK, &mut spans);
    push_matches(&line, &RE_IP, 3, COLOR_IP, &mut spans);
    push_matches(&line, &RE_NUMBER, 4, COLOR_NUMBER, &mut spans);

    spans.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.2.cmp(&b.2))
            .then_with(|| (b.1 - b.0).cmp(&(a.1 - a.0)))
    });
    let mut taken: Vec<(usize, usize)> = Vec::new();
    for (s, e, _, color) in spans {
        if taken.iter().any(|&(ts, te)| s < te && e > ts) {
            continue;
        }
        taken.push((s, e));
        paint_span(&mut snap.cells[start..end], s, e, color);
    }
}

fn push_matches(
    line: &str,
    re: &Regex,
    prio: u8,
    color: Rgb,
    out: &mut Vec<(usize, usize, u8, Rgb)>,
) {
    for m in re.find_iter(line) {
        let s = byte_to_char(line, m.start());
        let e = byte_to_char(line, m.end());
        if s < e {
            out.push((s, e, prio, color));
        }
    }
}

fn paint_span(cells: &mut [CellAttr], start_char: usize, end_char: usize, color: Rgb) {
    for (i, cell) in cells.iter_mut().enumerate() {
        if i < start_char || i >= end_char {
            continue;
        }
        if !is_default_fg(cell) {
            continue;
        }
        cell.fg = color;
    }
}

fn is_default_fg(cell: &CellAttr) -> bool {
    if cell.inverse {
        return false;
    }
    let fg_ok = cell.fg == FG_DEFAULT || cell.fg == FG_BRIGHT_WHITE;
    fg_ok && is_default_bg(cell.bg)
}

fn is_default_bg(bg: Rgb) -> bool {
    bg == Rgb::new(0, 0, 0)
}

fn byte_to_char(s: &str, byte: usize) -> usize {
    s.get(..byte).map(|p| p.chars().count()).unwrap_or_else(|| s.chars().count())
}

#[cfg(test)]
mod tests {
    use super::*;
    use term_core::CellAttr;

    fn cell(ch: char) -> CellAttr {
        CellAttr {
            ch,
            fg: FG_DEFAULT,
            bg: Rgb::new(0, 0, 0),
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            inverse: false,
        }
    }

    #[test]
    fn colors_unknown_and_number() {
        let line = "state UNKNOWN count 1000";
        let cols = line.chars().count();
        let mut snap = TerminalSnapshot {
            cols,
            rows: 1,
            cells: line.chars().map(cell).collect(),
            cursor: (0, 0),
            cursor_visible: false,
            history_size: 0,
            display_offset: 0,
        };
        apply_semantic(&mut snap);
        let cells = &snap.cells;
        // U of UNKNOWN
        let u_idx = line.find('U').unwrap();
        assert_eq!(cells[u_idx].fg, COLOR_ERROR);
        // 1 of 1000
        let n_idx = line.find('1').unwrap();
        assert_eq!(cells[n_idx].fg, COLOR_NUMBER);
    }

    #[test]
    fn respects_ansi_color() {
        let line = "UNKNOWN";
        let mut snap = TerminalSnapshot {
            cols: 7,
            rows: 1,
            cells: line
                .chars()
                .map(|ch| {
                    let mut c = cell(ch);
                    c.fg = Rgb::new(80, 250, 123); // already green from remote
                    c
                })
                .collect(),
            cursor: (0, 0),
            cursor_visible: false,
            history_size: 0,
            display_offset: 0,
        };
        apply_semantic(&mut snap);
        assert_eq!(snap.cells[0].fg, Rgb::new(80, 250, 123));
    }
}
