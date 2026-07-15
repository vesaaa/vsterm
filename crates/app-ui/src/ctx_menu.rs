//! Unified context-menu chrome (WindTerm-style rows).
//!
//! Every right-click menu should call [`prepare`] once, then use [`item`] /
//! [`separator`] so width, padding, icon, label, and optional shortcut align.

use crate::ui_icon::{self, Icon};
use egui::{Align2, Color32, FontId, Sense, Ui};

/// Fixed menu width (popup `available_width` is huge — never size rows from it).
pub const MENU_WIDTH: f32 = 200.0;
const ROW_H: f32 = 26.0;
const PAD_X: f32 = 8.0;
const ICON_SIZE: f32 = 13.0;
const ICON_SLOT: f32 = 18.0;

/// Call at the start of every `context_menu` body.
pub fn prepare(ui: &mut Ui) {
    ui.set_min_width(MENU_WIDTH);
    ui.set_max_width(MENU_WIDTH);
    ui.spacing_mut().item_spacing.y = 1.0;
    ui.spacing_mut().button_padding = egui::vec2(0.0, 0.0);
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
/// When `shortcut` is `None`/empty the shortcut column is blank, but width stays shared.
pub fn item(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    shortcut: Option<&str>,
    enabled: bool,
) -> egui::Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(MENU_WIDTH, ROW_H), sense);

    let visuals = ui.style().interact(&resp);
    let fg = if enabled {
        visuals.fg_stroke.color
    } else {
        ui.visuals().weak_text_color()
    };
    let shortcut_color = if enabled {
        Color32::from_rgb(120, 124, 132)
    } else {
        ui.visuals().weak_text_color()
    };

    if enabled && (resp.hovered() || resp.highlighted() || resp.has_focus()) {
        ui.painter().rect_filled(
            rect.shrink2(egui::vec2(2.0, 1.0)),
            3.0,
            visuals.bg_fill,
        );
    }

    let mut x = rect.min.x + PAD_X;
    let y = rect.center().y;

    ui.painter().text(
        egui::pos2(x, y),
        Align2::LEFT_CENTER,
        ui_icon::glyph_or_dot(icon),
        ui_icon::font_id(icon, ICON_SIZE),
        fg,
    );
    x += ICON_SLOT;

    // Leave room for optional shortcut on the right.
    let label_right = if shortcut.filter(|s| !s.is_empty()).is_some() {
        rect.max.x - PAD_X - 36.0
    } else {
        rect.max.x - PAD_X
    };
    let label_w = (label_right - x).max(8.0);
    ui.painter().text(
        egui::pos2(x, y),
        Align2::LEFT_CENTER,
        truncate_to_width(ui, label, FontId::proportional(13.0), label_w),
        FontId::proportional(13.0),
        fg,
    );

    if let Some(sc) = shortcut.filter(|s| !s.is_empty()) {
        ui.painter().text(
            egui::pos2(rect.max.x - PAD_X, y),
            Align2::RIGHT_CENTER,
            sc,
            FontId::proportional(11.0),
            shortcut_color,
        );
    }

    resp
}

pub fn separator(ui: &mut Ui) {
    ui.add_space(3.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(MENU_WIDTH, 1.0), Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        egui::Stroke::new(1.0, Color32::from_rgb(220, 222, 226)),
    );
    ui.add_space(3.0);
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
