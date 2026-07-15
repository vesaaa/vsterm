//! Cross-platform UI fonts: OS native CJK on Windows/macOS; embed only on Linux.
//!
//! | Platform | UI proportional                         | Terminal mono        |
//! |----------|-----------------------------------------|----------------------|
//! | Windows  | Microsoft YaHei Light (`msyhl.ttc`)     | JetBrains Mono       |
//! | macOS    | PingFang SC / Heiti SC Light            | JetBrains Mono       |
//! | Linux    | System Noto CJK when present, else embed| JetBrains Mono       |
//!
//! Noto Sans SC Light is embedded **only on Linux** so Windows/macOS builds stay small.
//! Loading system fonts for local rendering (not redistributing them) matches normal
//! desktop-app practice.

use egui::{FontData, FontDefinitions, FontFamily, FontId, TextStyle};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const JETBRAINS_MONO_REGULAR: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");

#[cfg(target_os = "linux")]
const NOTO_SANS_SC_LIGHT: &[u8] =
    include_bytes!("../../../assets/fonts/NotoSansSC-Light.otf");

const FAMILY_UI: &str = "VsTermUI";
const FAMILY_MONO: &str = "JetBrainsMono";
#[cfg(target_os = "linux")]
const FAMILY_FALLBACK_CJK: &str = "NotoSansSC-Light";

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        FAMILY_MONO.to_owned(),
        Arc::new(FontData::from_static(JETBRAINS_MONO_REGULAR)),
    );

    #[cfg(target_os = "linux")]
    {
        fonts.font_data.insert(
            FAMILY_FALLBACK_CJK.to_owned(),
            Arc::new(FontData::from_static(NOTO_SANS_SC_LIGHT)),
        );
    }

    let default_proportional = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();

    let (ui_source, ui_family) = match load_platform_ui_font() {
        Some((label, data)) => {
            fonts
                .font_data
                .insert(FAMILY_UI.to_owned(), Arc::new(data));
            (label, Some(FAMILY_UI.to_owned()))
        }
        None => {
            #[cfg(target_os = "linux")]
            {
                (
                    "embedded Noto Sans SC Light".into(),
                    Some(FAMILY_FALLBACK_CJK.to_owned()),
                )
            }
            #[cfg(not(target_os = "linux"))]
            {
                ("egui default (no system CJK font found)".into(), None)
            }
        }
    };

    if let Some(ref ui) = ui_family {
        // Keep egui defaults behind the UI font so rare symbols (menu ▸ / emoji)
        // still resolve instead of rendering as `?`.
        let mut proportional = vec![ui.clone()];
        #[cfg(target_os = "linux")]
        {
            if ui.as_str() != FAMILY_FALLBACK_CJK {
                proportional.push(FAMILY_FALLBACK_CJK.to_owned());
            }
        }
        for name in &default_proportional {
            if !proportional.iter().any(|n| n == name) {
                proportional.push(name.clone());
            }
        }
        fonts
            .families
            .insert(FontFamily::Proportional, proportional);
    }

    let mut mono = vec![FAMILY_MONO.to_owned()];
    if let Some(ref ui) = ui_family {
        mono.push(ui.clone());
    }
    #[cfg(target_os = "linux")]
    {
        if ui_family.as_deref() != Some(FAMILY_FALLBACK_CJK) {
            mono.push(FAMILY_FALLBACK_CJK.to_owned());
        }
    }
    if ui_family.is_none() {
        // Still cover CJK (where possible) via egui's default proportional stack.
        mono.extend(default_proportional);
    }
    fonts.families.insert(FontFamily::Monospace, mono);

    register_lucide_fonts(&mut fonts);

    ctx.set_fonts(fonts);
    apply_text_styles(ctx);

    #[cfg(target_os = "linux")]
    tracing::info!(
        "fonts: UI={ui_source}; mono=JetBrains Mono; CJK fallback=Noto Sans SC Light ({} KB); icons=Lucide",
        NOTO_SANS_SC_LIGHT.len() / 1024
    );
    #[cfg(not(target_os = "linux"))]
    tracing::info!("fonts: UI={ui_source}; mono=JetBrains Mono; CJK=system only; icons=Lucide");
}

fn register_lucide_fonts(fonts: &mut FontDefinitions) {
    for asset in iconflow::fonts() {
        let family = asset.family.to_string();
        fonts
            .font_data
            .insert(family.clone(), Arc::new(FontData::from_static(asset.bytes)));
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
    let windir = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
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
