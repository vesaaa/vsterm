//! Shared modal dialog chrome — title size and title-bar height match the auth prompt.

use egui::{Context, Margin, RichText, Ui};

const TITLE_SIZE: f32 = 12.5;
const TITLE_INTERACT_Y: f32 = 14.0;
const TITLE_WINDOW_MARGIN: Margin = Margin::symmetric(8, 3);

/// Applies compact window title-bar metrics until dropped.
pub struct CompactTitleBar {
    ctx: Context,
    old_interact_y: f32,
    old_window_margin: Margin,
}

impl CompactTitleBar {
    pub fn push(ctx: &Context) -> Self {
        let old_interact_y = ctx.style().spacing.interact_size.y;
        let old_window_margin = ctx.style().spacing.window_margin;
        ctx.style_mut(|s| {
            s.spacing.interact_size.y = TITLE_INTERACT_Y;
            s.spacing.window_margin = TITLE_WINDOW_MARGIN;
        });
        Self {
            ctx: ctx.clone(),
            old_interact_y,
            old_window_margin,
        }
    }
}

impl Drop for CompactTitleBar {
    fn drop(&mut self) {
        self.ctx.style_mut(|s| {
            s.spacing.interact_size.y = self.old_interact_y;
            s.spacing.window_margin = self.old_window_margin;
        });
    }
}

/// Standard modal window title text (matches auth dialog).
pub fn title(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).size(TITLE_SIZE)
}

/// Dialog footnote actions — the whole button strip centered as one group.
///
/// egui is single-pass: `Align::Center` only places the *requested* `desired_size`
/// box. `Vec2::ZERO` therefore pins the origin on the midline and LTR content
/// grows to the right (exactly the “first button centered” look).
///
/// Correct approach: remember last frame’s strip width (TempData) and left-pad
/// so the group’s midpoint matches the content area midpoint.
pub fn centered_actions(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    let id = ui.id().with("centered_actions_w");
    let prev_w = ui.ctx().data(|d| d.get_temp::<f32>(id)).unwrap_or(0.0);
    let pad = if prev_w > 1.0 {
        ((ui.available_width() - prev_w) * 0.5).max(0.0)
    } else {
        0.0
    };

    ui.horizontal(|ui| {
        if pad > 0.0 {
            ui.add_space(pad);
        }
        let strip = ui.horizontal(add_contents);
        ui.ctx()
            .data_mut(|d| d.insert_temp(id, strip.response.rect.width()));
    });
}
