use egui::{Color32, Context, Visuals};

/// Temporary light UI. Dark/light toggle will come later.
pub fn apply(ctx: &Context) {
    let mut visuals = Visuals::light();
    visuals.panel_fill = Color32::from_rgb(248, 249, 250);
    visuals.window_fill = Color32::from_rgb(255, 255, 255);
    visuals.extreme_bg_color = Color32::from_rgb(235, 237, 240);
    visuals.faint_bg_color = Color32::from_rgb(240, 242, 245);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(232, 234, 237);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(220, 224, 230);
    visuals.widgets.active.bg_fill = Color32::from_rgb(200, 210, 230);
    visuals.selection.bg_fill = Color32::from_rgb(200, 220, 250);
    visuals.override_text_color = Some(Color32::from_rgb(52, 56, 62));
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, Color32::from_rgb(52, 56, 62));
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, Color32::from_rgb(52, 56, 62));
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, Color32::from_rgb(42, 46, 52));
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, Color32::from_rgb(38, 42, 48));
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.window_margin = egui::Margin::same(8);
    ctx.set_style(style);
}
