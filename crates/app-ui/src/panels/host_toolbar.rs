//! Main area tabs: Terminal / System Info / Routes (+ terminal ops on the right).

use crate::i18n;
use crate::ui_icon::{self, Icon};
use egui::{Color32, PointerButton, RichText, Ui};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MainTab {
    #[default]
    Terminal,
    SystemInfo,
    Routes,
}

/// Actions from the right-side terminal ops control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostToolbarAction {
    /// Keep at most this many scrollback lines (`None` = clear all).
    TrimScrollback { keep: Option<usize> },
}

pub fn show(ui: &mut Ui, tab: &mut MainTab) -> Option<HostToolbarAction> {
    let mut action = None;
    ui.spacing_mut().item_spacing.x = 4.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);
        // Stick to glyphs that exist in common CJK UI fonts (avoid tofu / "?").
        tab_chip(ui, tab, MainTab::Terminal, None, &i18n::t("main.tab.terminal"));
        tab_chip(
            ui,
            tab,
            MainTab::SystemInfo,
            None,
            &i18n::t("main.tab.sysinfo"),
        );
        tab_chip(ui, tab, MainTab::Routes, None, &i18n::t("main.tab.routes"));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            action = term_ops_menu(ui);
        });
    });
    action
}

fn term_ops_menu(ui: &mut Ui) -> Option<HostToolbarAction> {
    let mut action = None;
    let icon = ui_icon::rich(Icon::Eraser, 16.0, ui_icon::COLOR_MUTED);
    let btn = egui::Button::new(icon)
        .fill(Color32::from_rgb(255, 255, 255))
        .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(200, 205, 212)))
        .corner_radius(4.0)
        .min_size([28.0, 28.0].into());
    let resp = ui.add(btn).on_hover_text(i18n::t("term.ops.tip"));
    if resp.has_focus() {
        resp.surrender_focus();
    }

    let popup_id = ui.make_persistent_id("host_toolbar_term_ops");
    if resp.clicked_by(PointerButton::Primary) {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }

    egui::popup::popup_below_widget(
        ui,
        popup_id,
        &resp,
        egui::popup::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_min_width(180.0);
            ui.label(
                RichText::new(i18n::t("term.ops.clear_scrollback"))
                    .size(12.0)
                    .weak(),
            );
            ui.separator();
            for (label_key, keep) in [
                ("term.ops.keep_2000", Some(2_000usize)),
                ("term.ops.keep_5000", Some(5_000)),
                ("term.ops.keep_10000", Some(10_000)),
                ("term.ops.clear_all", None),
            ] {
                if ui
                    .add(egui::Button::new(i18n::t(label_key)).wrap_mode(egui::TextWrapMode::Extend))
                    .clicked()
                {
                    action = Some(HostToolbarAction::TrimScrollback { keep });
                    ui.memory_mut(|m| m.close_popup());
                }
            }
        },
    );

    action
}

fn tab_chip(ui: &mut Ui, current: &mut MainTab, value: MainTab, icon: Option<&str>, label: &str) {
    let selected = *current == value;
    let fill = if selected {
        Color32::from_rgb(220, 232, 250)
    } else {
        Color32::from_rgb(255, 255, 255)
    };
    let stroke = if selected {
        Color32::from_rgb(70, 130, 200)
    } else {
        Color32::from_rgb(200, 205, 212)
    };
    let text_color = Color32::from_rgb(32, 34, 40);
    let text = match icon {
        Some(icon) => format!("{icon}  {label}"),
        None => label.to_string(),
    };
    let btn = egui::Button::new(RichText::new(text).size(13.0).color(text_color))
        .fill(fill)
        .stroke(egui::Stroke::new(1.0_f32, stroke))
        .corner_radius(4.0)
        .min_size([110.0, 28.0].into());
    let resp = ui.add(btn);
    // Mouse only — Space/Enter must not switch tabs while typing in the terminal.
    if resp.clicked_by(PointerButton::Primary) {
        *current = value;
    }
    if resp.has_focus() {
        resp.surrender_focus();
    }
}
