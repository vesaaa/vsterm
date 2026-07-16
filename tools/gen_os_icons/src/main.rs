//! Generate 64×64 OS icons from [Simple Icons](https://simpleicons.org) (CC0)
//! brand-colored silhouettes, plus a few procedural marks when SI has no slug.

use image::{ImageBuffer, Rgba, RgbaImage};
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};
use std::path::PathBuf;

const SIZE: u32 = 64;

#[derive(Clone, Copy)]
enum Source {
    /// simple-icons slug
    SimpleIcons(&'static str),
    Procedural(ProcKind),
}

#[derive(Clone, Copy)]
enum ProcKind {
    Windows,
    OpenEuler,
    OpenKylin,
}

#[derive(Clone, Copy)]
enum Layout {
    /// Colored glyph on transparent (Debian-style).
    Glyph,
    /// Brand-colored rounded tile + white glyph (for wide wordmarks like ASUS).
    Badge,
}

struct Spec {
    id: &'static str,
    source: Source,
    color: [u8; 3],
    layout: Layout,
}

const ICONS: &[Spec] = &[
    Spec { id: "debian", source: Source::SimpleIcons("debian"), color: [168, 29, 51], layout: Layout::Glyph },
    Spec { id: "ubuntu", source: Source::SimpleIcons("ubuntu"), color: [233, 84, 32], layout: Layout::Glyph },
    Spec { id: "centos", source: Source::SimpleIcons("centos"), color: [38, 37, 119], layout: Layout::Glyph },
    Spec { id: "rocky", source: Source::SimpleIcons("rockylinux"), color: [16, 185, 129], layout: Layout::Glyph },
    Spec { id: "rhel", source: Source::SimpleIcons("redhat"), color: [238, 0, 0], layout: Layout::Glyph },
    Spec { id: "fedora", source: Source::SimpleIcons("fedora"), color: [81, 162, 218], layout: Layout::Glyph },
    Spec { id: "arch", source: Source::SimpleIcons("archlinux"), color: [23, 147, 209], layout: Layout::Glyph },
    Spec { id: "alpine", source: Source::SimpleIcons("alpinelinux"), color: [13, 89, 124], layout: Layout::Glyph },
    Spec { id: "opensuse", source: Source::SimpleIcons("opensuse"), color: [115, 186, 37], layout: Layout::Glyph },
    Spec { id: "macos", source: Source::SimpleIcons("apple"), color: [85, 85, 90], layout: Layout::Glyph },
    Spec { id: "openwrt", source: Source::SimpleIcons("openwrt"), color: [0, 181, 226], layout: Layout::Glyph },
    Spec { id: "merlin", source: Source::SimpleIcons("republicofgamers"), color: [255, 0, 41], layout: Layout::Glyph },
    Spec { id: "freebsd", source: Source::SimpleIcons("freebsd"), color: [171, 43, 40], layout: Layout::Glyph },
    Spec { id: "linux", source: Source::SimpleIcons("linux"), color: [32, 34, 40], layout: Layout::Glyph },
    Spec { id: "deepin", source: Source::SimpleIcons("deepin"), color: [0, 124, 255], layout: Layout::Glyph },
    Spec { id: "harmonyos", source: Source::SimpleIcons("harmonyos"), color: [224, 0, 18], layout: Layout::Glyph },
    Spec { id: "almalinux", source: Source::SimpleIcons("almalinux"), color: [0, 82, 155], layout: Layout::Glyph },
    Spec { id: "windows", source: Source::Procedural(ProcKind::Windows), color: [0, 120, 212], layout: Layout::Glyph },
    Spec { id: "openeuler", source: Source::Procedural(ProcKind::OpenEuler), color: [42, 103, 226], layout: Layout::Glyph },
    Spec { id: "openkylin", source: Source::Procedural(ProcKind::OpenKylin), color: [45, 158, 110], layout: Layout::Glyph },
];

fn main() {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/icons/os");
    std::fs::create_dir_all(&out).expect("create assets/icons/os");

    for spec in ICONS {
        let img = match spec.source {
            Source::SimpleIcons(slug) => render_simple_icons(spec, slug),
            Source::Procedural(kind) => Ok(render_procedural(kind, spec.color)),
        }
        .unwrap_or_else(|e| panic!("failed {}: {e}", spec.id));
        let path = out.join(format!("{}.png", spec.id));
        img.save(&path)
            .unwrap_or_else(|e| panic!("save {}: {e}", path.display()));
        println!("wrote {}", path.display());
    }
}

fn render_simple_icons(spec: &Spec, slug: &str) -> Result<RgbaImage, String> {
    let url = format!("https://cdn.jsdelivr.net/npm/simple-icons@v13/icons/{slug}.svg");
    let svg_text = ureq::get(&url)
        .call()
        .map_err(|e| format!("download {url}: {e}"))?
        .into_string()
        .map_err(|e| format!("read body: {e}"))?;

    let (fill_hex, bg) = match spec.layout {
        Layout::Glyph => (format!("{:02X}{:02X}{:02X}", spec.color[0], spec.color[1], spec.color[2]), None),
        Layout::Badge => ("FFFFFF".into(), Some(spec.color)),
    };
    let fill = format!("fill=\"#{fill_hex}\"");
    let colored = svg_text
        .replace("fill=\"currentColor\"", &fill)
        .replace("fill=\"#000\"", &fill)
        .replace("fill=\"#000000\"", &fill)
        .replace("<path ", &format!("<path {fill} "))
        .replace("<path\n", &format!("<path {fill} "));

    let mut opt = Options::default();
    opt.fontdb_mut().load_system_fonts();
    let tree = Tree::from_str(&colored, &opt).map_err(|e| format!("parse svg: {e}"))?;

    match spec.layout {
        Layout::Glyph => {
            let mut pixmap = Pixmap::new(SIZE, SIZE).ok_or("pixmap")?;
            let pad = 4.0;
            let size = tree.size();
            let avail = SIZE as f32 - pad * 2.0;
            let scale = (avail / size.width()).min(avail / size.height());
            let dx = (SIZE as f32 - size.width() * scale) * 0.5;
            let dy = (SIZE as f32 - size.height() * scale) * 0.5;
            let transform = Transform::from_row(scale, 0.0, 0.0, scale, dx, dy);
            resvg::render(&tree, transform, &mut pixmap.as_mut());
            Ok(pixmap_to_image(pixmap))
        }
        Layout::Badge => {
            // Render oversized, then tight-crop the ink and fit into a solid tile.
            // Wide wordmarks (ASUS) have empty viewBox padding that would otherwise
            // leave a thin strip on a square badge.
            let hi = 256u32;
            let mut big = Pixmap::new(hi, hi).ok_or("pixmap")?;
            let size = tree.size();
            let scale = (hi as f32 / size.width()).min(hi as f32 / size.height()) * 0.92;
            let dx = (hi as f32 - size.width() * scale) * 0.5;
            let dy = (hi as f32 - size.height() * scale) * 0.5;
            let transform = Transform::from_row(scale, 0.0, 0.0, scale, dx, dy);
            resvg::render(&tree, transform, &mut big.as_mut());
            let ink = pixmap_to_image(big);
            let (x0, y0, x1, y1) = alpha_bbox(&ink).ok_or("empty svg ink")?;
            let crop_w = (x1 - x0 + 1) as f32;
            let crop_h = (y1 - y0 + 1) as f32;

            let mut out: RgbaImage = ImageBuffer::from_pixel(SIZE, SIZE, Rgba([0, 0, 0, 0]));
            let rgb = bg.unwrap();
            fill_rounded_rect_img(&mut out, rgb, 12.0);
            let pad = 7.0;
            let avail = SIZE as f32 - pad * 2.0;
            let aspect = crop_w / crop_h.max(1.0);
            // Extremely wide wordmarks: fill height and crop sides so the
            // tile reads as solid at 14px (contain would leave a thin strip).
            let fit = if aspect > 2.4 {
                avail / crop_h
            } else {
                (avail / crop_w).min(avail / crop_h)
            };
            let dw = (crop_w * fit).round().max(1.0) as u32;
            let dh = (crop_h * fit).round().max(1.0) as u32;
            let ox = (SIZE as i32 - dw as i32) / 2;
            let oy = (SIZE as i32 - dh as i32) / 2;
            blit_scaled(
                &ink,
                x0,
                y0,
                x1,
                y1,
                &mut out,
                ox,
                oy,
                dw,
                dh,
            );
            Ok(out)
        }
    }
}

fn alpha_bbox(img: &RgbaImage) -> Option<(u32, u32, u32, u32)> {
    let mut min_x = img.width();
    let mut min_y = img.height();
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut any = false;
    for y in 0..img.height() {
        for x in 0..img.width() {
            if img.get_pixel(x, y)[3] > 16 {
                any = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    any.then_some((min_x, min_y, max_x, max_y))
}

fn blit_scaled(
    src: &RgbaImage,
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
    dst: &mut RgbaImage,
    ox: i32,
    oy: i32,
    dw: u32,
    dh: u32,
) {
    let sw = (x1 - x0 + 1) as f32;
    let sh = (y1 - y0 + 1) as f32;
    for dy in 0..dh {
        for dx in 0..dw {
            let sx = x0 + ((dx as f32 + 0.5) * sw / dw as f32).floor() as u32;
            let sy = y0 + ((dy as f32 + 0.5) * sh / dh as f32).floor() as u32;
            let p = src.get_pixel(sx.min(x1), sy.min(y1));
            if p[3] < 16 {
                continue;
            }
            let x = ox + dx as i32;
            let y = oy + dy as i32;
            if x >= 0 && y >= 0 && (x as u32) < dst.width() && (y as u32) < dst.height() {
                // Premultiply onto existing badge using source alpha.
                let dest = dst.get_pixel_mut(x as u32, y as u32);
                let a = p[3] as f32 / 255.0;
                dest[0] = ((p[0] as f32) * a + dest[0] as f32 * (1.0 - a)) as u8;
                dest[1] = ((p[1] as f32) * a + dest[1] as f32 * (1.0 - a)) as u8;
                dest[2] = ((p[2] as f32) * a + dest[2] as f32 * (1.0 - a)) as u8;
                dest[3] = 255;
            }
        }
    }
}

fn fill_rounded_rect_img(img: &mut RgbaImage, rgb: [u8; 3], radius: f32) {
    let w = SIZE as f32;
    let h = SIZE as f32;
    for y in 0..SIZE {
        for x in 0..SIZE {
            if inside_rounded_rect(x as f32 + 0.5, y as f32 + 0.5, w, h, radius) {
                img.put_pixel(x, y, Rgba([rgb[0], rgb[1], rgb[2], 255]));
            }
        }
    }
}

fn inside_rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> bool {
    let r = r.min(w * 0.5).min(h * 0.5);
    let cx = x.clamp(r, w - r);
    let cy = y.clamp(r, h - r);
    let dx = x - cx;
    let dy = y - cy;
    dx * dx + dy * dy <= r * r
}

fn render_procedural(kind: ProcKind, color: [u8; 3]) -> RgbaImage {
    let mut img: RgbaImage = ImageBuffer::from_pixel(SIZE, SIZE, Rgba([0, 0, 0, 0]));
    match kind {
        ProcKind::Windows => draw_windows(&mut img, color),
        ProcKind::OpenEuler => draw_openeuler(&mut img, color),
        ProcKind::OpenKylin => draw_openkylin(&mut img, color),
    }
    img
}

/// Classic four-pane Windows mark (stylized, not a trademark vector dump).
fn draw_windows(img: &mut RgbaImage, rgb: [u8; 3]) {
    let gap = 3i32;
    let margin = 8i32;
    let inner = SIZE as i32 - margin * 2;
    let cell = (inner - gap) / 2;
    let panes = [
        (margin, margin),
        (margin + cell + gap, margin),
        (margin, margin + cell + gap),
        (margin + cell + gap, margin + cell + gap),
    ];
    for (ox, oy) in panes {
        fill_rect(img, ox, oy, cell, cell, rgb, 255);
    }
}

/// openEuler-inspired: blue disc + lighter ring accent (flat, recognizable at 14px).
fn draw_openeuler(img: &mut RgbaImage, rgb: [u8; 3]) {
    let cx = (SIZE as f32 - 1.0) * 0.5;
    let cy = cx;
    fill_circle(img, cx, cy, 26.0, rgb, 255);
    let light = [
        rgb[0].saturating_add(40).min(255),
        rgb[1].saturating_add(40).min(255),
        rgb[2].saturating_add(20).min(255),
    ];
    // Inner cutout ring suggestion via lighter arc blob.
    fill_circle(img, cx + 4.0, cy - 2.0, 10.0, light, 255);
    // Small “e” bar.
    fill_rect(img, 22, 30, 20, 5, [255, 255, 255], 255);
    fill_rect(img, 22, 22, 5, 20, [255, 255, 255], 255);
}

/// openKylin-inspired: green rounded shield with a pale stripe.
fn draw_openkylin(img: &mut RgbaImage, rgb: [u8; 3]) {
    let cx = (SIZE as f32 - 1.0) * 0.5;
    let cy = 28.0;
    fill_circle(img, cx, cy, 22.0, rgb, 255);
    // Pointed bottom (shield tip).
    for y in 40..58 {
        let t = (y - 40) as f32 / 18.0;
        let half = (1.0 - t) * 18.0;
        for x in 0..SIZE as i32 {
            let dx = x as f32 - cx;
            if dx.abs() <= half {
                img.put_pixel(x as u32, y as u32, Rgba([rgb[0], rgb[1], rgb[2], 255]));
            }
        }
    }
    fill_rect(img, 28, 20, 8, 22, [255, 255, 255], 230);
}

fn fill_rect(img: &mut RgbaImage, x: i32, y: i32, w: i32, h: i32, rgb: [u8; 3], a: u8) {
    for py in y..y + h {
        for px in x..x + w {
            if px >= 0 && py >= 0 && px < SIZE as i32 && py < SIZE as i32 {
                img.put_pixel(px as u32, py as u32, Rgba([rgb[0], rgb[1], rgb[2], a]));
            }
        }
    }
}

fn fill_circle(img: &mut RgbaImage, cx: f32, cy: f32, r: f32, rgb: [u8; 3], a: u8) {
    let r2 = r * r;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            if dx * dx + dy * dy <= r2 {
                img.put_pixel(x, y, Rgba([rgb[0], rgb[1], rgb[2], a]));
            }
        }
    }
}

fn pixmap_to_image(pixmap: Pixmap) -> RgbaImage {
    let w = pixmap.width();
    let h = pixmap.height();
    let mut img: RgbaImage = ImageBuffer::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let p = pixmap.pixel(x, y).unwrap();
            let a = p.alpha();
            let (r, g, b) = if a == 0 {
                (0, 0, 0)
            } else if a == 255 {
                (p.red(), p.green(), p.blue())
            } else {
                let af = a as f32;
                (
                    ((p.red() as f32 / af) * 255.0).round().clamp(0.0, 255.0) as u8,
                    ((p.green() as f32 / af) * 255.0).round().clamp(0.0, 255.0) as u8,
                    ((p.blue() as f32 / af) * 255.0).round().clamp(0.0, 255.0) as u8,
                )
            };
            img.put_pixel(x, y, Rgba([r, g, b, a]));
        }
    }
    img
}
