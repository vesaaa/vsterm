//! Unified menu chrome (WindTerm-style rows).
//!
//! Every popup menu should call [`prepare`] once, then use [`item`] / [`submenu`] /
//! [`separator`] so width, padding, icon, label, and trailing (shortcut / chevron /
//! check) stay aligned across rows.

use crate::ui_icon::{self, Icon};
use egui::{Align2, Color32, FontId, Sense, Ui};

/// Fixed menu width (popup `available_width` is huge — never size rows from it).
pub const MENU_WIDTH: f32 = 216.0;
const ROW_H: f32 = 26.0;
const PAD_X: f32 = 8.0;
const ICON_SIZE: f32 = 13.0;
/// Always reserved so labels share one left edge even when a row has no icon.
const ICON_SLOT: f32 = 18.0;
/// Always reserved so labels share one right edge (shortcuts / chevron / check).
const TRAIL_SLOT: f32 = 52.0;
/// Opaque row hover (egui menu style sets `weak_bg_fill` transparent — don't use it).
const ROW_HOVER: Color32 = Color32::from_rgb(220, 224, 230);

enum Trailing<'a> {
    Empty,
    Text(&'a str),
    Submenu,
    Check,
}

/// Call at the start of every menu / submenu body.
pub fn prepare(ui: &mut Ui) {
    ui.set_min_width(MENU_WIDTH);
    ui.set_max_width(MENU_WIDTH);
    ui.spacing_mut().item_spacing.y = 1.0;
    ui.spacing_mut().button_padding = egui::vec2(0.0, 0.0);

    // `egui::menu` sets inactive.weak_bg_fill = TRANSPARENT so stock submenu
    // buttons look "glassy" over whatever is behind the popup. Force an opaque
    // chrome matching File menu / Frame::menu.
    let bg = ui.visuals().window_fill();
    ui.visuals_mut().widgets.inactive.weak_bg_fill = bg;
    ui.visuals_mut().widgets.hovered.weak_bg_fill = ROW_HOVER;
    ui.visuals_mut().widgets.open.weak_bg_fill = ROW_HOVER;
    ui.painter()
        .rect_filled(ui.available_rect_before_wrap(), 0.0, bg);
}

/// Modifier label for accelerators (`Ctrl+` / `⌘`).
pub fn mod_key() -> &'static str {
    if cfg!(target_os = "macos") {
        "⌘"
    } else {
        "Ctrl+"
    }
}

pub fn shortcut_ctrl(letter: &str) -> String {
    format!("{}{letter}", mod_key())
}

/// One menu row: `[icon] label ………… shortcut`.
/// Missing icon / shortcut still reserves the slot so labels align.
pub fn item(
    ui: &mut Ui,
    icon: Option<Icon>,
    label: &str,
    shortcut: Option<&str>,
    enabled: bool,
) -> egui::Response {
    let trail = match shortcut.filter(|s| !s.is_empty()) {
        Some(s) => Trailing::Text(s),
        None => Trailing::Empty,
    };
    paint_row(ui, icon, label, trail, enabled)
}

/// Selectable row with a trailing check when `checked`.
pub fn check_item(
    ui: &mut Ui,
    icon: Option<Icon>,
    label: &str,
    checked: bool,
    enabled: bool,
) -> egui::Response {
    let trail = if checked {
        Trailing::Check
    } else {
        Trailing::Empty
    };
    paint_row(ui, icon, label, trail, enabled)
}

/// Nested submenu trigger. Uses egui's hover submenu, but redraws the row so the
/// default `⏵` (often missing as `?` without emoji fonts) becomes a Lucide chevron.
///
/// Layout advances by the same [`ROW_H`] as [`item`], so spacing matches File menu.
pub fn submenu<R>(
    ui: &mut Ui,
    icon: Option<Icon>,
    label: &str,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> egui::InnerResponse<Option<R>> {
    // Reserve the same slot as `item`, but leave interaction to egui's menu_button.
    let (_, rect) = ui.allocate_space(egui::vec2(MENU_WIDTH, ROW_H));

    let mut inner = None;
    let mut button_response = None;
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.set_min_size(rect.size());
        ui.spacing_mut().interact_size = rect.size();
        let egui::InnerResponse {
            inner: sub_inner,
            response,
        } = ui.menu_button(egui::RichText::new("\u{00a0}").size(1.0), |ui| {
            prepare(ui);
            add_contents(ui)
        });
        inner = sub_inner;
        button_response = Some(response);
    });

    let response = button_response.expect("submenu button response");
    let hovered = response.hovered() || inner.is_some();
    // Paint on the parent over the exact row so the panel stays opaque like File.
    paint_row_in_rect(ui, rect, icon, label, Trailing::Submenu, true, hovered);
    egui::InnerResponse::new(inner, response)
}

