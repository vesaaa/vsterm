//! Lucide UI icons via iconflow (flat monochrome stroke glyphs).

use egui::{Color32, FontFamily, FontId, RichText, Ui};
use iconflow::{try_icon, Pack, Size, Style};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;

/// Default muted icon color (light UI chrome).
pub const COLOR_MUTED: Color32 = Color32::from_rgb(70, 74, 82);
/// Selected / accent icon color.
pub const COLOR_ACCENT: Color32 = Color32::from_rgb(30, 80, 160);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    Folder,
    FolderUp,
    FolderPlus,
    Server,
    File,
    Copy,
    Paste,
    Terminal,
    Pencil,
    Trash,
    Plus,
    Languages,
    Sparkles,
    Plug,
    Unplug,
    RefreshCw,
    Upload,
    Download,
    LogOut,
    Check,
    ChevronRight,
    ChevronDown,
}

impl Icon {
    fn lucide_name(self) -> &'static str {
        match self {
            Icon::Folder => "folder",
            Icon::FolderUp => "folder-up",
            Icon::FolderPlus => "folder-plus",
            Icon::Server => "server",
            Icon::File => "file",
            Icon::Copy => "copy",
            Icon::Paste => "clipboard-paste",
            Icon::Terminal => "terminal",
            Icon::Pencil => "pencil",
            Icon::Trash => "trash-2",
            Icon::Plus => "plus",
            Icon::Languages => "languages",
            Icon::Sparkles => "sparkles",
            Icon::Plug => "plug",
            Icon::Unplug => "unplug",
            Icon::RefreshCw => "refresh-cw",
            Icon::Upload => "upload",
            Icon::Download => "download",
            Icon::LogOut => "log-out",
            Icon::Check => "check",
            Icon::ChevronRight => "chevron-right",
            Icon::ChevronDown => "chevron-down",
        }
    }

    fn fallback_name(self) -> Option<&'static str> {
        match self {
            Icon::Paste => Some("clipboard"),
            Icon::Trash => Some("trash"),
            Icon::FolderPlus => Some("folder"),
            Icon::FolderUp => Some("arrow-up"),
            Icon::Pencil => Some("pen-line"),
            Icon::Sparkles => Some("wand"),
            Icon::Unplug => Some("plug"),
            _ => None,
        }
    }
}

#[derive(Clone)]
struct Resolved {
    glyph: char,
    family: String,
}

static CACHE: Lazy<Mutex<HashMap<Icon, Option<Resolved>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn resolve(icon: Icon) -> Option<Resolved> {
    let mut guard = CACHE.lock().ok()?;
    if let Some(cached) = guard.get(&icon) {
        return cached.clone();
    }
    let resolved = resolve_uncached(icon);
    guard.insert(icon, resolved.clone());
    resolved
}

fn resolve_uncached(icon: Icon) -> Option<Resolved> {
    let names = std::iter::once(icon.lucide_name()).chain(icon.fallback_name());
    for name in names {
        if let Ok(r) = try_icon(Pack::Lucide, name, Style::Regular, Size::Regular) {
            let glyph = char::from_u32(r.codepoint)?;
            return Some(Resolved {
                glyph,
                family: r.family.to_string(),
            });
        }
    }
    None
}

/// Font family name for Lucide (for callers that need a FontId).
#[allow(dead_code)]
pub fn family_name(icon: Icon) -> Option<String> {
    resolve(icon).map(|r| r.family)
}

/// Glyph char or a simple fallback.
pub fn glyph_or_dot(icon: Icon) -> String {
    match resolve(icon) {
        Some(r) => r.glyph.to_string(),
        None => "·".into(),
    }
}

/// FontId for icon painting (Lucide family when available).
pub fn font_id(icon: Icon, size: f32) -> FontId {
    match resolve(icon) {
        Some(r) => FontId::new(size, FontFamily::Name(r.family.into())),
        None => FontId::proportional(size),
    }
}

/// RichText for the icon glyph alone.
pub fn rich(icon: Icon, size: f32, color: Color32) -> RichText {
    match resolve(icon) {
        Some(r) => RichText::new(r.glyph.to_string())
            .font(FontId::new(size, FontFamily::Name(r.family.into())))
            .color(color),
        None => RichText::new("·").size(size).color(color),
    }
}

/// Label composed as `[icon]  text` using mixed fonts via horizontal layout.
pub fn labeled(ui: &mut Ui, icon: Icon, text: &str, icon_size: f32, color: Color32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(rich(icon, icon_size, color));
        ui.label(RichText::new(text).color(color));
    });
}

/// Button whose label is `[icon]  text`. Returns the button response.
pub fn button(ui: &mut Ui, icon: Icon, text: &str, icon_size: f32, enabled: bool) -> egui::Response {
    use egui::text::{LayoutJob, TextFormat};

    let color = if enabled {
        ui.visuals().widgets.inactive.fg_stroke.color
    } else {
        ui.visuals().weak_text_color()
    };

    let mut job = LayoutJob::default();
    if let Some(r) = resolve(icon) {
        job.append(
            &r.glyph.to_string(),
            0.0,
            TextFormat {
                font_id: FontId::new(icon_size, FontFamily::Name(r.family.into())),
                color,
                ..Default::default()
            },
        );
        job.append(
            "  ",
            0.0,
            TextFormat {
                font_id: FontId::new(icon_size, FontFamily::Proportional),
                color,
                ..Default::default()
            },
        );
    }
    job.append(
        text,
        0.0,
        TextFormat {
            font_id: FontId::new(13.0, FontFamily::Proportional),
            color,
            ..Default::default()
        },
    );
    ui.add_enabled(enabled, egui::Button::new(job).min_size([120.0, 0.0].into()))
}

/// Prefix an existing RichText label with an icon (for tree rows).
pub fn with_label(icon: Icon, label: &str, icon_size: f32, color: Color32) -> LayoutJobOwned {
    LayoutJobOwned::compose(icon, label, icon_size, color)
}

/// Owned layout job helper so callers can pass into CollapsingHeader / Button.
pub struct LayoutJobOwned(pub egui::text::LayoutJob);

impl LayoutJobOwned {
    pub fn compose(icon: Icon, label: &str, icon_size: f32, color: Color32) -> Self {
        use egui::text::{LayoutJob, TextFormat};
        let mut job = LayoutJob::default();
        if let Some(r) = resolve(icon) {
            job.append(
                &r.glyph.to_string(),
                0.0,
                TextFormat {
                    font_id: FontId::new(icon_size, FontFamily::Name(r.family.into())),
                    color,
                    ..Default::default()
                },
            );
            job.append(
                "  ",
                0.0,
                TextFormat {
                    font_id: FontId::new(icon_size, FontFamily::Proportional),
                    color,
                    ..Default::default()
                },
            );
        }
        job.append(
            label,
            0.0,
            TextFormat {
                font_id: FontId::new(13.0, FontFamily::Proportional),
                color,
                ..Default::default()
            },
        );
        Self(job)
    }
}

impl From<LayoutJobOwned> for egui::WidgetText {
    fn from(value: LayoutJobOwned) -> Self {
        value.0.into()
    }
}
