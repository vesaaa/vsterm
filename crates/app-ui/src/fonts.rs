//! Embedded fonts: JetBrains Mono (terminal) + Noto Sans SC (UI Chinese).
//!
//! Sources under `assets/fonts/` (SIL OFL 1.1). No system font dependency.

use egui::{FontData, FontDefinitions, FontFamily, FontId, TextStyle};
use std::sync::Arc;

const JETBRAINS_MONO_REGULAR: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");
const JETBRAINS_MONO_BOLD: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono-Bold.ttf");
const NOTO_SANS_SC_REGULAR: &[u8] =
    include_bytes!("../../../assets/fonts/NotoSansSC-Regular.otf");

const FAMILY_UI: &str = "NotoSansSC";
const FAMILY_MONO: &str = "JetBrainsMono";
const FAMILY_MONO_BOLD: &str = "JetBrainsMonoBold";

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        FAMILY_UI.to_owned(),
        Arc::new(FontData::from_static(NOTO_SANS_SC_REGULAR)),
    );
    fonts.font_data.insert(
        FAMILY_MONO.to_owned(),
        Arc::new(FontData::from_static(JETBRAINS_MONO_REGULAR)),
    );
    fonts.font_data.insert(
        FAMILY_MONO_BOLD.to_owned(),
        Arc::new(FontData::from_static(JETBRAINS_MONO_BOLD)),
    );

    // Proportional UI: Noto Sans SC first for Chinese menus / labels.
    if let Some(fam) = fonts.families.get_mut(&FontFamily::Proportional) {
        fam.insert(0, FAMILY_UI.to_owned());
        fam.push(FAMILY_MONO.to_owned());
    }

    // Monospace terminal: JetBrains Mono first, CJK as fallback.
    if let Some(fam) = fonts.families.get_mut(&FontFamily::Monospace) {
        fam.insert(0, FAMILY_MONO.to_owned());
        fam.push(FAMILY_UI.to_owned());
    }

    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();
    style.text_styles.insert(
        TextStyle::Body,
        FontId::new(14.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(14.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(18.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(13.0, FontFamily::Monospace),
    );
    ctx.set_style(style);

    tracing::info!(
        "embedded fonts: {FAMILY_UI} ({} KB) + {FAMILY_MONO} ({} KB)",
        NOTO_SANS_SC_REGULAR.len() / 1024,
        JETBRAINS_MONO_REGULAR.len() / 1024
    );
}
