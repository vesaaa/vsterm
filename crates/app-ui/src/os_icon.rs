//! Per-session OS icons (stylized badges under `assets/icons/os/`).

use egui::{Color32, TextureHandle, Ui};
use std::collections::HashMap;

/// Known icon ids (also used as YAML `icon:` values).
pub const PRESETS: &[&str] = &[
    "debian", "ubuntu", "centos", "rocky", "rhel", "fedora", "arch", "alpine",
    "opensuse", "macos", "windows", "openwrt", "merlin", "freebsd", "linux",
    "deepin", "openeuler", "openkylin", "harmonyos", "almalinux",
];

/// Default / unknown-id fallback (Tux).
const FALLBACK_ID: &str = "linux";

thread_local! {
    static TEXTURES: std::cell::RefCell<HashMap<&'static str, TextureHandle>> =
        std::cell::RefCell::new(HashMap::new());
}

fn png_bytes(id: &str) -> Option<&'static [u8]> {
    Some(match id {
        "debian" => include_bytes!("../../../assets/icons/os/debian.png").as_slice(),
        "ubuntu" => include_bytes!("../../../assets/icons/os/ubuntu.png").as_slice(),
        "centos" => include_bytes!("../../../assets/icons/os/centos.png").as_slice(),
        "rocky" => include_bytes!("../../../assets/icons/os/rocky.png").as_slice(),
        "rhel" => include_bytes!("../../../assets/icons/os/rhel.png").as_slice(),
        "fedora" => include_bytes!("../../../assets/icons/os/fedora.png").as_slice(),
        "arch" => include_bytes!("../../../assets/icons/os/arch.png").as_slice(),
        "alpine" => include_bytes!("../../../assets/icons/os/alpine.png").as_slice(),
        "opensuse" => include_bytes!("../../../assets/icons/os/opensuse.png").as_slice(),
        "macos" => include_bytes!("../../../assets/icons/os/macos.png").as_slice(),
        "windows" => include_bytes!("../../../assets/icons/os/windows.png").as_slice(),
        "openwrt" => include_bytes!("../../../assets/icons/os/openwrt.png").as_slice(),
        "merlin" => include_bytes!("../../../assets/icons/os/merlin.png").as_slice(),
        "freebsd" => include_bytes!("../../../assets/icons/os/freebsd.png").as_slice(),
        "linux" => include_bytes!("../../../assets/icons/os/linux.png").as_slice(),
        "deepin" => include_bytes!("../../../assets/icons/os/deepin.png").as_slice(),
        "openeuler" => include_bytes!("../../../assets/icons/os/openeuler.png").as_slice(),
        "openkylin" => include_bytes!("../../../assets/icons/os/openkylin.png").as_slice(),
        "harmonyos" => include_bytes!("../../../assets/icons/os/harmonyos.png").as_slice(),
        "almalinux" => include_bytes!("../../../assets/icons/os/almalinux.png").as_slice(),
        _ => return None,
    })
}

fn texture(ctx: &egui::Context, id: &str) -> Option<TextureHandle> {
    let key = PRESETS.iter().copied().find(|p| *p == id)?;
    TEXTURES.with(|map| {
        if let Some(t) = map.borrow().get(key).cloned() {
            return Some(t);
        }
        let bytes = png_bytes(key)?;
        let img = image::load_from_memory(bytes).ok()?.into_rgba8();
        let size = [img.width() as usize, img.height() as usize];
        let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
        let tex = ctx.load_texture(format!("os_icon_{key}"), color, egui::TextureOptions::LINEAR);
        map.borrow_mut().insert(key, tex.clone());
        Some(tex)
    })
}

fn resolve_texture(ctx: &egui::Context, icon_id: Option<&str>) -> Option<TextureHandle> {
    if let Some(id) = icon_id {
        if let Some(tex) = texture(ctx, id) {
            return Some(tex);
        }
    }
    texture(ctx, FALLBACK_ID)
}

