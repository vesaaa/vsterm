use connection_mgr::ConnectionManager;
use connection_mgr::ConnectionState;
use egui::{Color32, FontId, Id, PointerButton, Pos2, Rect, Sense, Ui, Vec2};
use std::sync::Arc;
use term_core::{Rgb, TerminalSnapshot};

const CELL_W: f32 = 8.4;
const CELL_H: f32 = 16.0;
const TERM_BG: Color32 = Color32::BLACK;

#[derive(Clone, Copy, Default)]
struct TermSelection {
    /// Inclusive start cell (col, row) in snapshot space.
    anchor: Option<(usize, usize)>,
    /// Inclusive end cell while dragging / after release.
    focus: Option<(usize, usize)>,
}

impl TermSelection {
    fn clear(&mut self) {
        self.anchor = None;
        self.focus = None;
    }

    fn has_range(&self) -> bool {
        self.anchor.is_some() && self.focus.is_some() && self.anchor != self.focus
    }

    fn normalized(&self) -> Option<((usize, usize), (usize, usize))> {
        let a = self.anchor?;
        let b = self.focus?;
        let start = if (a.1, a.0) <= (b.1, b.0) { a } else { b };
        let end = if (a.1, a.0) <= (b.1, b.0) { b } else { a };
        Some((start, end))
    }

    fn contains(&self, col: usize, row: usize) -> bool {
        if !self.has_range() {
            return false;
        }
        let Some(((sc, sr), (ec, er))) = self.normalized() else {
            return false;
        };
        if row < sr || row > er {
            return false;
        }
        if sr == er {
            return col >= sc && col <= ec;
        }
        if row == sr {
            return col >= sc;
        }
        if row == er {
            return col <= ec;
        }
        true
    }
}

pub struct TerminalView;

impl TerminalView {
    /// Renders the active terminal. Returns desired (cols, rows) based on available space.
    pub fn show(ui: &mut Ui, mgr: &Arc<ConnectionManager>) -> (u16, u16) {
        let avail = ui.available_size();
        let cols = ((avail.x / CELL_W).floor() as u16).max(20);
        let rows = ((avail.y / CELL_H).floor() as u16).max(5);

        let (rect, resp) = ui.allocate_exact_size(avail, Sense::click_and_drag());
        ui.painter().rect_filled(rect, 0.0, TERM_BG);

        let sel_id = Id::new("vsterm_term_selection");
        let mut selection = ui
            .ctx()
            .data_mut(|d| d.get_temp::<TermSelection>(sel_id).unwrap_or_default());

        let snapshot = mgr
            .with_active(|c| {
                if c.state == ConnectionState::Connecting || c.state == ConnectionState::Failed {
                    return None;
                }
                Some(c.terminal.snapshot())
            })
            .flatten();

        let Some(mut snapshot) = snapshot else {
            selection.clear();
            ui.ctx().data_mut(|d| d.insert_temp(sel_id, selection));
            paint_status(ui, rect, mgr);
            return (cols, rows);
        };

        crate::term_highlight::apply_semantic(&mut snapshot);

        let font = FontId::monospace(13.0);
        let max_rows = snapshot.rows.min(rows as usize);
        let max_cols = snapshot.cols.min(cols as usize);
        let can_input = mgr
            .with_active(|c| c.state == ConnectionState::Connected)
            .unwrap_or(false);
        let menu_open = resp.context_menu_opened();

        // Primary-button selection only; leave range intact while the context menu is open.
        if !menu_open {
            let pointer_cell = resp
                .interact_pointer_pos()
                .or_else(|| ui.input(|i| i.pointer.interact_pos()))
                .and_then(|pos| pos_to_cell(pos, rect, max_cols, max_rows));

            if let Some(cell) = pointer_cell {
                if ui.input(|i| i.pointer.primary_pressed()) {
                    selection.anchor = Some(cell);
                    selection.focus = Some(cell);
                } else if ui.input(|i| i.pointer.primary_down()) && selection.anchor.is_some() {
                    selection.focus = Some(cell);
                }
            }
            if resp.clicked_by(PointerButton::Primary) && !resp.dragged() {
                if selection.anchor == selection.focus {
                    selection.clear();
                }
            }
        }

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
                if selection.contains(col, row) {
                    ui.painter().rect_filled(
                        cell_rect,
                        0.0,
                        Color32::from_rgba_unmultiplied(70, 130, 210, 140),
                    );
                } else if bg != TERM_BG {
                    ui.painter().rect_filled(cell_rect, 0.0, bg);
                }
                if cell.ch != ' ' {
                    ui.painter().text(
                        cell_rect.left_top() + Vec2::new(0.0, 1.0),
                        egui::Align2::LEFT_TOP,
                        cell.ch.to_string(),
                        font.clone(),
                        fg,
                    );
                }
            }
        }

        if snapshot.cursor_visible && !selection.has_range() {
            let (cx, cy) = snapshot.cursor;
            if cy < max_rows && cx < max_cols {
                let cursor_rect = Rect::from_min_size(
                    rect.min + Vec2::new(cx as f32 * CELL_W, cy as f32 * CELL_H),
                    Vec2::new(CELL_W, CELL_H),
                );
                ui.painter().rect_filled(
                    cursor_rect,
                    0.0,
                    Color32::from_rgba_unmultiplied(248, 248, 242, 160),
                );
            }
        }

        if resp.clicked_by(PointerButton::Primary) {
            resp.request_focus();
        }

        let selected_text = selection_text(&snapshot, &selection).filter(|t| !t.is_empty());
        let can_copy = selected_text.is_some();

        resp.context_menu(|ui| {
            ui.set_min_width(128.0);
            let copy_resp = crate::ui_icon::button(
                ui,
                crate::ui_icon::Icon::Copy,
                &crate::i18n::t("term.ctx.copy"),
                14.0,
                can_copy,
            );
            if copy_resp.clicked() {
                if let Some(text) = &selected_text {
                    ui.ctx().copy_text(text.clone());
                }
                ui.close_menu();
            }

            let paste_resp = crate::ui_icon::button(
                ui,
                crate::ui_icon::Icon::Paste,
                &crate::i18n::t("term.ctx.paste"),
                14.0,
                can_input,
            );
            if paste_resp.clicked() {
                if let Some(text) = read_clipboard_text() {
                    paste_to_session(mgr, &text);
                }
                resp.request_focus();
                ui.close_menu();
            }
        });

        if resp.has_focus() || resp.hovered() {
            let copy_requested = ui.input(|i| {
                let ctrl = i.modifiers.command || i.modifiers.ctrl;
                if !ctrl {
                    return false;
                }
                (i.key_pressed(egui::Key::C) && (selection.has_range() || i.modifiers.shift))
                    || i.key_pressed(egui::Key::Insert)
            });
            if copy_requested {
                if let Some(text) = &selected_text {
                    ui.ctx().copy_text(text.clone());
                }
            }

            if can_input {
                ui.input(|i| {
                    for event in &i.events {
                        match event {
                            egui::Event::Copy => {}
                            egui::Event::Paste(text) => {
                                paste_to_session(mgr, text);
                            }
                            egui::Event::Key {
                                key: egui::Key::C,
                                pressed: true,
                                modifiers,
                                ..
                            } if (modifiers.ctrl || modifiers.command)
                                && (selection.has_range() || modifiers.shift) => {}
                            egui::Event::Key {
                                key: egui::Key::Insert,
                                pressed: true,
                                modifiers,
                                ..
                            } if modifiers.ctrl || modifiers.command => {}
                            egui::Event::Key {
                                key: egui::Key::Escape,
                                pressed: true,
                                ..
                            } if selection.has_range() => {}
                            other => {
                                if let Some(bytes) = event_to_bytes(other) {
                                    let _ = mgr.write_to_active(&bytes);
                                }
                            }
                        }
                    }
                });
            }

            if ui.input(|i| i.key_pressed(egui::Key::Escape)) && selection.has_range() {
                selection.clear();
            }
        }

        ui.ctx().data_mut(|d| d.insert_temp(sel_id, selection));

        (cols, rows)
    }
}