pub fn separator(ui: &mut Ui) {
    ui.add_space(3.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(MENU_WIDTH, 1.0), Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        egui::Stroke::new(1.0_f32, Color32::from_rgb(220, 222, 226)),
    );
    ui.add_space(3.0);
}

fn paint_row(
    ui: &mut Ui,
    icon: Option<Icon>,
    label: &str,
    trail: Trailing<'_>,
    enabled: bool,
) -> egui::Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(MENU_WIDTH, ROW_H), sense);
    let hovered = enabled && (resp.hovered() || resp.highlighted() || resp.has_focus());
    paint_row_in_rect(ui, rect, icon, label, trail, enabled, hovered);
    resp
}

fn paint_row_in_rect(
    ui: &Ui,
    rect: egui::Rect,
    icon: Option<Icon>,
    label: &str,
    trail: Trailing<'_>,
    enabled: bool,
    hovered: bool,
) {
    // Solid base — never rely on menu-style transparent weak_bg_fill.
    ui.painter()
        .rect_filled(rect, 0.0, ui.visuals().window_fill());

    if hovered {
        ui.painter()
            .rect_filled(rect.shrink2(egui::vec2(2.0, 1.0)), 3.0, ROW_HOVER);
    }

    let fg = if enabled {
        if hovered {
            ui.style().visuals.widgets.hovered.fg_stroke.color
        } else {
            ui.style().visuals.widgets.inactive.fg_stroke.color
        }
    } else {
        ui.visuals().weak_text_color()
    };
    let trail_color = if enabled {
        Color32::from_rgb(120, 124, 132)
    } else {
        ui.visuals().weak_text_color()
    };

    let mut x = rect.min.x + PAD_X;
    let y = rect.center().y;

    if let Some(ic) = icon {
        ui.painter().text(
            egui::pos2(x, y),
            Align2::LEFT_CENTER,
            ui_icon::glyph_or_dot(ic),
            ui_icon::font_id(ic, ICON_SIZE),
            fg,
        );
    }
    x += ICON_SLOT;

    let label_right = rect.max.x - PAD_X - TRAIL_SLOT;
    let label_w = (label_right - x).max(8.0);
    ui.painter().text(
        egui::pos2(x, y),
        Align2::LEFT_CENTER,
        truncate_to_width(ui, label, FontId::proportional(13.0), label_w),
        FontId::proportional(13.0),
        fg,
    );

    let trail_right = rect.max.x - PAD_X;
    match trail {
        Trailing::Empty => {}
        Trailing::Text(sc) => {
            ui.painter().text(
                egui::pos2(trail_right, y),
                Align2::RIGHT_CENTER,
                sc,
                FontId::proportional(11.0),
                trail_color,
            );
        }
        Trailing::Submenu => {
            let ic = Icon::ChevronRight;
            ui.painter().text(
                egui::pos2(trail_right, y),
                Align2::RIGHT_CENTER,
                ui_icon::glyph_or_dot(ic),
                ui_icon::font_id(ic, ICON_SIZE),
                trail_color,
            );
        }
        Trailing::Check => {
            let ic = Icon::Check;
            ui.painter().text(
                egui::pos2(trail_right, y),
                Align2::RIGHT_CENTER,
                ui_icon::glyph_or_dot(ic),
                ui_icon::font_id(ic, ICON_SIZE),
                fg,
            );
        }
    }
}

fn truncate_to_width(ui: &Ui, text: &str, font: FontId, max_w: f32) -> String {
    let galley = ui.fonts(|f| f.layout_no_wrap(text.to_string(), font.clone(), Color32::WHITE));
    if galley.size().x <= max_w {
        return text.to_string();
    }
    let ellipsis = "…";
    let mut out = String::new();
    for ch in text.chars() {
        let trial = format!("{out}{ch}{ellipsis}");
        let g = ui.fonts(|f| f.layout_no_wrap(trial.clone(), font.clone(), Color32::WHITE));
        if g.size().x > max_w {
            break;
        }
        out.push(ch);
    }
    if out.is_empty() {
        ellipsis.into()
    } else {
        format!("{out}{ellipsis}")
    }
}
