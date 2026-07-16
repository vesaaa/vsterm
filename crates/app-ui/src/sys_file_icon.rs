//! OS file / folder icons for lists and trees (Lucide fallback when unavailable).

use crate::ui_icon::{self, Icon};
use egui::{Color32, TextureHandle, Ui};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

thread_local! {
    static TEXTURES: std::cell::RefCell<HashMap<IconKey, TextureHandle>> =
        std::cell::RefCell::new(HashMap::new());
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileIconKind {
    Folder,
    File,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
enum IconKey {
    Folder,
    FileExt(String),
}

const ICON_SRC_PX: u32 = 32;

/// Load generic folder/file icons on the main thread before the first frame.
pub fn warm_up() {
    static DONE: OnceLock<()> = OnceLock::new();
    DONE.get_or_init(|| {
        let _ = load_rgba(IconKey::Folder);
        let _ = load_rgba(IconKey::FileExt("txt".into()));
    });
}

pub fn add_icon(ui: &mut Ui, kind: FileIconKind, size_px: f32, fallback: Icon) -> egui::Response {
    let key = match kind {
        FileIconKind::Folder => IconKey::Folder,
        FileIconKind::File => IconKey::FileExt("txt".into()),
    };
    add_icon_key(ui, &key, size_px, fallback)
}

pub fn paint_entry(ui: &mut Ui, name: &str, is_dir: bool, rect: egui::Rect, size_px: f32) {
    let key = icon_key(name, is_dir);
    let fallback = if is_dir { Icon::Folder } else { Icon::File };
    paint_key(ui, &key, rect, fallback, size_px);
}

pub fn paint(
    ui: &mut Ui,
    kind: FileIconKind,
    rect: egui::Rect,
    fallback: Icon,
    size_px: f32,
) {
    let key = match kind {
        FileIconKind::Folder => IconKey::Folder,
        FileIconKind::File => IconKey::FileExt("txt".into()),
    };
    paint_key(ui, &key, rect, fallback, size_px);
}

fn add_icon_key(ui: &mut Ui, key: &IconKey, size_px: f32, fallback: Icon) -> egui::Response {
    let size = egui::vec2(size_px, size_px);
    if let Some(tex) = texture(ui.ctx(), key) {
        ui.add(
            egui::Image::new((tex.id(), size))
                .fit_to_exact_size(size)
                .maintain_aspect_ratio(true),
        )
    } else {
        ui.label(ui_icon::rich(fallback, size_px, ui_icon::COLOR_MUTED))
    }
}

fn paint_key(ui: &mut Ui, key: &IconKey, rect: egui::Rect, fallback: Icon, size_px: f32) {
    let size = egui::vec2(size_px, size_px);
    let pos = egui::pos2(rect.left() + 2.0, rect.center().y - size_px * 0.5);
    let icon_rect = egui::Rect::from_min_size(pos, size);
    if let Some(tex) = texture(ui.ctx(), key) {
        ui.painter().image(
            tex.id(),
            icon_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        paint_lucide(ui, fallback, icon_rect);
    }
}

fn icon_key(name: &str, is_dir: bool) -> IconKey {
    if is_dir {
        return IconKey::Folder;
    }
    let ext = Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("txt")
        .to_ascii_lowercase();
    IconKey::FileExt(ext)
}

fn paint_lucide(ui: &Ui, icon: Icon, rect: egui::Rect) {
    let g = ui.fonts(|f| {
        f.layout_no_wrap(
            ui_icon::glyph_or_dot(icon),
            ui_icon::font_id(icon, rect.height()),
            ui_icon::COLOR_MUTED,
        )
    });
    ui.painter().galley(
        egui::pos2(rect.left(), rect.center().y - g.size().y * 0.5),
        g,
        ui_icon::COLOR_MUTED,
    );
}

fn texture(ctx: &egui::Context, key: &IconKey) -> Option<TextureHandle> {
    if let Some(existing) = TEXTURES.with(|textures| textures.borrow().get(key).cloned()) {
        return Some(existing);
    }

    let (w, h, rgba) = load_rgba(key.clone())?;
    let image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
    let name = match key {
        IconKey::Folder => "sys_icon_folder".to_owned(),
        IconKey::FileExt(ext) => format!("sys_icon_{ext}"),
    };
    let handle = ctx.load_texture(
        name,
        image,
        egui::TextureOptions {
            magnification: egui::TextureFilter::Linear,
            minification: egui::TextureFilter::Linear,
            ..Default::default()
        },
    );
    TEXTURES.with(|textures| {
        textures.borrow_mut().insert(key.clone(), handle.clone());
    });
    Some(handle)
}

fn load_rgba(key: IconKey) -> Option<(u32, u32, Vec<u8>)> {
    if let Some(px) = try_platform_icon(&key) {
        return Some(px);
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(px) = try_linux_theme(&key) {
            return Some(px);
        }
    }
    None
}

fn try_platform_icon(key: &IconKey) -> Option<(u32, u32, Vec<u8>)> {
    let path = sample_path(key);
    let icon = file_icon_provider::get_file_icon(&path, ICON_SRC_PX as u16).ok()?;
    if icon.pixels.len() != (icon.width as usize) * (icon.height as usize) * 4 {
        return None;
    }
    Some((icon.width, icon.height, icon.pixels))
}

fn sample_path(key: &IconKey) -> PathBuf {
    match key {
        IconKey::Folder => probe_plain_folder(),
        IconKey::FileExt(ext) => probe_file_for_extension(ext),
    }
}

fn probe_plain_folder() -> PathBuf {
    let path = std::env::temp_dir().join("vsterm-icon-folder");
    let _ = std::fs::create_dir_all(&path);
    path
}

fn probe_file_for_extension(ext: &str) -> PathBuf {
    let ext = if ext.is_empty() { "txt" } else { ext };
    let path = std::env::temp_dir().join(format!("vsterm-icon-probe.{ext}"));
    if !path.exists() {
        let _ = std::fs::File::create(&path);
    }
    path
}

#[cfg(target_os = "linux")]
fn try_linux_theme(key: &IconKey) -> Option<(u32, u32, Vec<u8>)> {
    let (categories, names): (&[&str], &[&str]) = match key {
        IconKey::Folder => (&["places"], &["folder", "inode-directory"]),
        IconKey::FileExt(_) => (&["mimetypes"], &["text-x-generic", "unknown"]),
    };
    let size = ICON_SRC_PX;
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join(".local/share/icons"));
        roots.push(home.join(".icons"));
    }
    roots.push(PathBuf::from("/usr/share/icons"));
    let themes = ["Adwaita", "Yaru", "Papirus", "hicolor", "gnome", "Humanity"];
    for root in roots {
        for theme in themes {
            for cat in categories {
                for name in names {
                    let path = root
                        .join(theme)
                        .join(format!("{size}x{size}"))
                        .join(cat)
                        .join(format!("{name}.png"));
                    if let Some(px) = read_png_rgba(&path) {
                        return Some(px);
                    }
                    let path = root
                        .join(theme)
                        .join(format!("{size}x{size}@2x"))
                        .join(cat)
                        .join(format!("{name}.png"));
                    if let Some(px) = read_png_rgba(&path) {
                        return Some(px);
                    }
                }
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn read_png_rgba(path: &Path) -> Option<(u32, u32, Vec<u8>)> {
    let img = image::open(path).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    Some((w, h, img.into_raw()))
}
