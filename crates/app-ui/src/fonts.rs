//! Cross-platform UI fonts: prefer the OS native light CJK face; embed only as fallback.
//!
//! | Platform | UI proportional                         | Terminal mono        |
//! |----------|-----------------------------------------|----------------------|
//! | Windows  | Microsoft YaHei Light (`msyhl.ttc`)     | JetBrains Mono       |
//! | macOS    | PingFang SC / Heiti SC Light            | JetBrains Mono       |
//! | Linux    | System Noto CJK when present, else embed| JetBrains Mono       |
//!
//! Embedded Noto Sans SC Light remains the universal fallback so Chinese never tofu.
//! Loading system fonts for local rendering (not redistributing them) matches normal
//! desktop-app practice.

use egui::{FontData, FontDefinitions, FontFamily, FontId, TextStyle};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const JETBRAINS_MONO_REGULAR: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");
const NOTO_SANS_SC_LIGHT: &[u8] =
    include_bytes!("../../../assets/fonts/NotoSansSC-Light.otf");

const FAMILY_UI: &str = "VsTermUI";
const FAMILY_MONO: &str = "JetBrainsMono";
const FAMILY_FALLBACK_CJK: &str = "NotoSansSC-Light";

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        FAMILY_MONO.to_owned(),
        Arc::new(FontData::from_static(JETBRAINS_MONO_REGULAR)),
    );
    fonts.font_data.insert(
        FAMILY_FALLBACK_CJK.to_owned(),
        Arc::new(FontData::from_static(NOTO_SANS_SC_LIGHT)),
    );

    let (ui_name, ui_source) = match load_platform_ui_font() {
        Some((label, data)) => {
            fonts.font_data.insert(FAMILY_UI.to_owned(), Arc::new(data));
            (FAMILY_UI.to_owned(), label)
        }
        None => (FAMILY_FALLBACK_CJK.to_owned(), "embedded Noto Sans SC Light".into()),
    };

    // Proportional: system (or embedded) UI first, then embedded CJK as glyph coverage backup.
    let mut proportional = vec![ui_name.clone()];
    if ui_name != FAMILY_FALLBACK_CJK {
        proportional.push(FAMILY_FALLBACK_CJK.to_owned());
    }
    fonts
        .families
        .insert(FontFamily::Proportional, proportional);

    fonts.families.insert(
        FontFamily::Monospace,
        vec![FAMILY_MONO.to_owned(), ui_name.clone(), FAMILY_FALLBACK_CJK.to_owned()],
    );

    // Lucide icon font (flat stroke glyphs) — separate family, not used for body text.
    register_lucide_fonts(&mut fonts);

    ctx.set_fonts(fonts);
    apply_text_styles(ctx);

    tracing::info!(
        "fonts: UI={ui_source}; mono=JetBrains Mono; CJK fallback=Noto Sans SC Light ({} KB); icons=Lucide",
        NOTO_SANS_SC_LIGHT.len() / 1024
    );
}

fn register_lucide_fonts(fonts: &mut FontDefinitions) {
    for asset in iconflow::fonts() {
        let family = asset.family.to_string();
        fonts
            .font_data
            .insert(family.clone(), Arc::new(FontData::from_static(asset.bytes)));
        // Dedicated named family so IconId can select Lucide without CJK fallback swapping glyphs.
        fonts
            .families
            .insert(FontFamily::Name(family.clone().into()), vec![family]);
    }
}

fn apply_text_styles(ctx: &egui::Context) {
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
}

fn load_platform_ui_font() -> Option<(String, FontData)> {
    #[cfg(target_os = "windows")]
    {
        return load_windows_ui_font();
    }
    #[cfg(target_os = "macos")]
    {
        return load_macos_ui_font();
    }
    #[cfg(target_os = "linux")]
    {
        return load_linux_ui_font();
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

#[cfg(target_os = "windows")]
fn load_windows_ui_font() -> Option<(String, FontData)> {
    // 微软雅黑细体 → 微软雅黑 → UI 变体
    let windir = std::env::var_os("WINDIR").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(r"C:\Windows")
    });
    let fonts_dir = windir.join("Fonts");
    let candidates: &[(&str, u32, &str)] = &[
        ("msyhl.ttc", 0, "Microsoft YaHei Light"),
        ("msyhl.ttc", 1, "Microsoft YaHei UI Light"),
        ("msyh.ttc", 0, "Microsoft YaHei"),
        ("msyh.ttc", 1, "Microsoft YaHei UI"),
    ];
    try_load_named(&fonts_dir, candidates)
}

#[cfg(target_os = "macos")]
fn load_macos_ui_font() -> Option<(String, FontData)> {
    // Prefer modern PingFang SC Light; fall back to classic Heiti SC Light / Songti.
    // Face indices vary by macOS version — probe a few common Light slots.
    let candidates: &[(&str, &[u32], &str)] = &[
        (
            "/System/Library/Fonts/PingFang.ttc",
            &[3, 2, 1, 0, 4, 5],
            "PingFang SC",
        ),
        (
            "/System/Library/Fonts/STHeiti Light.ttc",
            &[0, 1],
            "Heiti SC Light",
        ),
        (
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            &[0, 1, 2],
            "Hiragino Sans GB",
        ),
        (
            "/System/Library/Fonts/Supplemental/Songti.ttc",
            &[0, 1],
            "Songti SC",
        ),
    ];
    for (path, indices, label) in candidates {
        if let Some(data) = try_load_ttc(Path::new(path), indices) {
            return Some(((*label).into(), data));
        }
    }
    // Newer macOS may keep PingFang under MobileAsset after download.
    if let Ok(entries) = std::fs::read_dir("/System/Library/AssetsV2") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.contains("Font") {
                continue;
            }
            let asset = entry.path().join(".asset/AssetData/PingFang.ttc");
            if let Some(data) = try_load_ttc(&asset, &[3, 2, 1, 0, 4, 5]) {
                return Some(("PingFang SC (Asset)".into(), data));
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn load_linux_ui_font() -> Option<(String, FontData)> {
    // Prefer a system Noto CJK Light when distro ships it; else caller uses embed.
    let candidates: &[(&str, u32, &str)] = &[
        (
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Light.ttc",
            0,
            "Noto Sans CJK Light",
        ),
        (
            "/usr/share/fonts/opentype/noto/NotoSansCJKsc-Light.otf",
            0,
            "Noto Sans CJK SC Light",
        ),
        (
            "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Light.ttc",
            0,
            "Noto Sans CJK Light",
        ),
        (
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Light.ttc",
            0,
            "Noto Sans CJK Light",
        ),
        (
            "/usr/share/fonts/truetype/noto/NotoSansSC-Light.otf",
            0,
            "Noto Sans SC Light",
        ),
    ];
    for (path, index, label) in candidates {
        if let Some(data) = read_font_file(Path::new(path), *index) {
            return Some(((*label).into(), data));
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn try_load_named(dir: &Path, candidates: &[(&str, u32, &str)]) -> Option<(String, FontData)> {
    for (file, index, label) in candidates {
        let path = dir.join(file);
        if let Some(data) = read_font_file(&path, *index) {
            return Some(((*label).into(), data));
        }
    }
    None
}

fn try_load_ttc(path: &Path, indices: &[u32]) -> Option<FontData> {
    let index = *indices.first()?;
    read_font_file(path, index)
}

fn read_font_file(path: &Path, index: u32) -> Option<FontData> {
    if !path.is_file() {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() < 64 {
        return None;
    }
    let mut data = FontData::from_owned(bytes);
    data.index = index;
    Some(data)
}
