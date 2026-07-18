use connection_mgr::ConnectionManager;
use connection_mgr::ConnectionState;
use egui::{Align2, Color32, FontId, Id, PointerButton, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use std::sync::Arc;
use term_core::{CellAttr, FoldControl, FoldGuide, Rgb, TerminalSnapshot};

const CELL_W: f32 = 8.4;
const CELL_H: f32 = 16.0;
const TERM_BG: Color32 = Color32::BLACK;
/// Solid amber block cursor, matching the product reference.
const CURSOR_COLOR: Color32 = Color32::from_rgb(255, 183, 0);
const SCROLL_W: f32 = 12.0;
/// Left chrome: `[HH:MM:SS]` + lineno + fold control (WindTerm-style).
const GUTTER_TIME_CHARS: f32 = 10.0;
const GUTTER_LINENO_CHARS: f32 = 5.0;
const GUTTER_FOLD_CHARS: f32 = 2.5;
const GUTTER_PAD: f32 = 6.0;
const GUTTER_W: f32 =
    CELL_W * (GUTTER_TIME_CHARS + GUTTER_LINENO_CHARS + GUTTER_FOLD_CHARS) + GUTTER_PAD;
/// Muted teal line numbers (WindTerm reference).
const GUTTER_LINENO: Color32 = Color32::from_rgb(78, 158, 168);
/// Active / cursor line number.
const GUTTER_LINENO_ACTIVE: Color32 = Color32::from_rgb(236, 110, 180);
const GUTTER_TIME: Color32 = Color32::from_rgb(110, 140, 148);
/// Fold box border, stem, hook, and expanded − (WindTerm medium-dark grey — not white).
const GUTTER_FOLD_CHROME: Color32 = Color32::from_rgb(100, 108, 118);
/// Collapsed (+) chip fill (same family as chrome).
const GUTTER_FOLD_FILL: Color32 = Color32::from_rgb(100, 108, 118);
const GUTTER_ACTIVE_BG: Color32 = Color32::from_rgb(28, 26, 34);
const ELLIPSIS_FG: Color32 = Color32::from_rgb(180, 186, 198);
const FOLD_BOX_SIZE: f32 = 11.0;

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
    /// Estimate PTY cols/rows for a terminal pane of `size` (includes gutter + scrollbar).
    pub fn size_for_rect(size: egui::Vec2) -> (u16, u16) {
        let content_w = (size.x - SCROLL_W - GUTTER_W).max(CELL_W * 20.0);
        let cols = ((content_w / CELL_W).floor() as u16).max(20);
        let rows = ((size.y / CELL_H).floor() as u16).max(5);
        (cols, rows)
    }

    /// Renders the active terminal. Returns desired (cols, rows) based on available space.
    ///
    /// When `grab_focus` is true, keyboard focus moves to the terminal grid so the
    /// user can type immediately after connect.
    pub fn show(ui: &mut Ui, mgr: &Arc<ConnectionManager>, grab_focus: bool) -> (u16, u16) {
        // Stay strictly inside the parent-allocated rect (no bleed into bottom strip).
        let outer_max = ui.max_rect();
        ui.set_clip_rect(outer_max);
        let avail = ui.available_size().min(outer_max.size());
        let content_w = (avail.x - SCROLL_W - GUTTER_W).max(CELL_W * 20.0);
        let (cols, rows) = Self::size_for_rect(avail);

        let (outer, _) = ui.allocate_exact_size(avail, Sense::hover());
        let outer = outer.intersect(outer_max);
        let gutter_rect = Rect::from_min_size(
            outer.min,
            Vec2::new(GUTTER_W.min(outer.width()), outer.height()),
        );
        let grid_rect = Rect::from_min_size(
            Pos2::new(gutter_rect.max.x, outer.min.y),
            Vec2::new(content_w.min(outer.width() - GUTTER_W).max(0.0), outer.height()),
        );
        let scroll_rect = Rect::from_min_max(
            Pos2::new(grid_rect.max.x, outer.min.y),
            outer.max,
        );

        let gutter_resp = ui.interact(gutter_rect, ui.id().with("term_gutter"), Sense::click());
        let resp = ui.interact(grid_rect, ui.id().with("term_grid"), Sense::click_and_drag());
        ui.painter().rect_filled(gutter_rect, 0.0, Color32::from_rgb(12, 12, 14));
        ui.painter().rect_filled(grid_rect, 0.0, TERM_BG);

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
            paint_status(ui, grid_rect, mgr);
            return (cols, rows);
        };

        crate::term_highlight::apply_semantic(&mut snapshot);

        let font = FontId::monospace(13.0);
        let gutter_font = FontId::monospace(11.0);
        let max_rows = snapshot.rows.min(rows as usize);
        let max_cols = snapshot.cols.min(cols as usize);
        let can_input = mgr
            .with_active(|c| c.state == ConnectionState::Connected)
            .unwrap_or(false);
        let menu_open = resp.context_menu_opened();

        // Default cursor on gutter chrome; I-beam only over the cell grid.
        if gutter_resp.hovered() && !menu_open {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
        } else if resp.hovered() && !menu_open {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }

        // Mouse wheel on grid or gutter → scrollback (positive = into history).
        if resp.hovered() || gutter_resp.hovered() {
            let scroll_y = ui.input(|i| {
                i.events.iter().fold(0.0_f32, |acc, e| match e {
                    egui::Event::MouseWheel {
                        unit,
                        delta,
                        modifiers,
                        ..
                    } if !modifiers.ctrl => {
                        let lines = match unit {
                            egui::MouseWheelUnit::Line => delta.y,
                            egui::MouseWheelUnit::Page => delta.y * max_rows as f32,
                            egui::MouseWheelUnit::Point => delta.y / CELL_H,
                        };
                        acc + lines
                    }
                    _ => acc,
                })
            });
            let delta = scroll_y.round() as i32;
            if delta != 0 {
                let _ = mgr.with_active(|c| c.terminal.scroll_lines(delta));
                selection.clear();
                ui.ctx().request_repaint();
                if let Some(s) = mgr.with_active(|c| c.terminal.snapshot()) {
                    snapshot = s;
                    crate::term_highlight::apply_semantic(&mut snapshot);
                }
            }
        }

        // Fold control clicks (gutter only) — before selection handling.
        if !menu_open && gutter_resp.clicked_by(PointerButton::Primary) {
            if let Some(pos) = gutter_resp.interact_pointer_pos() {
                if let Some(row) = pos_to_row(pos, gutter_rect, max_rows) {
                    if let Some(g) = snapshot.gutters.get(row) {
                        if g.fold.is_some() {
                            if let Some(id) = g.block_id {
                                let _ = mgr.with_active(|c| c.terminal.toggle_fold(id));
                                selection.clear();
                                ui.ctx().request_repaint();
                                if let Some(s) = mgr.with_active(|c| c.terminal.snapshot()) {
                                    snapshot = s;
                                    crate::term_highlight::apply_semantic(&mut snapshot);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Primary-button selection only on the cell grid.
        if !menu_open {
            let pointer_cell = resp
                .interact_pointer_pos()
                .or_else(|| ui.input(|i| i.pointer.interact_pos()))
                .and_then(|pos| pos_to_cell(pos, grid_rect, max_cols, max_rows));

            if let Some(cell) = pointer_cell {
                if ui.input(|i| i.pointer.primary_pressed()) {
                    selection.anchor = Some(cell);
                    selection.focus = Some(cell);
                    ui.ctx().request_repaint();
                } else if ui.input(|i| i.pointer.primary_down()) && selection.anchor.is_some() {
                    selection.focus = Some(cell);
                    ui.ctx().request_repaint();
                }
            }
            if resp.clicked_by(PointerButton::Primary) && !resp.dragged() {
                if selection.anchor == selection.focus {
                    selection.clear();
                }
            }
        }

        for row in 0..max_rows {
            let is_cursor_row = snapshot.cursor_visible && snapshot.cursor.1 == row;
            if is_cursor_row {
                let row_bg = Rect::from_min_size(
                    Pos2::new(gutter_rect.min.x, gutter_rect.min.y + row as f32 * CELL_H),
                    Vec2::new(gutter_rect.width() + grid_rect.width(), CELL_H),
                );
                ui.painter().rect_filled(row_bg, 0.0, GUTTER_ACTIVE_BG);
            }

            paint_gutter_row(
                ui,
                gutter_rect,
                row,
                snapshot.gutters.get(row),
                &gutter_font,
                is_cursor_row,
            );

            let mut col = 0;
            while col < max_cols {
                col = paint_terminal_run(
                    ui,
                    &snapshot,
                    grid_rect,
                    row,
                    col,
                    max_cols,
                    &font,
                    &selection,
                );
            }

            // Collapsed block: boxed "···" after the command text.
            if snapshot
                .gutters
                .get(row)
                .is_some_and(|g| g.collapsed_mark)
            {
                paint_collapsed_ellipsis(ui, &snapshot, grid_rect, row, max_cols, &font);
            }
        }

        if snapshot.cursor_visible && !selection.has_range() {
            let (cx, cy) = snapshot.cursor;
            if cy < max_rows && cx < max_cols {
                let cursor_rect = Rect::from_min_size(
                    grid_rect.min + Vec2::new(cx as f32 * CELL_W, cy as f32 * CELL_H),
                    Vec2::new(CELL_W, CELL_H),
                );
                ui.painter().rect_filled(
                    cursor_rect,
                    0.0,
                    CURSOR_COLOR,
                );
            }
        }

        // Scrollbar over the virtual (fold-aware) line list.
        let scroll_max = snapshot.virtual_len.saturating_sub(max_rows);
        if scroll_max > 0 {
            if let Some(new_offset) =
                paint_scrollbar(ui, scroll_rect, snapshot.display_offset, scroll_max, max_rows)
            {
                let _ = mgr.with_active(|c| c.terminal.set_display_offset(new_offset));
                selection.clear();
                ui.ctx().request_repaint();
            }
        } else {
            ui.painter()
                .rect_filled(scroll_rect, 0.0, Color32::from_rgb(20, 20, 22));
        }

        if grab_focus || resp.clicked_by(PointerButton::Primary) {
            resp.request_focus();
        }

        // Hovering the grid claims keyboard focus so Space/Enter go to the PTY,
        // not leftover-focused chrome buttons (quick commands, toolbar, …).
        if !menu_open && !grab_focus && resp.hovered() && !resp.has_focus() {
            resp.request_focus();
        }

        let selected_text = selection_text(&snapshot, &selection).filter(|t| !t.is_empty());
        let can_copy = selected_text.is_some();

        resp.context_menu(|ui| {
            crate::ctx_menu::prepare(ui);
            let copy_sc = crate::ctx_menu::shortcut_ctrl("C");
            let paste_sc = crate::ctx_menu::shortcut_ctrl("V");
            if crate::ctx_menu::item(
                ui,
                Some(crate::ui_icon::Icon::Copy),
                &crate::i18n::t("term.ctx.copy"),
                Some(&copy_sc),
                can_copy,
            )
            .clicked()
            {
                if let Some(text) = &selected_text {
                    ui.ctx().copy_text(text.clone());
                }
                ui.close_menu();
            }

            if crate::ctx_menu::item(
                ui,
                Some(crate::ui_icon::Icon::Paste),
                &crate::i18n::t("term.ctx.paste"),
                Some(&paste_sc),
                can_input,
            )
            .clicked()
            {
                if let Some(text) = read_clipboard_text() {
                    paste_to_session(mgr, &text);
                }
                resp.request_focus();
                ui.close_menu();
            }
        });

        // Only drive PTY input when this widget owns keyboard focus.
        if !menu_open && resp.has_focus() {
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

            // Shift+PageUp/Down → scrollback (Alacritty-style). Bare PageUp/Down go to the PTY.
            if ui.input(|i| i.key_pressed(egui::Key::PageUp) && i.modifiers.shift) {
                let _ = mgr.with_active(|c| c.terminal.scroll_page_up());
                selection.clear();
            }
            if ui.input(|i| i.key_pressed(egui::Key::PageDown) && i.modifiers.shift) {
                let _ = mgr.with_active(|c| c.terminal.scroll_page_down());
                selection.clear();
            }

            if can_input {
                ui.input(|i| {
                    for event in &i.events {
                        match event {
                            // egui-winit converts Ctrl+C into Event::Copy before
                            // producing a Key event. Preserve terminal SIGINT
                            // when there is no selection; Ctrl+Shift+C remains
                            // the explicit clipboard shortcut.
                            egui::Event::Copy => {
                                if let Some(text) = &selected_text {
                                    ui.ctx().copy_text(text.clone());
                                } else if !i.modifiers.shift {
                                    let _ = mgr.write_to_active(&[0x03]);
                                }
                            }
                            // Ctrl+X is also consumed by egui as a clipboard
                            // command. Terminals need the CAN control byte.
                            egui::Event::Cut => {
                                let _ = mgr.write_to_active(&[0x18]);
                            }
                            egui::Event::Paste(text) => {
                                let _ = mgr.with_active(|c| c.terminal.scroll_to_bottom());
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
                                key: egui::Key::PageUp | egui::Key::PageDown,
                                pressed: true,
                                modifiers,
                                ..
                            } if modifiers.shift => {}
                            egui::Event::Key {
                                key: egui::Key::Escape,
                                pressed: true,
                                ..
                            } if selection.has_range() => {}
                            other => {
                                if let Some(bytes) = event_to_bytes(other) {
                                    let _ = mgr.with_active(|c| c.terminal.scroll_to_bottom());
                                    // Stamp command blocks before the PTY advances past the line.
                                    if bytes.iter().any(|&b| b == b'\r' || b == b'\n') {
                                        let _ = mgr.with_active(|c| c.terminal.on_client_enter());
                                    }
                                    if std::env::var_os("VSTERM_DIAG").is_some() {
                                        let t0 = std::time::Instant::now();
                                        let _ = mgr.write_to_active(&bytes);
                                        let ms = t0.elapsed().as_secs_f64() * 1000.0;
                                        // Only log slow writes — per-keystroke console I/O
                                        // would itself inflate the latency under test.
                                        if ms >= 1.0 {
                                            tracing::warn!(
                                                "VSTERM_DIAG: terminal write_to_active {ms:.2} ms ({} bytes)",
                                                bytes.len()
                                            );
                                        }
                                    } else {
                                        let _ = mgr.write_to_active(&bytes);
                                    }
                                }
                            }
                        }
                    }
                });
            }

            if ui.input(|i| i.key_pressed(egui::Key::Escape)) && selection.has_range() {
                selection.clear();
            }
        } else if resp.hovered() {
            // Still allow copy shortcuts while reading scrollback without focus steal races.
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
        }

        ui.ctx().data_mut(|d| d.insert_temp(sel_id, selection));

        (cols, rows)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct CellPaintKey {
    fg: Color32,
    bg: Color32,
    selected: bool,
}

fn cell_paint_key(cell: &CellAttr, col: usize, row: usize, selection: &TermSelection) -> CellPaintKey {
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
    CellPaintKey {
        fg,
        bg,
        selected: selection.contains(col, row),
    }
}

/// Paint one horizontal run of cells sharing the same colours/selection.
/// Returns the next column index to paint.
fn paint_terminal_run(
    ui: &mut Ui,
    snapshot: &TerminalSnapshot,
    grid_rect: Rect,
    row: usize,
    col: usize,
    max_cols: usize,
    font: &FontId,
    selection: &TermSelection,
) -> usize {
    let idx = row * snapshot.cols + col;
    let Some(cell) = snapshot.cells.get(idx) else {
        return col + 1;
    };
    let key = cell_paint_key(cell, col, row, selection);
    let mut run_end = col + 1;
    while run_end < max_cols {
        let next = snapshot.cells.get(row * snapshot.cols + run_end);
        let Some(next) = next else {
            break;
        };
        if cell_paint_key(next, run_end, row, selection) != key {
            break;
        }
        run_end += 1;
    }

    let row_y = row as f32 * CELL_H;
    // Background can be one rect for the whole run; glyphs must be pinned to
    // cell columns. A single LayoutJob uses the font's natural advances, which
    // diverge from CELL_W and make the block cursor drift away from the last
    // character as the line grows — that is not how a grid terminal works.
    let run_rect = Rect::from_min_size(
        grid_rect.min + Vec2::new(col as f32 * CELL_W, row_y),
        Vec2::new((run_end - col) as f32 * CELL_W, CELL_H),
    );
    if key.selected {
        ui.painter().rect_filled(
            run_rect,
            0.0,
            Color32::from_rgba_unmultiplied(70, 130, 210, 140),
        );
    } else if key.bg != TERM_BG {
        ui.painter().rect_filled(run_rect, 0.0, key.bg);
    }

    let painter = ui.painter();
    for c in col..run_end {
        let ch = snapshot.cells[row * snapshot.cols + c].ch;
        if ch == ' ' {
            continue;
        }
        let pos = grid_rect.min + Vec2::new(c as f32 * CELL_W, row_y + 1.0);
        painter.text(pos, Align2::LEFT_TOP, ch, font.clone(), key.fg);
    }
    run_end
}

/// Returns a new display_offset when the user drags the thumb.
/// `scroll_max` is the maximum virtual offset (virtual_len − screen_rows).
fn paint_scrollbar(
    ui: &mut Ui,
    rect: Rect,
    display_offset: usize,
    scroll_max: usize,
    screen_rows: usize,
) -> Option<usize> {
    if scroll_max == 0 || rect.width() < 2.0 {
        return None;
    }

    ui.painter()
        .rect_filled(rect, 0.0, Color32::from_rgb(28, 28, 32));

    let track = rect.shrink2(Vec2::new(2.0, 3.0));
    let total = (scroll_max + screen_rows) as f32;
    let thumb_h = ((screen_rows as f32 / total) * track.height()).clamp(18.0, track.height());
    let travel = (track.height() - thumb_h).max(0.0);
    // offset=scroll_max → thumb at top; offset=0 → thumb at bottom.
    let t = 1.0 - (display_offset as f32 / scroll_max as f32);
    let thumb_y = track.min.y + travel * t;
    let thumb = Rect::from_min_size(
        Pos2::new(track.min.x, thumb_y),
        Vec2::new(track.width(), thumb_h),
    );

    let resp = ui.interact(rect, ui.id().with("term_scroll"), Sense::click_and_drag());
    let fill = if resp.hovered() || resp.dragged() {
        Color32::from_rgb(110, 115, 125)
    } else {
        Color32::from_rgb(70, 74, 82)
    };
    ui.painter().rect_filled(thumb, 2.0, fill);

    if resp.dragged() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let rel = ((pos.y - track.min.y - thumb_h * 0.5) / travel.max(1.0)).clamp(0.0, 1.0);
            let offset = ((1.0 - rel) * scroll_max as f32).round() as usize;
            return Some(offset.min(scroll_max));
        }
    }
    if resp.clicked() && !resp.dragged() {
        if let Some(pos) = resp.interact_pointer_pos() {
            if pos.y < thumb.min.y {
                return Some(display_offset.saturating_add(screen_rows).min(scroll_max));
            }
            if pos.y > thumb.max.y {
                return Some(display_offset.saturating_sub(screen_rows));
            }
        }
    }
    None
}

fn paint_gutter_row(
    ui: &mut Ui,
    gutter_rect: Rect,
    row: usize,
    info: Option<&term_core::GutterInfo>,
    font: &FontId,
    is_cursor_row: bool,
) {
    let Some(info) = info else {
        return;
    };
    let row_top = gutter_rect.min.y + row as f32 * CELL_H;
    let y_text = row_top + 1.0;
    let mut x = gutter_rect.min.x + 2.0;
    let painter = ui.painter_at(gutter_rect);

    if let Some((h, m, s)) = info.time_hm {
        let label = format!("[{h:02}:{m:02}:{s:02}]");
        painter.text(
            Pos2::new(x, y_text),
            egui::Align2::LEFT_TOP,
            label,
            font.clone(),
            if is_cursor_row {
                GUTTER_LINENO_ACTIVE
            } else {
                GUTTER_TIME
            },
        );
    }
    x += CELL_W * GUTTER_TIME_CHARS;

    if let Some(n) = info.lineno {
        let label = format!("{n:>4}");
        let color = if is_cursor_row {
            GUTTER_LINENO_ACTIVE
        } else {
            GUTTER_LINENO
        };
        painter.text(
            Pos2::new(x, y_text),
            egui::Align2::LEFT_TOP,
            label,
            font.clone(),
            color,
        );
    }
    x += CELL_W * GUTTER_LINENO_CHARS;

    let fold_col_x = x;
    let box_size = FOLD_BOX_SIZE;
    let box_x = fold_col_x + 2.0;
    let box_y = row_top + (CELL_H - box_size) * 0.5;
    let box_rect = Rect::from_min_size(Pos2::new(box_x, box_y), Vec2::splat(box_size));
    let stem_x = box_rect.center().x;
    let stroke = Stroke::new(1.0, GUTTER_FOLD_CHROME);

    // Boxed −/+ on the command header.
    // Font glyphs for +/- carry large ink margins, so they look tiny in an 11px
    // box; WindTerm-style vector strokes fill the frame tightly instead.
    // Collapsed (+): filled grey chip with dark “cut-out” plus.
    // Expanded (−): hollow border; − and stem share GUTTER_FOLD_CHROME.
    if let Some(fold) = info.fold {
        let collapsed = matches!(fold, FoldControl::Expand);
        if collapsed {
            painter.rect_filled(box_rect, 1.5, GUTTER_FOLD_FILL);
        }
        painter.rect_stroke(box_rect, 1.5, stroke, egui::StrokeKind::Inside);
        paint_fold_glyph(
            &painter,
            box_rect,
            collapsed,
            if collapsed {
                TERM_BG // negative-space + like WindTerm
            } else {
                GUTTER_FOLD_CHROME
            },
        );
    }

    // Tree stem under expanded blocks.
    match info.fold_guide {
        Some(FoldGuide::Header) => {
            // Stem from bottom of the fold box to the bottom of this row.
            painter.line_segment(
                [
                    Pos2::new(stem_x, box_rect.bottom()),
                    Pos2::new(stem_x, row_top + CELL_H),
                ],
                stroke,
            );
        }
        Some(FoldGuide::Middle) => {
            painter.line_segment(
                [
                    Pos2::new(stem_x, row_top),
                    Pos2::new(stem_x, row_top + CELL_H),
                ],
                stroke,
            );
        }
        Some(FoldGuide::End) => {
            let mid_y = row_top + CELL_H * 0.5;
            painter.line_segment(
                [Pos2::new(stem_x, row_top), Pos2::new(stem_x, mid_y)],
                stroke,
            );
            // Right-angle hook toward the text.
            painter.line_segment(
                [
                    Pos2::new(stem_x, mid_y),
                    Pos2::new(stem_x + box_size * 0.65, mid_y),
                ],
                stroke,
            );
        }
        None => {}
    }
}

fn paint_fold_glyph(painter: &egui::Painter, box_rect: Rect, expand: bool, color: Color32) {
    // ~2px inset → glyph almost fills the box (WindTerm-like), not floating tiny.
    let r = box_rect.shrink(2.0);
    let cx = r.center().x;
    let cy = r.center().y;
    let stroke = Stroke::new(1.35, color);
    painter.line_segment(
        [Pos2::new(r.left(), cy), Pos2::new(r.right(), cy)],
        stroke,
    );
    if expand {
        painter.line_segment(
            [Pos2::new(cx, r.top()), Pos2::new(cx, r.bottom())],
            stroke,
        );
    }
}

fn paint_collapsed_ellipsis(
    ui: &mut Ui,
    snapshot: &TerminalSnapshot,
    grid_rect: Rect,
    row: usize,
    max_cols: usize,
    font: &FontId,
) {
    // Place the box just after the last non-space cell on this row.
    let mut end_col = 0usize;
    for col in 0..max_cols {
        let idx = row * snapshot.cols + col;
        if snapshot.cells.get(idx).is_some_and(|c| c.ch != ' ') {
            end_col = col + 1;
        }
    }
    if end_col >= max_cols {
        end_col = max_cols.saturating_sub(3);
    }
    let box_w = CELL_W * 3.2;
    let box_h = CELL_H - 3.0;
    let origin = grid_rect.min + Vec2::new(end_col as f32 * CELL_W + 4.0, row as f32 * CELL_H + 1.5);
    if origin.x + box_w > grid_rect.max.x {
        return;
    }
    let r = Rect::from_min_size(origin, Vec2::new(box_w, box_h));
    ui.painter().rect_stroke(
        r,
        2.0,
        Stroke::new(1.0, Color32::from_rgb(90, 96, 110)),
        egui::StrokeKind::Outside,
    );
    ui.painter().text(
        r.center(),
        egui::Align2::CENTER_CENTER,
        "···",
        font.clone(),
        ELLIPSIS_FG,
    );
}

fn pos_to_row(pos: Pos2, rect: Rect, max_rows: usize) -> Option<usize> {
    if max_rows == 0 || !rect.contains(pos) {
        return None;
    }
    let row = ((pos.y - rect.min.y) / CELL_H).floor() as isize;
    if row < 0 {
        return None;
    }
    Some((row as usize).min(max_rows.saturating_sub(1)))
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

    // C0 control keys. Do not derive this from `key.name()`: names such as
    // "ArrowUp" used to turn Ctrl+Up into Ctrl+A. Alt+Ctrl is commonly AltGr,
    // so printable AltGr input must be left to Event::Text.
    if modifiers.ctrl && !modifiers.alt {
        let control = match key {
            Key::A => Some(0x01),
            Key::B => Some(0x02),
            Key::C => Some(0x03), // SIGINT
            Key::D => Some(0x04), // EOF
            Key::E => Some(0x05),
            Key::F => Some(0x06),
            Key::G => Some(0x07),
            Key::H => Some(0x08),
            Key::I => Some(0x09),
            Key::J => Some(0x0a),
            Key::K => Some(0x0b),
            Key::L => Some(0x0c),
            Key::M => Some(0x0d),
            Key::N => Some(0x0e),
            Key::O => Some(0x0f),
            Key::P => Some(0x10),
            Key::Q => Some(0x11),
            Key::R => Some(0x12),
            Key::S => Some(0x13),
            Key::T => Some(0x14),
            Key::U => Some(0x15),
            Key::V => Some(0x16),
            Key::W => Some(0x17),
            Key::X => Some(0x18),
            Key::Y => Some(0x19),
            Key::Z => Some(0x1a), // SIGTSTP
            Key::Space | Key::Num2 => Some(0x00),
            Key::OpenBracket | Key::Num3 => Some(0x1b),
            Key::Backslash | Key::Num4 => Some(0x1c),
            Key::CloseBracket | Key::Num5 => Some(0x1d),
            Key::Num6 => Some(0x1e),
            Key::Minus | Key::Num7 => Some(0x1f),
            Key::Questionmark | Key::Num8 => Some(0x7f),
            _ => None,
        };
        if let Some(control) = control {
            return Some(vec![control]);
        }
    }

    let modifier = xterm_modifier(modifiers);
    let csi_final = |final_byte: u8| {
        if modifier == 1 {
            vec![0x1b, b'[', final_byte]
        } else {
            format!("\x1b[1;{modifier}{}", final_byte as char).into_bytes()
        }
    };

    // Meta/Alt+letter and Alt+digit conventionally sends ESC then the key.
    if modifiers.alt && !modifiers.ctrl {
        if let Some(ch) = printable_key_char(key, modifiers.shift) {
            return Some(vec![0x1b, ch]);
        }
    }

    match key {
        Key::Enter => Some(vec![b'\r']),
        Key::Backspace => Some(vec![0x7f]),
        Key::Tab if modifiers.shift => Some(b"\x1b[Z".to_vec()),
        Key::Tab => Some(vec![b'\t']),
        Key::Escape => Some(vec![0x1b]),
        Key::ArrowUp => Some(csi_final(b'A')),
        Key::ArrowDown => Some(csi_final(b'B')),
        Key::ArrowRight => Some(csi_final(b'C')),
        Key::ArrowLeft => Some(csi_final(b'D')),
        Key::Home => Some(csi_final(b'H')),
        Key::End => Some(csi_final(b'F')),
        Key::Insert => Some(csi_tilde(2, modifier)),
        Key::Delete => Some(csi_tilde(3, modifier)),
        Key::PageUp => Some(csi_tilde(5, modifier)),
        Key::PageDown => Some(csi_tilde(6, modifier)),
        Key::F1 => Some(b"\x1bOP".to_vec()),
        Key::F2 => Some(b"\x1bOQ".to_vec()),
        Key::F3 => Some(b"\x1bOR".to_vec()),
        Key::F4 => Some(b"\x1bOS".to_vec()),
        Key::F5 => Some(csi_tilde(15, modifier)),
        Key::F6 => Some(csi_tilde(17, modifier)),
        Key::F7 => Some(csi_tilde(18, modifier)),
        Key::F8 => Some(csi_tilde(19, modifier)),
        Key::F9 => Some(csi_tilde(20, modifier)),
        Key::F10 => Some(csi_tilde(21, modifier)),
        Key::F11 => Some(csi_tilde(23, modifier)),
        Key::F12 => Some(csi_tilde(24, modifier)),
        _ => None,
    }
}

fn xterm_modifier(modifiers: &egui::Modifiers) -> u8 {
    1 + u8::from(modifiers.shift)
        + 2 * u8::from(modifiers.alt)
        + 4 * u8::from(modifiers.ctrl)
}

fn csi_tilde(number: u8, modifier: u8) -> Vec<u8> {
    if modifier == 1 {
        format!("\x1b[{number}~").into_bytes()
    } else {
        format!("\x1b[{number};{modifier}~").into_bytes()
    }
}

fn printable_key_char(key: egui::Key, shift: bool) -> Option<u8> {
    use egui::Key;
    let ch = match key {
        Key::A => b'a',
        Key::B => b'b',
        Key::C => b'c',
        Key::D => b'd',
        Key::E => b'e',
        Key::F => b'f',
        Key::G => b'g',
        Key::H => b'h',
        Key::I => b'i',
        Key::J => b'j',
        Key::K => b'k',
        Key::L => b'l',
        Key::M => b'm',
        Key::N => b'n',
        Key::O => b'o',
        Key::P => b'p',
        Key::Q => b'q',
        Key::R => b'r',
        Key::S => b's',
        Key::T => b't',
        Key::U => b'u',
        Key::V => b'v',
        Key::W => b'w',
        Key::X => b'x',
        Key::Y => b'y',
        Key::Z => b'z',
        Key::Num0 => b'0',
        Key::Num1 => b'1',
        Key::Num2 => b'2',
        Key::Num3 => b'3',
        Key::Num4 => b'4',
        Key::Num5 => b'5',
        Key::Num6 => b'6',
        Key::Num7 => b'7',
        Key::Num8 => b'8',
        Key::Num9 => b'9',
        _ => return None,
    };
    Some(if shift && ch.is_ascii_lowercase() {
        ch.to_ascii_uppercase()
    } else {
        ch
    })
}

#[cfg(test)]
mod key_tests {
    use super::*;

    #[test]
    fn ctrl_letters_emit_c0_codes() {
        assert_eq!(
            key_to_bytes(egui::Key::Z, &egui::Modifiers::CTRL),
            Some(vec![0x1a])
        );
        assert_eq!(
            key_to_bytes(egui::Key::C, &egui::Modifiers::CTRL),
            Some(vec![0x03])
        );
    }

    #[test]
    fn ctrl_navigation_is_not_misread_as_letter() {
        assert_eq!(
            key_to_bytes(egui::Key::ArrowUp, &egui::Modifiers::CTRL),
            Some(b"\x1b[1;5A".to_vec())
        );
    }

    #[test]
    fn common_terminal_keys_emit_xterm_sequences() {
        assert_eq!(
            key_to_bytes(egui::Key::Tab, &egui::Modifiers::SHIFT),
            Some(b"\x1b[Z".to_vec())
        );
        assert_eq!(
            key_to_bytes(egui::Key::F12, &egui::Modifiers::NONE),
            Some(b"\x1b[24~".to_vec())
        );
    }
}
