//! Main area tabs: Terminal / System Info / Routes.

use crate::i18n;
use egui::{Color32, RichText, Ui};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MainTab {
    #[default]
    Terminal,
    SystemInfo,
    Routes,
}

pub fn show(ui: &mut Ui, tab: &mut MainTab) {
    ui.spacing_mut().item_spacing.x = 4.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);
        // Stick to glyphs that exist in common CJK UI fonts (avoid tofu / "?").
        tab_chip(ui, tab, MainTab::Terminal, None, &i18n::t("main.tab.terminal"));
        tab_chip(ui, tab, MainTab::SystemInfo, None, &i18n::t("main.tab.sysinfo"));
        tab_chip(ui, tab, MainTab::Routes, None, &i18n::t("main.tab.routes"));
    });
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
    if ui.add(btn).clicked() {
        *current = value;
    }
}
