use connection_mgr::ConnectionManager;
use connection_mgr::ConnectionState;
use egui::{Color32, FontId, Rect, Sense, Ui, Vec2};
use std::sync::Arc;
use term_core::Rgb;

const CELL_W: f32 = 8.4;
const CELL_H: f32 = 16.0;
const TERM_BG: Color32 = Color32::BLACK;

pub struct TerminalView;

impl TerminalView {
    /// Renders the active terminal. Returns desired (cols, rows) based on available space.
    pub fn show(ui: &mut Ui, mgr: &Arc<ConnectionManager>) -> (u16, u16) {
        let avail = ui.available_size();
        let cols = ((avail.x / CELL_W).floor() as u16).max(20);
        let rows = ((avail.y / CELL_H).floor() as u16).max(5);

        let (rect, resp) = ui.allocate_exact_size(avail, Sense::click_and_drag());
        ui.painter().rect_filled(rect, 0.0, TERM_BG);

        let snapshot = mgr
            .with_active(|c| {
                if c.state == ConnectionState::Connecting || c.state == ConnectionState::Failed {
                    return None;
                }
                Some(c.terminal.snapshot())
            })
            .flatten();

        let Some(mut snapshot) = snapshot else {
            if let Some(state) = mgr.with_active(|c| c.state) {
                let (msg, color) = match state {
                    ConnectionState::Connecting => (
                        crate::i18n::t("term.connecting"),
                        Color32::from_rgb(152, 158, 180),
                    ),
                    ConnectionState::Failed => (
                        mgr.with_active(|c| c.error_message.clone())
                            .flatten()
                            .unwrap_or_else(|| crate::i18n::t("term.failed")),
                        Color32::from_rgb(255, 120, 120),
                    ),
                    _ => (
                        crate::i18n::t("term.empty"),
                        Color32::from_rgb(152, 158, 180),
                    ),
                };
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    msg,
                    FontId::proportional(15.0),
                    color,
                );
            } else {
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    crate::i18n::t("term.empty"),
                    FontId::proportional(16.0),
                    Color32::from_rgb(152, 158, 180),
                );
            }
            return (cols, rows);
        };

        crate::term_highlight::apply_semantic(&mut snapshot);

        let font = FontId::monospace(13.0);
        let max_rows = snapshot.rows.min(rows as usize);
        let max_cols = snapshot.cols.min(cols as usize);

        for row in 0..max_rows {
            for col in 0..max_cols {
                let idx = row * snapshot.cols + col;
                let Some(cell) = snapshot.cells.get(idx) else {
                    continue;
                };
                let mut fg = to_color32(cell.fg);
                let mut bg = to_color32(cell.bg);
                if cell.inverse {
                    std::mem::swap(&mut fg, &mut bg);
                }
                if cell.dim {
                    fg = Color32::from_rgb(
                        (fg.r() as u16 * 2 / 3) as u8,
                        (fg.g() as u16 * 2 / 3) as u8,
                        (fg.b() as u16 * 2 / 3) as u8,
                    );
                }

                let cell_rect = Rect::from_min_size(
                    rect.min + Vec2::new(col as f32 * CELL_W, row as f32 * CELL_H),
                    Vec2::new(CELL_W, CELL_H),
                );
                if bg != TERM_BG {
                    ui.painter().rect_filled(cell_rect, 0.0, bg);
                }
                if cell.ch != ' ' {
                    let mut text = egui::RichText::new(cell.ch.to_string())
                        .font(font.clone())
                        .color(fg);
                    if cell.bold {
                        text = text.strong();
                    }
                    // Use painter text for performance
                    ui.painter().text(
                        cell_rect.left_top() + Vec2::new(0.0, 1.0),
                        egui::Align2::LEFT_TOP,
                        cell.ch.to_string(),
                        font.clone(),
                        fg,
                    );
                    let _ = text;
                }
            }
        }

        if snapshot.cursor_visible {
            let (cx, cy) = snapshot.cursor;
            if cy < max_rows && cx < max_cols {
                let cursor_rect = Rect::from_min_size(
                    rect.min + Vec2::new(cx as f32 * CELL_W, cy as f32 * CELL_H),
                    Vec2::new(CELL_W, CELL_H),
                );
                ui.painter()
                    .rect_filled(cursor_rect, 0.0, Color32::from_rgba_unmultiplied(248, 248, 242, 160));
            }
        }

        // Focus + keyboard input
        if resp.clicked() {
            resp.request_focus();
        }
        if resp.has_focus() || resp.hovered() {
            let can_input = mgr
                .with_active(|c| c.state == ConnectionState::Connected)
                .unwrap_or(false);
            if can_input {
                ui.input(|i| {
                    for event in &i.events {
                        if let Some(bytes) = event_to_bytes(event) {
                            let _ = mgr.write_to_active(&bytes);
                        }
                    }
                });
            }
        }

        (cols, rows)
    }
}

fn to_color32(c: Rgb) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

fn event_to_bytes(event: &egui::Event) -> Option<Vec<u8>> {
    match event {
        egui::Event::Text(text) => Some(text.as_bytes().to_vec()),
        egui::Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => key_to_bytes(*key, modifiers),
        egui::Event::Paste(text) => Some(text.as_bytes().to_vec()),
        _ => None,
    }
}

fn key_to_bytes(key: egui::Key, modifiers: &egui::Modifiers) -> Option<Vec<u8>> {
    use egui::Key;
    if modifiers.ctrl {
        // Ctrl+A .. Ctrl+Z
        if let Some(c) = key.name().chars().next() {
            if c.is_ascii_alphabetic() {
                let b = (c.to_ascii_uppercase() as u8) - b'A' + 1;
                return Some(vec![b]);
            }
        }
    }
    match key {
        Key::Enter => Some(vec![b'\r']),
        Key::Backspace => Some(vec![0x7f]),
        Key::Tab => Some(vec![b'\t']),
        Key::Escape => Some(vec![0x1b]),
        Key::ArrowUp => Some(b"\x1b[A".to_vec()),
        Key::ArrowDown => Some(b"\x1b[B".to_vec()),
        Key::ArrowRight => Some(b"\x1b[C".to_vec()),
        Key::ArrowLeft => Some(b"\x1b[D".to_vec()),
        Key::Home => Some(b"\x1b[H".to_vec()),
        Key::End => Some(b"\x1b[F".to_vec()),
        Key::Delete => Some(b"\x1b[3~".to_vec()),
        Key::PageUp => Some(b"\x1b[5~".to_vec()),
        Key::PageDown => Some(b"\x1b[6~".to_vec()),
        _ => None,
    }
}