fn paint_status(ui: &mut Ui, rect: Rect, mgr: &Arc<ConnectionManager>) {
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
}

fn paste_to_session(mgr: &Arc<ConnectionManager>, text: &str) {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    if !normalized.is_empty() {
        let _ = mgr.write_to_active(normalized.as_bytes());
    }
}

fn read_clipboard_text() -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    clipboard.get_text().ok().filter(|s| !s.is_empty())
}

fn pos_to_cell(pos: Pos2, rect: Rect, max_cols: usize, max_rows: usize) -> Option<(usize, usize)> {
    if max_cols == 0 || max_rows == 0 || !rect.contains(pos) {
        return None;
    }
    let col = ((pos.x - rect.min.x) / CELL_W).floor() as isize;
    let row = ((pos.y - rect.min.y) / CELL_H).floor() as isize;
    if col < 0 || row < 0 {
        return None;
    }
    let col = (col as usize).min(max_cols.saturating_sub(1));
    let row = (row as usize).min(max_rows.saturating_sub(1));
    Some((col, row))
}

fn selection_text(snapshot: &TerminalSnapshot, sel: &TermSelection) -> Option<String> {
    if !sel.has_range() {
        return None;
    }
    let ((sc, sr), (ec, er)) = sel.normalized()?;
    let mut out = String::new();
    for row in sr..=er {
        let (c0, c1) = if sr == er {
            (sc, ec)
        } else if row == sr {
            (sc, snapshot.cols.saturating_sub(1))
        } else if row == er {
            (0, ec)
        } else {
            (0, snapshot.cols.saturating_sub(1))
        };
        let mut line = String::new();
        for col in c0..=c1.min(snapshot.cols.saturating_sub(1)) {
            let idx = row * snapshot.cols + col;
            if let Some(cell) = snapshot.cells.get(idx) {
                line.push(cell.ch);
            }
        }
        out.push_str(line.trim_end());
        if row != er {
            out.push('\n');
        }
    }
    Some(out)
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
        egui::Event::Paste(text) => {
            let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
            Some(normalized.into_bytes())
        }
        _ => None,
    }
}

fn key_to_bytes(key: egui::Key, modifiers: &egui::Modifiers) -> Option<Vec<u8>> {
    use egui::Key;
    if modifiers.ctrl {
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
