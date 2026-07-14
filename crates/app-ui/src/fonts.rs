//! Embedded fonts: JetBrains Mono (terminal) + Noto Sans SC Light (UI).
//!
//! Light weight matches thin UI fonts used by tools like FinalShell / WindTerm.
//! Sources under `assets/fonts/` (SIL OFL 1.1).

use egui::{FontData, FontDefinitions, FontFamily, FontId, TextStyle};
use std::sync::Arc;

const JETBRAINS_MONO_REGULAR: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");
const NOTO_SANS_SC_LIGHT: &[u8] =
    include_bytes!("../../../assets/fonts/NotoSansSC-Light.otf");

const FAMILY_UI: &str = "NotoSansSC-Light";
const FAMILY_MONO: &str = "JetBrainsMono";

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        FAMILY_UI.to_owned(),
        Arc::new(FontData::from_static(NOTO_SANS_SC_LIGHT)),
    );
    fonts.font_data.insert(
        FAMILY_MONO.to_owned(),
        Arc::new(FontData::from_static(JETBRAINS_MONO_REGULAR)),
    );

    // UI: only our light CJK font (drop egui default Ubuntu/Hack to avoid mixed weights).
    fonts
        .families
        .insert(FontFamily::Proportional, vec![FAMILY_UI.to_owned()]);
    fonts.families.insert(
        FontFamily::Monospace,
        vec![FAMILY_MONO.to_owned(), FAMILY_UI.to_owned()],
    );

    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();
    style.text_styles.insert(
        TextStyle::Body,
        FontId::new(13.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(13.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(12.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(15.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(13.0, FontFamily::Monospace),
    );
    ctx.set_style(style);

    tracing::info!(
        "embedded fonts: {FAMILY_UI} ({} KB) + {FAMILY_MONO} ({} KB)",
        NOTO_SANS_SC_LIGHT.len() / 1024,
        JETBRAINS_MONO_REGULAR.len() / 1024
    );
}
