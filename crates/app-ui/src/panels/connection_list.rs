use crate::i18n;
use connection_mgr::{ConnectionId, ConnectionManager, ConnectionState};
use egui::{Color32, FontId, Sense, Ui};
use std::sync::Arc;

const TAB_H: f32 = 40.0;

pub enum ConnAction {
    Select(ConnectionId),
    Close(ConnectionId),
}

pub fn show(ui: &mut Ui, mgr: &Arc<ConnectionManager>) -> (Option<ConnAction>, Option<egui::Rect>) {
    let mut action = None;
    let mut active_tab_rect = None;
    let active = mgr.active_id();
    let list = mgr.list_meta();

    // Full-bleed to the right edge so the active tab can fuse with column 3.
    let w = ui.available_width().max(1.0);
    let h = ui.available_height().max(1.0);

    egui::ScrollArea::vertical()
        .id_salt("connection_list_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_min_width(w);
            ui.set_max_width(w);

            if list.is_empty() {
                ui.add_space(8.0);
                ui.weak(i18n::t("conn.empty"));
                return;
            }

            for meta in &list {
                let is_active = Some(meta.id) == active;
                match host_tab(ui, w, meta, is_active) {
                    Some(ConnAction::Close(id)) => action = Some(ConnAction::Close(id)),
                    Some(ConnAction::Select(id)) => action = Some(ConnAction::Select(id)),
                    None => {}
                }
                if is_active {
                    active_tab_rect = ui.ctx().data(|d| {
                        d.get_temp::<egui::Rect>(ui.id().with(("tab_rect", meta.id.0)))
                    });
                }
            }

            // Keep column height so width/scale stays stable while resizing.
            let used = list.len() as f32 * TAB_H;
            if h > used {
                ui.allocate_exact_size(egui::vec2(w, h - used), Sense::hover());
            }
        });

    (action, active_tab_rect.filter(|r| r.is_positive()))
}

fn host_tab(
    ui: &mut Ui,
    w: f32,
    meta: &connection_mgr::ConnectionMeta,
    is_active: bool,
) -> Option<ConnAction> {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, TAB_H), Sense::click());
    let screen_rect = if let Some(to_global) = ui.ctx().layer_transform_to_global(ui.layer_id()) {
        to_global * rect
    } else {
        rect
    };
    ui.ctx().data_mut(|d| {
        d.insert_temp(ui.id().with(("tab_rect", meta.id.0)), screen_rect);
    });

    let side_bg = Color32::from_rgb(248, 249, 250);
    let central_bg = Color32::from_rgb(255, 255, 255);
    let accent = Color32::from_rgb(60, 120, 210);

    // Background — active tab: no right border; top/bottom only.
    if is_active {
        ui.painter().rect_filled(rect, 0.0, central_bg);
        let mut bar = rect;
        bar.max.x = rect.min.x + 3.0;
        ui.painter().rect_filled(bar, 0.0, accent);
        let stroke = egui::Stroke::new(1.0_f32, Color32::from_rgb(190, 205, 230));
        // Top/bottom span full tab width; separator overlap is masked in app.rs.
        ui.painter().hline(rect.x_range(), rect.min.y, stroke);
        ui.painter().hline(rect.x_range(), rect.max.y - 1.0, stroke);
        ui.painter().rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(rect.min.x + 3.0, rect.min.y + 1.0),
                egui::pos2(rect.max.x, rect.min.y + 2.0),
            ),
            0.0,
            Color32::from_rgba_unmultiplied(60, 120, 210, 40),
        );
    } else {
        ui.painter().rect_filled(rect, 0.0, side_bg);
        let stroke = egui::Stroke::new(1.0_f32, Color32::from_rgb(228, 230, 234));
        ui.painter().hline(rect.x_range(), rect.max.y - 0.5, stroke);
        if resp.hovered() {
            ui.painter().rect_filled(
                rect,
                0.0,
                Color32::from_rgba_unmultiplied(230, 235, 245, 160),
            );
        }
    }

    // Close hit area (right)
    let close_w = 22.0;
    let close_rect = egui::Rect::from_min_size(
        egui::pos2(rect.max.x - close_w - 4.0, rect.center().y - 11.0),
        egui::vec2(close_w, 22.0),
    );
    let close_resp = ui.interact(
        close_rect,
        ui.id().with("close").with(meta.id.0),
        Sense::click(),
    );

    // Content layout inside tab (single line, fixed height)
    let mut x = rect.min.x + if is_active { 10.0 } else { 8.0 };
    let y = rect.center().y;

    if let Some(tag) = &meta.color_tag {
        if let Some(color) = parse_hex_color(tag) {
            let tag_rect = egui::Rect::from_center_size(
                egui::pos2(x + 1.5, y),
                egui::vec2(3.0, 22.0),
            );
            ui.painter().rect_filled(tag_rect, 0.0, color);
            x += 8.0;
        }
    }

    let state_color = match meta.state {
        ConnectionState::Connected => Color32::from_rgb(40, 160, 90),
        ConnectionState::Connecting => Color32::from_rgb(200, 160, 40),
        ConnectionState::Disconnected => Color32::from_rgb(140, 145, 155),
        ConnectionState::Failed => Color32::from_rgb(200, 60, 60),
    };
    ui.painter().text(
        egui::pos2(x, y),
        egui::Align2::LEFT_CENTER,
        "●",
        FontId::proportional(9.0),
        state_color,
    );
    x += 12.0;

    let text_right = close_rect.min.x - 4.0;
    let text_w = (text_right - x).max(24.0);
    let max_chars = ((text_w / 7.5).floor() as usize).clamp(4, 48);
    let title = truncate(&meta.title, max_chars);
    ui.painter().text(
        egui::pos2(x, y),
        egui::Align2::LEFT_CENTER,
        title,
        FontId::proportional(13.0),
        if is_active {
            Color32::from_rgb(30, 50, 90)
        } else {
            Color32::from_rgb(52, 56, 62)
        },
    );

    // Close button
    if close_resp.hovered() {
        ui.painter().rect_filled(
            close_rect,
            3.0,
            Color32::from_rgb(230, 232, 236),
        );
    }
    ui.painter().text(
        close_rect.center(),
        egui::Align2::CENTER_CENTER,
        "×",
        FontId::proportional(16.0),
        Color32::from_rgb(80, 84, 92),
    );

    if close_resp.clicked() {
        return Some(ConnAction::Close(meta.id));
    }
    if resp.clicked() && !close_resp.clicked() {
        return Some(ConnAction::Select(meta.id));
    }
    if resp.hovered() || close_resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    None
}

fn truncate(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{t}…")
    }
}

fn parse_hex_color(s: &str) -> Option<Color32> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(Color32::from_rgb(r, g, b))
}