fn paint_texture(ui: &Ui, tex: &TextureHandle, rect: egui::Rect, size_px: f32) {
    let size = egui::vec2(size_px, size_px);
    let pos = egui::pos2(
        rect.left() + (rect.width() - size_px) * 0.5,
        rect.center().y - size_px * 0.5,
    );
    ui.painter().image(
        tex.id(),
        egui::Rect::from_min_size(pos, size),
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
}

/// Paint a 16px OS badge, or Linux Tux fallback.
pub fn paint(ui: &Ui, icon_id: Option<&str>, rect: egui::Rect, size_px: f32) {
    if let Some(tex) = resolve_texture(ui.ctx(), icon_id) {
        paint_texture(ui, &tex, rect, size_px);
    }
}

/// Inline icon widget (tight spacing with following label), Linux Tux fallback.
pub fn add(ui: &mut Ui, icon_id: Option<&str>, size_px: f32) -> egui::Response {
    let size = egui::vec2(size_px, size_px);
    if let Some(tex) = resolve_texture(ui.ctx(), icon_id) {
        return ui.add(
            egui::Image::new((tex.id(), size))
                .fit_to_exact_size(size)
                .maintain_aspect_ratio(true),
        );
    }
    // Texture load failed — keep layout with an empty spacer.
    ui.allocate_exact_size(size, egui::Sense::hover()).1
}

/// Clickable preset tile for the session editor.
pub fn selectable(ui: &mut Ui, id: &str, selected: bool, size_px: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size_px + 4.0, size_px + 4.0), egui::Sense::click());
    let inner = egui::Rect::from_center_size(rect.center(), egui::vec2(size_px, size_px));
    paint(ui, Some(id), inner, size_px);
    if selected {
        ui.painter().rect_stroke(
            rect,
            3.0,
            egui::Stroke::new(2.0_f32, Color32::from_rgb(40, 40, 50)),
            egui::StrokeKind::Outside,
        );
    }
    resp
}

/// Map `/etc/os-release` fields (+ uname) to a preset icon id.
pub fn detect_id_from_release(
    uname_s: &str,
    os_id: &str,
    id_like: &str,
    pretty: &str,
    openwrt_hint: bool,
    merlin_hint: bool,
    windows_hint: bool,
) -> Option<&'static str> {
    let uname = uname_s.trim().to_ascii_lowercase();
    if uname.contains("darwin") {
        return Some("macos");
    }
    // Native Windows shells / OpenSSH environments.
    if windows_hint
        || uname.contains("mingw")
        || uname.contains("msys")
        || uname.contains("cygwin")
        || uname.contains("windows_nt")
        || uname == "windows"
    {
        return Some("windows");
    }

    let pretty_l = pretty.trim().to_ascii_lowercase();
    let id = os_id.trim().to_ascii_lowercase();
    // Merlin / Asuswrt / Koolshare 官改·改版 — before generic OpenWrt.
    if merlin_hint
        || pretty_l.contains("merlin")
        || pretty_l.contains("asuswrt")
        || pretty_l.contains("koolshare")
        || id.contains("merlin")
        || id.contains("asuswrt")
        || id.contains("koolshare")
    {
        return Some("merlin");
    }
    if openwrt_hint || id == "openwrt" || pretty_l.contains("openwrt") {
        return Some("openwrt");
    }

    let like = id_like.trim().to_ascii_lowercase();

    let try_one = |s: &str| -> Option<&'static str> {
        if s.is_empty() {
            return None;
        }
        if s == "ubuntu" || s.contains("ubuntu") {
            return Some("ubuntu");
        }
        if s == "debian" || s.contains("debian") {
            return Some("debian");
        }
        if s == "centos" || s.contains("centos") {
            return Some("centos");
        }
        if s == "rocky" || s.contains("rocky") {
            return Some("rocky");
        }
        if s == "alma" || s == "almalinux" || s.contains("almalinux") {
            return Some("almalinux");
        }
        if s == "rhel" || s == "redhat" || s.contains("red hat") || s.contains("redhat") {
            return Some("rhel");
        }
        if s == "fedora" || s.contains("fedora") {
            return Some("fedora");
        }
        if s == "arch" || s == "archlinux" {
            return Some("arch");
        }
        if s == "alpine" || s.contains("alpine") {
            return Some("alpine");
        }
        if s.starts_with("opensuse") || s == "suse" || s.contains("opensuse") {
            return Some("opensuse");
        }
        if s == "freebsd" || s.contains("freebsd") {
            return Some("freebsd");
        }
        if s == "deepin" || s == "uos" || s.contains("deepin") || s.contains("uniontech") {
            return Some("deepin");
        }
        if s == "openeuler" || s.contains("openeuler") {
            return Some("openeuler");
        }
        if s == "openkylin" || s == "kylin" || s.contains("openkylin") || s.contains("kylin") {
            return Some("openkylin");
        }
        if s == "harmonyos" || s.contains("harmonyos") {
            return Some("harmonyos");
        }
        None
    };

    try_one(&id)
        .or_else(|| try_one(&pretty_l))
        .or_else(|| {
            for part in like.split_whitespace() {
                if let Some(m) = try_one(part) {
                    return Some(m);
                }
            }
            None
        })
}

/// Display label key for i18n: `dialog.session.icon.<id>`
pub fn i18n_key(id: &str) -> String {
    format!("dialog.session.icon.{id}")
}
